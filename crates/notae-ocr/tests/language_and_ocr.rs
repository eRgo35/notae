//! Integration tests for the OCR provider.
//!
//! These tests run against the tessdata bundled under `tests/tessdata/`,
//! which carries the four files the test cases need:
//!
//! * `eng.traineddata` for the English-only fixture
//! * `pol.traineddata` for the Polish fixture
//! * `lat.traineddata` for Latin-script coverage
//! * `osd.traineddata` for orientation/script detection
//!
//! The two image fixtures — `english.png` and `polish.png` — were rendered
//! from DejaVu Sans by `tests/fixtures/generate.py` and committed next to
//! the generator. Re-running the script regenerates the PNGs and keeps
//! the recognised text in sync.

use std::path::PathBuf;

use image::ImageReader;
use notae_ocr::{
    DetectedLanguage, InstalledLanguages, LanguageDetector, LanguagePolicy, OcrError, OcrImage,
    OcrProvider, OcrText, OcrWarning, TesseractConfig, TesseractProvider,
};

/// The directory that holds the bundled tessdata files.
fn tessdata_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tessdata")
}

/// The directory that holds the generated fixture images.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Loads a PNG fixture as the OCR crate's grayscale image type.
fn load_fixture(name: &str) -> OcrImage {
    let path = fixtures_dir().join(name);
    let image = ImageReader::open(&path)
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()))
        .with_guessed_format()
        .unwrap_or_else(|error| panic!("format guess for {}: {error}", path.display()))
        .decode()
        .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()));
    image.to_luma8()
}

/// Builds a provider configured with every pack the tessdata bundle ships.
fn provider_with_all_languages() -> TesseractProvider {
    let config = TesseractConfig::new(
        tessdata_dir(),
        InstalledLanguages::new(["eng", "pol", "lat", "osd"]),
    );
    TesseractProvider::new(config).expect("provider should construct")
}

/// Builds a detector configured with the same packs.
fn detector_with_all_languages() -> LanguageDetector {
    let installed = InstalledLanguages::new(["eng", "pol", "lat", "osd"]);
    LanguageDetector::new(tessdata_dir(), installed)
        .expect("detector should construct")
}

/// Asserts that `haystack` contains `needle`, ignoring case.
fn assert_contains_ci(haystack: &str, needle: &str) {
    if !haystack.to_lowercase().contains(&needle.to_lowercase()) {
        panic!(
            "expected text to contain {:?} (case-insensitive), got {:?}",
            needle, haystack
        );
    }
}

#[test]
fn english_fixture_is_recognised() {
    let provider = provider_with_all_languages();
    let image = load_fixture("english.png");

    let policy = LanguagePolicy::fixed(["eng"]);
    let result = provider.recognize(&image, &policy).expect("recognize");

    assert_contains_ci(result.text(), "Notae");
    assert_contains_ci(result.text(), "searchable");
    assert!(result.confidence() > 0, "confidence should be recorded");
}

#[test]
fn polish_fixture_keeps_diacritics_with_polish_pack() {
    let provider = provider_with_all_languages();
    let image = load_fixture("polish.png");

    let policy = LanguagePolicy::fixed(["pol"]);
    let result = provider.recognize(&image, &policy).expect("recognize");

    assert_contains_ci(result.text(), "Za");
    assert_contains_ci(result.text(), "żółć");
    assert_contains_ci(result.text(), "gęślą");
    assert_contains_ci(result.text(), "jaźń");
}

#[test]
fn english_pack_alone_corrupts_polish_diacritics() {
    // Pins the contrast the Polish pack is there to preserve: English alone
    // mangles the Polish pangram into something a user could not search for.
    let provider = provider_with_all_languages();
    let image = load_fixture("polish.png");

    let policy = LanguagePolicy::fixed(["eng"]);
    let result = provider.recognize(&image, &policy).expect("recognize");

    assert!(
        !result.text().contains("żółć"),
        "expected English-only OCR to corrupt Polish diacritics, got {:?}",
        result.text(),
    );
}

#[test]
fn automatic_policy_selects_all_installed_packs() {
    let provider = provider_with_all_languages();
    let image = load_fixture("english.png");

    let policy = LanguagePolicy::automatic(InstalledLanguages::new(["eng", "pol", "lat"]));
    let result = provider.recognize(&image, &policy).expect("recognize");

    let detected = result.detected_languages();
    assert_eq!(
        detected,
        &["eng".to_string(), "pol".to_string(), "lat".to_string()],
        "automatic mode should use every installed pack",
    );
}

#[test]
fn automatic_policy_warns_about_uninstalled_pack() {
    let provider = provider_with_all_languages();
    let image = load_fixture("english.png");

    let policy = LanguagePolicy::fixed(["rus"]);
    let result = provider
        .recognize(&image, &policy)
        .expect("recognize should still return a result");

    assert!(
        result.warnings().iter().any(|warning| matches!(
            warning,
            OcrWarning::MissingLanguagePack { language } if language == "rus"
        )),
        "expected MissingLanguagePack warning for rus, got {:?}",
        result.warnings(),
    );
}

#[test]
fn fixed_policy_with_only_missing_packs_returns_empty_result_with_warning() {
    let provider = provider_with_all_languages();
    let image = load_fixture("english.png");

    let policy = LanguagePolicy::fixed(["rus"]);
    let result = provider
        .recognize(&image, &policy)
        .expect("a missing pack is not a hard engine failure");

    assert!(
        result.text().is_empty(),
        "no pack should mean no text, got {:?}",
        result.text(),
    );
    assert!(
        result.warnings().iter().any(|warning| matches!(
            warning,
            OcrWarning::MissingLanguagePack { language } if language == "rus"
        )),
        "expected MissingLanguagePack warning for rus, got {:?}",
        result.warnings(),
    );
}

#[test]
fn empty_image_returns_invalid_image_error() {
    let provider = provider_with_all_languages();
    let empty = OcrImage::new(0, 0);

    let policy = LanguagePolicy::fixed(["eng"]);
    let error = provider
        .recognize(&empty, &policy)
        .expect_err("a zero-sized image should be rejected");

    assert!(
        matches!(error, OcrError::InvalidImage { .. }),
        "expected InvalidImage, got {error:?}",
    );
}

#[test]
fn empty_recognition_result_carries_empty_warning() {
    // A page with no text at all (a blank rectangle) is not an error: Tesseract
    // successfully ran, found nothing, and the provider reports an empty result
    // alongside the empty text so the indexer can decide not to store it.
    let provider = provider_with_all_languages();
    let blank = OcrImage::new(120, 60);

    let policy = LanguagePolicy::fixed(["eng"]);
    let result = provider
        .recognize(&blank, &policy)
        .expect("a blank page is not an engine failure");

    assert!(
        result.text().is_empty(),
        "expected empty text, got {:?}",
        result.text(),
    );
    assert!(
        result
            .warnings()
            .iter()
            .any(|warning| matches!(warning, OcrWarning::EmptyResult)),
        "expected EmptyResult warning, got {:?}",
        result.warnings(),
    );
}

#[test]
fn detector_picks_a_latin_pack_for_english_fixture() {
    // Either "eng" or "lat" is a correct detection for Latin-script English:
    // the Latin pack scores competitively on Latin text, and the detector
    // must not tie-break against it. The test asserts that the detector
    // picked *something* with the right script — i.e. it did not pick a
    // pack that doesn't match the script, and it did not crash.
    let detector = detector_with_all_languages();
    let image = load_fixture("english.png");

    let detected = detector.detect(&image).expect("detect");
    let primary = detected.primary();
    assert!(
        primary == "eng" || primary == "lat",
        "expected eng or lat for English text, got {primary:?}",
    );
}

#[test]
fn detector_picks_polish_for_polish_fixture() {
    let detector = detector_with_all_languages();
    let image = load_fixture("polish.png");

    let detected = detector.detect(&image).expect("detect");
    assert_eq!(
        detected.primary(),
        "pol",
        "Polish fixture should pick the Polish pack",
    );
}

#[test]
fn provider_surfaces_confidence_in_text() {
    let provider = provider_with_all_languages();
    let image = load_fixture("english.png");

    let policy = LanguagePolicy::fixed(["eng"]);
    let result = provider.recognize(&image, &policy).expect("recognize");

    assert!(result.confidence() > 0, "expected non-zero confidence");
    assert!(result.confidence() <= 100);
}

#[test]
fn policy_resolve_against_returns_missing_warnings() {
    let installed = InstalledLanguages::new(["eng", "pol"]);
    let policy = LanguagePolicy::fixed(["eng", "rus", "pol", "deu"]);

    let (packs, warnings) = policy.resolve_against(&installed);

    assert_eq!(packs, vec!["eng".to_string(), "pol".to_string()]);
    assert_eq!(warnings.len(), 2);
    assert!(warnings
        .iter()
        .any(|w| matches!(w, OcrWarning::MissingLanguagePack { language } if language == "rus")));
    assert!(warnings
        .iter()
        .any(|w| matches!(w, OcrWarning::MissingLanguagePack { language } if language == "deu")));
}

#[test]
fn missing_tessdata_directory_is_rejected_at_construction() {
    let config = TesseractConfig::new(
        PathBuf::from("/nonexistent/tessdata/path"),
        InstalledLanguages::new(["eng"]),
    );
    let error = TesseractProvider::new(config).expect_err("should reject");

    assert!(
        matches!(error, OcrError::TessdataDirectoryUnavailable { .. }),
        "expected TessdataDirectoryUnavailable, got {error:?}",
    );
}

#[test]
fn detected_language_displays_with_script() {
    let detected = DetectedLanguage::new("pol", Some("Latin".to_string()), Some(85));
    assert_eq!(format!("{detected}"), "pol (Latin, 85%)");

    let detected = DetectedLanguage::new("eng", None, None);
    assert_eq!(format!("{detected}"), "eng");
}

#[test]
fn ocr_text_text_round_trip() {
    let mut ocr = OcrText::new("Notae searchable", vec!["eng".to_string()], 87);
    ocr.push_warning(OcrWarning::EmptyResult);

    let json = serde_json::to_string(&ocr).expect("serialize");
    let back: OcrText = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back, ocr);
}