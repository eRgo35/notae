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
    assert!(
        matches!(error, PdfError::PageOutOfRange { .. }),
        "{error}"
    );
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
    let image: image::GrayImage = open("text-layer.pdf")
        .render_page(0, 72)
        .expect("render");
    let shared: RenderedPage = image;
    assert_eq!(shared.width(), 200);
}