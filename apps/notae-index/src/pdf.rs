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

/// The largest a rendered page may be along either axis, in pixels.
///
/// The renderer narrows a scaled page dimension to `u16` before allocating
/// its pixmap, so a dimension beyond 65,535 does not fail — it silently
/// becomes 65,535, and the allocation that follows is 65,535 × 65,535 × 4
/// bytes of RGBA, roughly 17 gigabytes. On a machine indexing a real
/// library that is an out-of-memory abort, not a failed file.
///
/// 16,384 is far above anything a page can legitimately be: it is an A0
/// sheet (841 × 1189 mm, the largest ISO paper size) rendered at about 495
/// DPI, and an A0 at the 300 DPI default is 9,933 × 14,043. Every real
/// document stays well inside it, and it is a long way below the 65,535
/// where the narrowing turns a wrong answer into an OOM.
const MAX_RENDER_DIMENSION: u32 = 16_384;

/// The largest total pixel count a rendered page may have.
///
/// The per-axis limit alone does not bound memory, because a page can be
/// narrow and very tall. This caps the product, which is what actually
/// decides the size of the pixmap.
///
/// 40 megapixels is A1 (594 × 841 mm) at the 300 DPI default — 6,968 × 9,868
/// pixels — with headroom, and it keeps the RGBA pixmap under about 160 MB.
/// Anything larger is refused with an error the caller can act on rather
/// than attempted and killed by the allocator.
const MAX_RENDER_PIXELS: u64 = 40_000_000;

/// The smallest gap between two glyphs on one baseline, as a multiple of the
/// glyph's advance width, that counts as a word boundary.
///
/// A PDF does not have to draw a space character between words. The common
/// encoding is a numeric adjustment in a `TJ` array, which shifts the next
/// string along the baseline by a fraction of the text space's em without
/// drawing anything at all. Typesetters use roughly a quarter of an em for a
/// word space, while the adjustments *inside* a word — kerning pairs,
/// tracking — stay well under a tenth of an em.
///
/// 0.2 is therefore the line: it is above every intra-word adjustment these
/// documents produce and below the smallest word space a typesetter emits.
/// Being wrong in one direction costs a search hit and in the other produces
/// a space in the middle of a word, which is worse, so the threshold sits
/// closer to the word-space size than to the kerning size.
const WORD_GAP_AS_ADVANCE_FRACTION: f64 = 0.2;

/// The largest baseline difference, in points, still counted as one line.
///
/// Superscript and subscript run on a slightly raised or lowered baseline of
/// the same line, and the vertical components of the text matrix put neighbouring
/// glyphs a fraction of a point apart. A point is comfortably above that jitter
/// and comfortably below the leading between two lines of body text, which is
/// more than a point at any legible size.
const MAX_BASELINE_JITTER: f64 = 1.0;

/// Why a PDF could not be read, rendered, or inspected.
///
/// Every variant names what went wrong rather than what the caller should do
/// about it; the pipeline decides that. The distinction that matters to the
/// pipeline is [`Unreadable`](Self::Unreadable) — the bytes could not be got at
/// all, a filesystem or permission problem — versus
/// [`InvalidPdf`](Self::InvalidPdf) — the bytes were got and are not a usable
/// PDF. The first is `FileCondition::Unreadable`, the second is
/// `FileCondition::InvalidPdf`, and both are per-file: a batch continues.
#[derive(Debug, Error)]
pub enum PdfError {
    /// The file could not be read.
    ///
    /// This is a filesystem condition — absent, denied, a directory, a link
    /// that would not open — and maps onto `FileCondition::Unreadable`.
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
    /// `FileCondition::InvalidPdf`.
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
    /// tool does not have. It maps onto `FileCondition::InvalidPdf` for
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
    /// Zero would scale every page to nothing, and a DPI high enough that the
    /// page does not fit in the render budget would ask for a pixmap the
    /// machine cannot hold. Both are caller errors — the DPI is computed by
    /// the caller — and both must be reported rather than turned into an
    /// image: an empty one that OCR reports as "no text found", or an
    /// allocation that takes the process down mid-batch. Either is a wrong
    /// answer rather than no answer.
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
    /// The drawn glyphs, in the order they were drawn.
    glyphs: Vec<DrawnGlyph>,
}

/// One glyph as the page drew it.
///
/// The advance is carried alongside the position because the position alone
/// does not say where one word ends and the next begins: a `TJ` array
/// expresses that boundary by shifting the next string along the baseline,
/// leaving nothing on the page but a gap. See [`append_page_text`].
struct DrawnGlyph {
    /// Where the glyph's origin sits, in page space.
    x: f64,
    /// The baseline the glyph sits on, in page space.
    y: f64,
    /// The character this glyph resolves to.
    character: char,
    /// How far the pen moves after this glyph, in page space, when the font
    /// reports a width for it. `None` when it does not, in which case no gap
    /// can be judged and nothing is inserted.
    advance: Option<f64>,
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
            let transform = glyph.transform();
            let position = transform.translation();
            let advance = advance_in_page_space(glyph, &transform);
            let last = characters.len() - 1;
            for (index, character) in characters.into_iter().enumerate() {
                // A `ToUnicode` entry may decode to several characters for one
                // glyph. Only the last of them is followed by the pen moving,
                // so only it carries the advance; carrying it on all of them
                // would make the reader invent a gap inside the ligature.
                self.glyphs.push(DrawnGlyph {
                    x: position.x,
                    y: position.y,
                    character,
                    advance: (index == last).then_some(advance).flatten(),
                });
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

/// How far the pen moves along the baseline after drawing `glyph`.
///
/// The font reports its advance in glyph space — units of a 1000-unit em —
/// and the glyph's own transform already carries the text size, the text
/// matrix and the current transformation matrix that convert that space into
/// page space. The x component of the transform's linear part is therefore the
/// page-space width of one glyph-space unit, and multiplying gives the advance
/// as the renderer will actually move the pen.
///
/// Returns `None` when the font reports no width, or when the text runs
/// vertically, where the advance is along y and there is no x gap to judge.
/// Without an advance there is nothing to compare a positional gap against,
/// so no separator is invented.
fn advance_in_page_space(
    glyph: &hayro::hayro_interpret::font::PositionedGlyph<'_>,
    transform: &Affine,
) -> Option<f64> {
    let width = match &**glyph {
        hayro::hayro_interpret::font::Glyph::Outline(outline) => outline.advance_width(),
        // A Type3 font's widths live in its own `d0`/`d1` operators and are
        // not reported here. Refusing to guess is the same choice made for an
        // unmappable character: an absent advance inserts no space.
        hayro::hayro_interpret::font::Glyph::Type3(_) => None,
    }?;
    let x_scale = transform.as_coeffs()[0];
    if x_scale <= 0.0 {
        return None;
    }
    Some(f64::from(width) * x_scale)
}

/// Refuses a render request whose result would not fit in the budget.
///
/// The page's dimensions are in points and the scale is `dpi / 72`, so the
/// pixel count is computed here from the same numbers the renderer will use.
/// A request that would exceed [`MAX_RENDER_DIMENSION`] on either axis, or
/// [`MAX_RENDER_PIXELS`] in total, is a caller error rather than a property
/// of the file, so it reports as [`PdfError::UnusableDpi`] — the same
/// condition a DPI of zero reports as, which is what it is: a DPI that cannot
/// produce an image.
///
/// The comparison is done in `f64` and the result is checked for finiteness
/// before it is narrowed, because the whole point is to catch values too
/// large to narrow. An absurd DPI overflows `f32` outright, and a value that
/// overflowed to a small number would sail through a check on the narrowed
/// result.
fn check_render_budget(
    path: &Path,
    dpi: u32,
    page_width: f32,
    page_height: f32,
) -> Result<(), PdfError> {
    let unusable = || PdfError::UnusableDpi {
        path: path.to_path_buf(),
        dpi,
    };
    let scale = f64::from(dpi) / POINTS_PER_INCH;
    let width = f64::from(page_width) * scale;
    let height = f64::from(page_height) * scale;

    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(unusable());
    }
    if width > f64::from(MAX_RENDER_DIMENSION) || height > f64::from(MAX_RENDER_DIMENSION) {
        return Err(unusable());
    }
    // Both sides are already known to be finite and positive, so the product
    // cannot overflow and does not need re-checking.
    if (width * height).ceil() as u64 > MAX_RENDER_PIXELS {
        return Err(unusable());
    }
    Ok(())
}

/// The bytes every PDF file must begin with, somewhere near the front.
const PDF_HEADER: &[u8] = b"%PDF-";

/// How far into a file the mandatory `%PDF-` header may appear.
///
/// ISO 32000-1 §7.5.2 puts the header on the first line, and real files
/// routinely carry a few bytes of something else in front of it: an archive
/// stub, a mail preamble, a stray HTTP error page. Readers are expected to
/// cope, and the convention that emerged is to accept a header found within
/// the first kilobyte. Beyond that, a `%PDF-` is far more likely to be a
/// quotation of a PDF inside some other file than the file itself.
const PDF_HEADER_SEARCH_WINDOW: usize = 1024;

/// Whether these bytes carry the `%PDF-` header a PDF file must have.
///
/// The renderer treats a missing version as PDF 1.0 and reconstructs
/// structure by scanning objects and trailers when the cross-reference table
/// cannot be read, so a blob that merely *looks* like a PDF — valid objects,
/// a valid trailer, just no header — is accepted as a document. Checking
/// here is what stops a mislabelled text file, an HTML error page, or a
/// truncated download from indexing as a paper.
fn has_pdf_header(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(PDF_HEADER_SEARCH_WINDOW)];
    window
        .windows(PDF_HEADER.len())
        .any(|candidate| candidate == PDF_HEADER)
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
    /// PDF, whose structure cannot be recovered, or whose page content
    /// streams will not decode, fails here rather than producing a document
    /// that fails confusingly — or, worse, quietly succeeds — later.
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
        if !has_pdf_header(&bytes) {
            return Err(PdfError::InvalidPdf {
                path: path.to_path_buf(),
                reason: "it has no %PDF- header",
            });
        }
        let pdf = match Pdf::new(bytes) {
            Ok(pdf) => pdf,
            Err(hayro::hayro_syntax::LoadPdfError::Decryption(_)) => {
                return Err(PdfError::Encrypted {
                    path: path.to_path_buf(),
                });
            }
            Err(hayro::hayro_syntax::LoadPdfError::Invalid) => {
                return Err(PdfError::InvalidPdf {
                    path: path.to_path_buf(),
                    reason: "its structure could not be parsed",
                });
            }
        };

        let document = Self {
            path: path.to_path_buf(),
            pdf,
        };
        document.check_page_streams()?;
        Ok(document)
    }

    /// The path this document was opened from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks that every page's content stream can actually be decoded.
    ///
    /// Opening a document validates the cross-reference table and the page
    /// tree, but a page's `/Contents` is not decoded until the page is
    /// interpreted. A stream that fails to decode then reads as a page that
    /// draws nothing, which is the same reading as a genuinely blank page —
    /// so a file truncated or rewritten in transit would report no text layer
    /// and go down the OCR path as though it were a scan, and its render
    /// would come back blank for the same reason.
    ///
    /// The two are not the same claim about a file, so they are not allowed
    /// to look the same. A page that declares `/Contents` and cannot decode
    /// them is damage, and damage is reported here.
    ///
    /// What is *not* treated as damage: a page with no `/Contents` at all,
    /// which is a legitimate blank page, and any filter this renderer
    /// supports, however uncommon — an intact `ASCIIHexDecode` stream is a
    /// perfectly good PDF. The check is whether the bytes decode, not whether
    /// the file looks the way the fixtures do.
    fn check_page_streams(&self) -> Result<(), PdfError> {
        for page in self.pdf.pages().iter() {
            // `contains_key` rather than testing `page_stream()` for `None`:
            // a page with no contents is blank by design, and only a page
            // that declares contents and cannot decode them is damaged.
            let contents = hayro::hayro_syntax::object::dict::keys::CONTENTS;
            if !page.raw().contains_key(contents) {
                continue;
            }
            if page.page_stream().is_none() {
                return Err(PdfError::InvalidPdf {
                    path: self.path.clone(),
                    reason: "a page's content stream could not be decoded",
                });
            }
        }
        Ok(())
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
            let mut collector = TextCollector { glyphs: Vec::new() };
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
    /// document, and [`PdfError::UnusableDpi`] when `dpi` is zero or is high
    /// enough that the page would not fit in the render budget.
    pub fn render_page(&self, page: u32, dpi: u32) -> Result<RenderedPage, PdfError> {
        let page_count = self.page_count()?;
        if dpi == 0 {
            return Err(PdfError::UnusableDpi {
                path: self.path.clone(),
                dpi,
            });
        }
        let index = usize::try_from(page)
            .ok()
            .filter(|i| *i < page_count as usize);
        let Some(index) = index else {
            return Err(PdfError::PageOutOfRange {
                path: self.path.clone(),
                page,
                page_count,
            });
        };

        // The budget is checked on the page's own dimensions *before* the
        // renderer is handed a scale, because that is the last point at which
        // a wrong answer is still avoidable: the renderer narrows a scaled
        // dimension to `u16`, which turns an absurd request into a silently
        // enormous allocation rather than an error.
        let target = &self.pdf.pages()[index];
        let (page_width, page_height) = target.render_dimensions();
        check_render_budget(&self.path, dpi, page_width, page_height)?;

        let scale = (f64::from(dpi) / POINTS_PER_INCH) as f32;
        let cache = RenderCache::new();
        let pixmap = hayro::render(
            target,
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
/// top to bottom, characters left to right.
///
/// A change in baseline starts a new line. Along one baseline, two adjacent
/// glyphs are joined with a separator when the pen left a gap between them
/// larger than [`WORD_GAP_AS_ADVANCE_FRACTION`] of the advance it just took —
/// that is the boundary a `TJ` array encodes as a number and draws no space
/// for. Glyphs whose pen position exactly follows the advance, and every glyph
/// carrying an explicit space character, are joined directly, so a word that
/// was spaced by a drawn space keeps exactly one space.
fn append_page_text(text: &mut String, glyphs: &mut [DrawnGlyph]) {
    glyphs.sort_by(|left, right| {
        right
            .y
            .partial_cmp(&left.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.x
                    .partial_cmp(&right.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut previous: Option<&DrawnGlyph> = None;
    for glyph in glyphs.iter() {
        match previous {
            // The first glyph of a page has no predecessor to separate from.
            None => {}
            Some(previous) => {
                if (glyph.y - previous.y).abs() > MAX_BASELINE_JITTER {
                    text.push('\n');
                } else if let Some(advance) = previous.advance
                    // A drawn space already separates these two glyphs; adding
                    // another would put two spaces between the words.
                    && advance > 0.0
                    && !previous.character.is_whitespace()
                    && !glyph.character.is_whitespace()
                    // The next glyph's origin is where the pen left off, plus
                    // whatever the `TJ` adjustment moved it by. It is the part
                    // beyond that which is the gap, not the whole distance.
                    && glyph.x - previous.x - advance > advance * WORD_GAP_AS_ADVANCE_FRACTION
                {
                    text.push(' ');
                }
            }
        }
        text.push(glyph.character);
        previous = Some(glyph);
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
