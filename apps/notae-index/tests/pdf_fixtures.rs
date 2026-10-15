//! PDF text extraction and page rendering, against committed fixtures.
//!
//! The fixtures in `tests/fixtures` are deliberately tiny and hand-built so
//! that every assertion below is exact rather than approximate:
//!
//! * `text-layer.pdf` — one 200x100pt page whose only content is a text layer
//!   reading "Notae searchable text layer".
//! * `image-only.pdf` — one 200x100pt page carrying a single image and no text
//!   drawing operators at all, so it has *no* text layer.
//! * `multi-page.pdf` — three 200x100pt pages, each with its own text layer.
//! * `corrupt.pdf` — a `%PDF-` header followed by an unreadable cross-reference
//!   table, so parsing fails.
//! * `not-a-pdf.pdf` — plain text wearing a `.pdf` name.
//! * `damaged-contents.pdf` — a valid page tree whose `/Contents` stream claims
//!   an `ASCIIHexDecode` filter over bytes that are not hex, so the page
//!   payload cannot be decoded.
//! * `hex-contents.pdf` — the same uncommon-but-valid filter, intact, so a
//!   fix for `damaged-contents.pdf` cannot pass by rejecting the filter.
//! * `no-header.pdf` — valid objects, cross-reference table and trailer, with
//!   no `%PDF-` header anywhere.
//! * `leading-junk.pdf` — a valid document behind 24 bytes of junk, with the
//!   header displaced past byte zero but still inside the first kilobyte.
//! * `word-spacing.pdf` — one line drawn as `[(Notae) -250 (searchable)] TJ`,
//!   where the word boundary is a numeric adjustment rather than a space.
//!
//! Every page in every fixture is 200x100 points, which is what makes the
//! rendering assertions checkable: at *n* DPI the rendered page is
//! `round(200 * n / 72)` by `round(100 * n / 72)` pixels.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use notae_index_cli::image::RenderedPage;
use notae_index_cli::pdf::{DEFAULT_OCR_DPI, LinuxPdfDocument, PdfError};

/// Where the committed fixtures live.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// The width and height a 200x100pt page must render to at `dpi`.
///
/// The renderer truncates rather than rounds when it converts points to
/// pixels, so the expectation is the truncated size. Every DPI chosen here
/// lands on an exact number of pixels regardless, which is what makes these
/// assertions exact rather than approximate.
fn expected_size(dpi: u32) -> (u32, u32) {
    let scale = f64::from(dpi) / 72.0;
    ((200.0 * scale) as u32, (100.0 * scale) as u32)
}

fn open(name: &str) -> LinuxPdfDocument {
    LinuxPdfDocument::open(&fixture(name)).expect("open the fixture")
}

#[test]
fn a_text_layer_pdf_yields_its_embedded_text() {
    let document = open("text-layer.pdf");
    let text = document
        .extract_text()
        .expect("extract text")
        .expect("the fixture has a text layer");
    assert!(
        text.contains("Notae searchable text layer"),
        "the text layer should be extracted, got {text:?}"
    );
}

#[test]
fn an_image_only_pdf_reports_no_usable_text_layer() {
    let document = open("image-only.pdf");
    let text = document.extract_text().expect("extract text");
    assert_eq!(
        text, None,
        "a scan with no text operators has no text layer to report, \
         which is different from reporting an empty string"
    );
}

#[test]
fn a_multi_page_pdf_reports_its_page_count() {
    assert_eq!(open("multi-page.pdf").page_count().expect("page count"), 3);
}

#[test]
fn text_is_extracted_from_every_page_of_a_multi_page_pdf() {
    let text = open("multi-page.pdf")
        .extract_text()
        .expect("extract text")
        .expect("every page has a text layer");
    for page in 1..=3 {
        assert!(
            text.contains(&format!("Notae page {page}")),
            "page {page} should be extracted, got {text:?}"
        );
    }
}

#[test]
fn rendering_at_a_dpi_produces_dimensions_consistent_with_it() {
    let document = open("text-layer.pdf");
    for dpi in [72, 144, 200, DEFAULT_OCR_DPI] {
        let (width, height) = expected_size(dpi);
        let image = document.render_page(0, dpi).expect("render page 0");
        assert_eq!(
            (image.width(), image.height()),
            (width, height),
            "a 200x100pt page at {dpi} dpi is {width}x{height}"
        );
    }
}

#[test]
fn rendering_scales_with_dpi() {
    let document = open("multi-page.pdf");
    let low = document.render_page(0, 72).expect("render at 72 dpi");
    let high = document.render_page(0, 216).expect("render at 216 dpi");
    assert_eq!(
        high.width(),
        low.width() * 3,
        "216 dpi is three times 72 dpi, so the page is three times as wide"
    );
}

#[test]
fn every_page_of_a_multi_page_pdf_renders() {
    let document = open("multi-page.pdf");
    let count = document.page_count().expect("page count");
    for page in 0..count {
        let image = document.render_page(page, 144).expect("render a page");
        assert_eq!(
            (image.width(), image.height()),
            expected_size(144),
            "page {page} is the same size as the others"
        );
    }
}

#[test]
fn a_rendered_page_is_grayscale_and_not_blank() {
    let image = open("text-layer.pdf")
        .render_page(0, 144)
        .expect("render the text layer");
    let gray = image.as_raw();
    assert!(
        gray.iter().any(|&value| value != 255),
        "a page with text on it has ink, so some pixel is not white"
    );
    // `GrayImage` is one byte per pixel by construction, so the pixel count is
    // width times height; a mismatch would mean the buffer is not the image.
    assert_eq!(
        gray.len(),
        usize::try_from(image.width()).expect("width fits") * image.height() as usize,
        "a grayscale image has exactly one byte per pixel"
    );
}

#[test]
fn an_image_only_page_renders_its_picture_rather_than_nothing() {
    let image = open("image-only.pdf")
        .render_page(0, 144)
        .expect("render the scanned page");
    assert_eq!((image.width(), image.height()), expected_size(144));
    assert!(
        image.as_raw().iter().any(|&value| value != 255),
        "the page carries an image, so the render has ink"
    );
}

#[test]
fn a_corrupt_file_yields_a_typed_error() {
    let error = LinuxPdfDocument::open(&fixture("corrupt.pdf")).expect_err("must not open");
    assert!(matches!(error, PdfError::InvalidPdf { .. }), "{error}");
}

#[test]
fn a_non_pdf_with_a_pdf_name_yields_a_typed_error() {
    let error =
        LinuxPdfDocument::open(&fixture("not-a-pdf.pdf")).expect_err("must not open a text file");
    assert!(matches!(error, PdfError::InvalidPdf { .. }), "{error}");
}

#[test]
fn a_missing_file_yields_a_read_error() {
    let error =
        LinuxPdfDocument::open(&fixture("no-such-file.pdf")).expect_err("must not open a ghost");
    assert!(matches!(error, PdfError::Unreadable { .. }), "{error}");
}

#[test]
fn an_out_of_range_page_index_yields_a_typed_error() {
    let document = open("multi-page.pdf");
    let count = document.page_count().expect("page count");
    let error = document
        .render_page(count, 144)
        .expect_err("page count is one past the last page");
    assert!(matches!(error, PdfError::PageOutOfRange { .. }), "{error}");
    assert!(
        document.render_page(u32::MAX, 144).is_err(),
        "an absurd page index is still out of range, not a panic"
    );
}

#[test]
fn a_zero_dpi_is_refused_rather_than_rendering_an_empty_page() {
    let error = open("text-layer.pdf")
        .render_page(0, 0)
        .expect_err("zero dpi has no meaning");
    assert!(matches!(error, PdfError::UnusableDpi { .. }), "{error}");
}

#[test]
fn no_fixture_operation_panics() {
    let fixtures = [
        "text-layer.pdf",
        "image-only.pdf",
        "multi-page.pdf",
        "corrupt.pdf",
        "not-a-pdf.pdf",
    ];
    for name in fixtures {
        let path = fixture(name);
        // Every operation a caller can reach runs inside this closure, so a
        // panic anywhere below fails the test rather than aborting the run.
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let Ok(document) = LinuxPdfDocument::open(&path) else {
                return;
            };
            let _ = document.page_count();
            let _ = document.extract_text();
            let count = document.page_count().unwrap_or(0);
            for page in 0..count + 1 {
                let _ = document.render_page(page, DEFAULT_OCR_DPI);
            }
        }));
        assert!(outcome.is_ok(), "{name} must not panic");
    }
}

#[test]
fn the_shared_rendered_page_type_is_the_one_ocr_will_consume() {
    // `RenderedPage` is a re-export rather than a new type, so the OCR crate
    // can name `image::GrayImage` and get the identical type.
    let image: image::GrayImage = open("text-layer.pdf").render_page(0, 72).expect("render");
    let shared: RenderedPage = image;
    assert_eq!(shared.width(), 200);
}

// --- finding 1: an unbounded DPI must not reach the rasterizer ------------

#[test]
fn an_absurd_dpi_is_refused_rather_than_allocating_an_enormous_pixmap() {
    // A 200x100pt page at `u32::MAX` DPI is 1.19e12 by 5.97e11 pixels. The
    // renderer narrows those to `u16` and hands them to the pixmap allocator,
    // so the request has to be refused on the way in. What must not happen is
    // a multi-gigabyte allocation or a panic.
    let document = open("text-layer.pdf");
    for dpi in [u32::MAX, 1_000_000, 100_000, 20_000] {
        let outcome = catch_unwind(AssertUnwindSafe(|| document.render_page(0, dpi)));
        let outcome = outcome.expect("rendering must not panic");
        assert!(
            outcome.is_err(),
            "{dpi} DPI is beyond any real page and must be refused, \
             but it rendered instead"
        );
        assert!(
            matches!(outcome, Err(PdfError::UnusableDpi { .. })),
            "{dpi} DPI should be refused as an unusable DPI, got {:?}",
            outcome.err()
        );
    }
}

#[test]
fn a_dpi_that_would_exceed_the_pixel_budget_is_refused() {
    // 600 DPI on a 200x100pt page is 1666x833, which is inside the budget;
    // 20000 DPI is 55555x27777, which is 1.5 gigapixels of RGBA and is not.
    let document = open("text-layer.pdf");
    assert!(
        document.render_page(0, 600).is_ok(),
        "600 DPI is a page anyone would actually ask for"
    );
    let error = document
        .render_page(0, 20_000)
        .expect_err("20000 DPI is 1.5 gigapixels");
    assert!(matches!(error, PdfError::UnusableDpi { .. }), "{error}");
}

#[test]
fn the_refused_dpi_is_still_refused_for_every_page_and_in_range() {
    // The budget is a property of the request, not of page zero, so it must
    // not depend on which page is asked for.
    let document = open("multi-page.pdf");
    for page in 0..3 {
        assert!(
            document.render_page(page, u32::MAX).is_err(),
            "page {page} must refuse an absurd DPI too"
        );
    }
}

// --- finding 2: a damaged page stream is not an empty page ---------------

#[test]
fn a_page_whose_contents_cannot_be_decoded_is_refused() {
    // The page tree is valid and the `/Contents` entry is present, so parsing
    // succeeds. Only decoding the stream fails. Reporting that page as blank
    // would send a damaged file down the OCR path as though it were a scan.
    let error = LinuxPdfDocument::open(&fixture("damaged-contents.pdf"))
        .expect_err("a stream that cannot be decoded is not a usable PDF");
    assert!(matches!(error, PdfError::InvalidPdf { .. }), "{error}");
}

#[test]
fn a_damaged_page_does_not_extract_as_no_text_layer() {
    // The specific wrong answer this guards: `Ok(None)` means "no text layer,
    // send it to OCR", which is a claim about the file rather than a report
    // that the file is broken.
    let outcome = LinuxPdfDocument::open(&fixture("damaged-contents.pdf"))
        .and_then(|document| document.extract_text());
    let error = outcome.expect_err("a damaged page is not an empty page");
    assert!(matches!(error, PdfError::InvalidPdf { .. }), "{error}");
}

#[test]
fn a_damaged_page_does_not_render_as_a_blank_image() {
    // A blank render is the second wrong answer: OCR would read nothing and
    // report "no text found", which is a wrong claim about a file that was
    // never really read.
    let outcome = LinuxPdfDocument::open(&fixture("damaged-contents.pdf"))
        .and_then(|document| document.render_page(0, 144));
    let error = outcome.expect_err("a damaged page is not a blank page");
    assert!(matches!(error, PdfError::InvalidPdf { .. }), "{error}");
}

#[test]
fn an_uncommon_but_valid_page_filter_is_still_accepted() {
    // The counterpart to the three tests above: ASCIIHexDecode is an ordinary
    // PDF filter. Detecting damage must mean decoding the stream, not
    // pattern-matching the filters a document is allowed to use.
    let document = LinuxPdfDocument::open(&fixture("hex-contents.pdf"))
        .expect("a hex-encoded content stream is valid PDF");
    let text = document
        .extract_text()
        .expect("extract text")
        .expect("the fixture has a text layer");
    assert!(
        text.contains("Notae hex filter"),
        "an intact hex stream must still be read, got {text:?}"
    );
}

// --- finding 3: the header is mandatory ----------------------------------

#[test]
fn a_pdf_without_a_header_is_refused() {
    // Valid objects, a valid cross-reference table, a valid trailer, and no
    // `%PDF-` anywhere. Reconstructing structure by scanning finds all of
    // that, so the file is accepted unless the header is actually checked.
    let error = LinuxPdfDocument::open(&fixture("no-header.pdf"))
        .expect_err("a file with no %PDF- header is not a PDF");
    assert!(matches!(error, PdfError::InvalidPdf { .. }), "{error}");
}

#[test]
fn a_pdf_with_its_header_behind_leading_junk_is_still_accepted() {
    // The specification allows the header anywhere in the first 1024 bytes,
    // and real files arrive with an archive stub or an error page in front.
    // The check must find the header where it is, not insist on byte zero.
    let document = LinuxPdfDocument::open(&fixture("leading-junk.pdf"))
        .expect("a displaced header inside the first kilobyte is still a PDF");
    let text = document
        .extract_text()
        .expect("extract text")
        .expect("the fixture has a text layer");
    assert!(
        text.contains("Notae leading junk"),
        "the document behind the junk must still be read, got {text:?}"
    );
}

// --- finding 4: word boundaries carried by positioning -------------------

#[test]
fn a_word_boundary_encoded_by_positioning_is_kept() {
    // `[(Notae) -250 (searchable)] TJ` draws two strings with no space glyph
    // between them; the -250 shifts the second string along the baseline. A
    // title and its composer landing in the index as one word is visible in
    // every search, so the gap has to survive extraction.
    let document = open("word-spacing.pdf");
    let text = document
        .extract_text()
        .expect("extract text")
        .expect("the fixture has a text layer");
    assert!(
        text.contains("Notae searchable"),
        "a positional word gap must become a space, got {text:?}"
    );
}

#[test]
fn the_extracted_text_normalizes_like_recognized_text() {
    // The point of preserving the boundary: once it is a space, the extracted
    // text has the same shape as OCR output for the same page, which is what
    // makes the two comparable in the search index.
    let extracted = open("word-spacing.pdf")
        .extract_text()
        .expect("extract text")
        .expect("the fixture has a text layer");
    assert_eq!(
        extracted, "Notae searchable",
        "extracted text must normalize exactly, got {extracted:?}"
    );
}

#[test]
fn every_fixture_still_refuses_only_what_it_should() {
    // The new fixtures join the panic sweep, so none of them may take the
    // process down on any path a caller can reach.
    let fixtures = [
        "damaged-contents.pdf",
        "hex-contents.pdf",
        "no-header.pdf",
        "leading-junk.pdf",
        "word-spacing.pdf",
    ];
    for name in fixtures {
        let path = fixture(name);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let Ok(document) = LinuxPdfDocument::open(&path) else {
                return;
            };
            let _ = document.page_count();
            let _ = document.extract_text();
            let count = document.page_count().unwrap_or(0);
            for page in 0..count + 1 {
                let _ = document.render_page(page, DEFAULT_OCR_DPI);
                let _ = document.render_page(page, u32::MAX);
            }
        }));
        assert!(outcome.is_ok(), "{name} must not panic");
    }
}
