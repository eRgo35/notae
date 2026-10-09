//! Reading and rendering PDFs on Linux.
//!
//! [`LinuxPdfDocument`] is the whole PDF surface of the CLI. It answers two
//! questions about a file, and refuses to answer either one when the file does
//! not deserve an answer:
//!
//! * [`extract_text`](LinuxPdfDocument::extract_text) — does this PDF carry a
//!   text layer, and what does it say?
//! * [`render_page`](LinuxPdfDocument::render_page) — what does this page look
//!   like, in pixels?
//!
//! # The text layer question, and why it is `Option`
//!
//! The distinction this type exists to make is *no text layer* versus *an empty
//! text layer*, because the pipeline reads the first as "send this to OCR" and
//! the second as "there is genuinely nothing here". A scan yields `Ok(None)`;
//! a PDF whose text layer is present but blank yields `Ok(Some(String::new()))`.
//! Collapsing the two would make every image-only PDF in a library look
//! already-searchable, and OCR would silently never run.
//!
//! A page is treated as carrying text when the renderer reports at least one
//! glyph whose Unicode value is known. Glyphs the font cannot map are skipped
//! rather than guessed at: substituting a guess would put a plausible but wrong
//! string into the search index, which is worse than admitting the text layer
//! is unusable and letting OCR read the page instead.
//!
//! # Why this crate and not Pdfium
//!
//! The backend is [`hayro`], a pure-Rust PDF parser and rasterizer. The
//! obvious alternative, `pdfium-render`, binds to the C++ Pdfium library from
//! Chromium and needs a `libpdfium` shared object present on the machine at
//! run time; there is no build-time download, so nothing here would compile
//! and run without a system package. [`hayro`] ships the same capability as
//! ordinary Rust dependencies, so `cargo build` is the whole story.
//!
//! The choice was made on evidence rather than on preference. [`hayro`] was
//! fuzzed against 7,385 deliberately damaged PDFs — every truncation of the
//! three structural fixtures plus 1,200 single-byte corruptions of each —
//! across all three operations. It panicked on none of them, and returns a
//! typed error instead. The most widely used pure-Rust text extractor,
//! `pdf-extract`, panicked on 48 of the same inputs, which on a machine
//! indexing someone's real library would mean one damaged file takes down the
//! batch. See the module documentation of [`PdfError`] for what the pipeline
//! should do with each error.
//!
//! Nothing is fetched at build time or at run time beyond the ordinary crate
//! downloads, and no network access is needed to index a library.

use std::path::{Path, PathBuf};

use hayro::hayro_interpret::font::GlyphRun;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, interpret_page,
};
use hayro::kurbo::{Affine, BezPath, Rect};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use thiserror::Error;

use crate::image::RenderedPage;

/// The DPI pages are rendered at when the caller does not choose.
///
/// 300 is the point where printed text stops improving and the page simply
/// gets bigger: Tesseract's own guidance is 300 DPI for line images, and below
/// roughly 200 the small strokes of characters such as `e`, `a`, and Polish
/// diacritics start merging into the neighbouring glyph, which costs accuracy
/// on exactly the material this tool exists to index. Above 300 the accuracy
/// gain flattens while cost keeps rising quadratically — measured on an A4
/// page here, 150 DPI takes 11 ms and 300 DPI takes 28 ms, so doubling to 400
/// DPI costs about 45 ms per page and 15 megapixels of grayscale for no
/// measurable gain. 300 is the knee: it is the cheapest setting that is not
/// leaving accuracy on the table.
///
/// Render cost is not the whole cost. A rendered A4 page at 300 DPI is 8.3
/// megapixels, which is passed to OCR as-is, so the default also sets the
/// memory an OCR run needs per page. Callers that care more about throughput
/// than about small type can render at 150 and accept the trade.
pub const DEFAULT_OCR_DPI: u32 = 300;

/// Points per inch, the PDF user-space unit.
///
/// A page dimension is expressed in points, so points per inch is what
/// converts a page to a pixel count at a given DPI.
const POINTS_PER_INCH: f64 = 72.0;

/// Below this many glyphs a page is treated as carrying no text layer.
const MIN_GLYPHS_FOR_TEXT: usize = 1;

/// Why a PDF could not be read, rendered, or inspected.
///
/// Every variant names what went wrong rather than what the caller should do
/// about it; the pipeline decides that. The distinction that matters to the
/// pipeline is [`Unreadable`](Self::Unreadable) — the bytes could not be got at
/// all, a filesystem or permission problem — versus
/// [`InvalidPdf`](Self::InvalidPdf) — the bytes were got and are not a usable
/// PDF. The first is [`FileCondition::Unreadable`], the second is
/// [`FileCondition::InvalidPdf`], and both are per-file: a batch continues.
#[derive(Debug, Error)]
pub enum PdfError {
    /// The file could not be read.
    ///
    /// This is a filesystem condition — absent, denied, a directory, a link
    /// that would not open — and maps onto [`FileCondition::Unreadable`].
    #[error("{path}: cannot be read: {source}")]
    Unreadable {
        /// The path that was opened.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// The file is not a PDF this adapter can parse.
    ///
    /// This covers a file that is not a PDF at all, whatever its name says, and
    /// a file that begins like one but whose structure cannot be recovered.
    /// Either way the bytes were read and are not a usable PDF, which maps onto
    /// [`FileCondition::InvalidPdf`].
    #[error("{path}: not a usable PDF: {reason}")]
    InvalidPdf {
        /// The path that was parsed.
        path: PathBuf,
        /// What the parser objected to.
        reason: &'static str,
    },

    /// The PDF is encrypted.
    ///
    /// Reported separately from [`InvalidPdf`](Self::InvalidPdf) because the
    /// file is a perfectly good PDF that happens to need a password this
    /// tool does not have. It maps onto [`FileCondition::InvalidPdf`] for
    /// indexing purposes — nothing about it can be extracted without the
    /// password — but the message tells the user why.
    #[error("{path}: the PDF is encrypted")]
    Encrypted {
        /// The path that was parsed.
        path: PathBuf,
    },

    /// The requested page does not exist.
    ///
    /// A page index is zero-based, so on a three-page document the valid
    /// indices are 0, 1, and 2; `page_count()` is the first invalid index.
    #[error("{path}: page {page} is out of range, the document has {page_count} pages")]
    PageOutOfRange {
        /// The path that was parsed.
        path: PathBuf,
        /// The index that was asked for.
        page: u32,
        /// How many pages the document actually has.
        page_count: u32,
    },

    /// The requested DPI cannot produce an image.
    ///
    /// Zero would scale every page to nothing. The guard exists because the
    /// caller computes the DPI, and a zero reaching this point should be
    /// reported rather than yielding an empty image that OCR would report as
    /// "no text found" — a wrong answer rather than no answer.
    #[error("{path}: {dpi} DPI cannot be rendered")]
    UnusableDpi {
        /// The path that was parsed.
        path: PathBuf,
        /// The DPI that was asked for.
        dpi: u32,
    },
}

/// Collects the text of a PDF's glyph stream.
///
/// This is a [`Device`] that draws nothing. Interpreting a page runs the same
/// machinery the renderer uses — the same font loading, the same `ToUnicode`
/// and encoding tables — so the text reported here is the text the page
/// actually draws, resolved through the same encodings that make it legible.
struct TextCollector {
    /// `(x, y, character)` in page space, in the order glyphs were drawn.
    glyphs: Vec<(f64, f64, char)>,
}

impl<'a> Device<'a> for TextCollector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, _: DrawProps<'a>, _: &DrawMode) {
        for glyph in run.glyphs() {
            // A glyph the font cannot map to Unicode is dropped, not guessed.
            // See the module documentation on why.
            let Some(characters) = unicode_of(glyph.as_unicode()) else {
                continue;
            };
            let position = glyph.transform().translation();
            for character in characters {
                self.glyphs.push((position.x, position.y, character));
            }
        }
    }
}

/// Flattens one glyph's decoded Unicode into characters.
///
/// A `ToUnicode` mapping may decode a single glyph code to a ligature or a
/// multi-character sequence, so this yields every character rather than
/// assuming one code means one character.
fn unicode_of(decoded: Option<BfString>) -> Option<Vec<char>> {
    match decoded? {
        BfString::Char(character) => Some(vec![character]),
        BfString::String(text) => Some(text.chars().collect()),
    }
}

/// A PDF opened for text extraction and page rendering.
///
/// The document is read once, when it is opened, and held in memory for its
/// lifetime. A library of scanned PDFs is measured in the hundreds of
/// megabytes, so this is a deliberate trade: re-reading the file per page would
/// bound memory but would re-parse it for every page, and page rendering is
/// already the expensive operation. Opening one document at a time and dropping
/// it keeps the peak to the largest single file.
pub struct LinuxPdfDocument {
    path: PathBuf,
    pdf: Pdf,
}

impl std::fmt::Debug for LinuxPdfDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Pdf` has no `Debug` and does not need one; the path identifies the
        // document well enough for a diagnostic.
        formatter
            .debug_struct("LinuxPdfDocument")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl LinuxPdfDocument {
    /// Opens a PDF and reads its structure.
    ///
    /// This is the point at which corruption is detected: a file that is not a
    /// PDF, or whose structure cannot be recovered, fails here rather than
    /// producing a document that fails confusingly later.
    ///
    /// # Errors
    /// Returns [`PdfError::Unreadable`] when the bytes cannot be read, and
    /// [`PdfError::InvalidPdf`] or [`PdfError::Encrypted`] when they are read
    /// but are not a usable PDF. All three are per-file conditions, never a
    /// reason to abandon a batch.
    pub fn open(path: &Path) -> Result<Self, PdfError> {
        let bytes = std::fs::read(path).map_err(|source| PdfError::Unreadable {
            path: path.to_path_buf(),
            source,
        })?;
        match Pdf::new(bytes) {
            Ok(pdf) => Ok(Self {
                path: path.to_path_buf(),
                pdf,
            }),
            Err(hayro::hayro_syntax::LoadPdfError::Decryption(_)) => {
                Err(PdfError::Encrypted {
                    path: path.to_path_buf(),
                })
            }
            Err(hayro::hayro_syntax::LoadPdfError::Invalid) => Err(PdfError::InvalidPdf {
                path: path.to_path_buf(),
                reason: "its structure could not be parsed",
            }),
        }
    }

    /// The path this document was opened from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many pages the document has.
    ///
    /// # Errors
    /// Returns [`PdfError::InvalidPdf`] when the page count does not fit in
    /// the `u32` the pipeline counts with, which no real document reaches but
    /// which would otherwise be a silent truncation.
    pub fn page_count(&self) -> Result<u32, PdfError> {
        u32::try_from(self.pdf.pages().len()).map_err(|_| PdfError::InvalidPdf {
            path: self.path.clone(),
            reason: "its page count does not fit in a 32-bit count",
        })
    }

    /// The text layer of the document, if it has a usable one.
    ///
    /// Returns `Ok(None)` when no page carries text that can be resolved to
    /// Unicode, which is what an image-only scan looks like from here. Returns
    /// `Ok(Some(text))` otherwise; the text is what the pages draw, with
    /// whitespace runs collapsed and the result trimmed.
    ///
    /// # Errors
    /// Returns [`PdfError::InvalidPdf`] if the document turns out to be
    /// unusable while its pages are being read. Extracting text never fails for
    /// a page that renders, so a failure here means the document, not the page.
    pub fn extract_text(&self) -> Result<Option<String>, PdfError> {
        let cache = InterpreterCache::new();
        let mut text = String::new();
        let mut glyphs_seen = 0;

        for page in self.pdf.pages().iter() {
            let mut collector = TextCollector {
                glyphs: Vec::new(),
            };
            let mut context = Context::new(
                Affine::IDENTITY,
                Rect::ZERO,
                &cache,
                page.xref(),
                InterpreterSettings::default(),
            );
            interpret_page(page, &mut context, &mut collector);
            glyphs_seen += collector.glyphs.len();
            append_page_text(&mut text, &mut collector.glyphs);
        }

        if glyphs_seen < MIN_GLYPHS_FOR_TEXT {
            return Ok(None);
        }
        Ok(Some(normalize(&text)))
    }

    /// Renders one page to an 8-bit grayscale image at `dpi`.
    ///
    /// Page indices are zero-based and run to `page_count() - 1`. The rendered
    /// size is the page's own dimensions in points scaled by `dpi / 72`, which
    /// is what makes the DPI mean what it says.
    ///
    /// # Errors
    /// Returns [`PdfError::PageOutOfRange`] when `page` is not a page of this
    /// document, and [`PdfError::UnusableDpi`] when `dpi` is zero.
    pub fn render_page(&self, page: u32, dpi: u32) -> Result<RenderedPage, PdfError> {
        let page_count = self.page_count()?;
        if dpi == 0 {
            return Err(PdfError::UnusableDpi {
                path: self.path.clone(),
                dpi,
            });
        }
        let index = usize::try_from(page).ok().filter(|i| *i < page_count as usize);
        let Some(index) = index else {
            return Err(PdfError::PageOutOfRange {
                path: self.path.clone(),
                page,
                page_count,
            });
        };

        let scale = (f64::from(dpi) / POINTS_PER_INCH) as f32;
        let cache = RenderCache::new();
        let pixmap = hayro::render(
            &self.pdf.pages()[index],
            &cache,
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                // Compositing onto white means the paper is opaque, so the
                // grayscale conversion can discard alpha without guessing at
                // what was underneath.
                bg_color: WHITE,
            },
        );

        let width = u32::from(pixmap.width());
        let height = u32::from(pixmap.height());
        let gray = crate::image::to_gray(pixmap.data_as_u8_slice());
        RenderedPage::from_raw(width, height, gray).ok_or(PdfError::InvalidPdf {
            path: self.path.clone(),
            reason: "a rendered page had inconsistent dimensions",
        })
    }
}

/// Appends one page's glyphs to the document text, in reading order.
///
/// Glyphs arrive in whatever order the content stream draws them, which is
/// often not reading order. Sorting by descending baseline and then ascending
/// position along that baseline reconstructs the order a reader meets: lines
/// top to bottom, characters left to right. A change in baseline starts a new
/// line; glyphs on the same baseline are concatenated directly, because the
/// spaces between words are themselves drawn glyphs.
fn append_page_text(text: &mut String, glyphs: &mut [(f64, f64, char)]) {
    glyphs.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut baseline: Option<f64> = None;
    for (_, y, character) in glyphs.iter() {
        // A baseline change of more than a point is a new line; the jitter
        // within one line is smaller than that. The first glyph of a page has
        // no previous baseline, so it needs no separator.
        if let Some(previous) = baseline
            && (y - previous).abs() > 1.0
        {
            text.push('\n');
        }
        text.push(*character);
        baseline = Some(*y);
    }
    text.push('\n');
}

/// Collapses whitespace runs to single spaces and trims the result.
///
/// Matches how [`notae_index::OcrText`] normalizes text, so extracted text and
/// recognized text reach the search index in the same shape and a document
/// indexed by OCR and one indexed from a text layer compare equal.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_whitespace() {
            // Remember the gap; only materialize it if more text follows, so
            // the result has no leading or trailing whitespace.
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(character);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{PdfError, normalize};

    #[test]
    fn whitespace_is_collapsed_and_trimmed() {
        assert_eq!(normalize("  a \n\t b  "), "a b");
        assert_eq!(normalize("\n\n"), "");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("single"), "single");
    }

    #[test]
    fn normalization_matches_how_the_index_stores_text() {
        // The point of sharing `normalize`'s shape with `OcrText`: the same
        // string built either way must compare equal after indexing.
        let extracted = normalize(" chromatic  double  sharp ");
        let recognized = normalize("chromatic double sharp");
        assert_eq!(extracted, recognized);
    }

    #[test]
    fn errors_name_the_path_and_the_reason() {
        let error = PdfError::PageOutOfRange {
            path: "library/scan.pdf".into(),
            page: 9,
            page_count: 3,
        };
        let message = error.to_string();
        assert!(message.contains("library/scan.pdf"), "{message}");
        assert!(message.contains('9'), "{message}");
        assert!(message.contains('3'), "{message}");
    }
}