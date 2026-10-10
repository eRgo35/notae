//! Local multilingual OCR provider for Notae, fronted by Tesseract.
//!
//! The provider layer exists so neither the indexer nor the Android UI depend
//! on the OCR engine. Every concrete engine — Tesseract here, the Android-side
//! adapter in the later plan — implements [`OcrProvider`], and the language
//! policy plus the recognized text cross the boundary through [`OcrText`] and
//! [`LanguagePolicy`].
//!
//! # Inputs and outputs
//!
//! An OCR call takes a rendered page image and a [`LanguagePolicy`], and
//! returns either [`OcrText`] or a typed [`OcrError`]. The image type is
//! [`OcrImage`], which is the same concrete type
//! [`notae_index_cli::image::RenderedPage`] resolves to — a rendered PDF page
//! flows in with no conversion. The policy decides which language packs the
//! engine runs against, and the result carries the recognized text, the
//! language(s) Tesseract reported, a confidence, and any warnings
//! ([`MissingLanguagePack`], [`EmptyResult`]) the run accumulated.
//!
//! [`MissingLanguagePack`]: OcrWarning::MissingLanguagePack
//! [`EmptyResult`]: OcrWarning::EmptyResult

mod image;
mod language;
mod tesseract;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use crate::image::OcrImage;
pub use crate::language::{
    DetectedLanguage, InstalledLanguages, LanguageDetector, LanguagePolicy,
};
pub use crate::tesseract::{TesseractConfig, TesseractProvider};

/// The text and metadata Tesseract recognized on one page.
///
/// `text` is the whitespace-normalized recognized string, exactly the form the
/// [`notae_index`](::notae_index) sidecar stores in `ocr_text`. The other
/// fields are what the OCR layer records alongside the text so a later stage
/// can rebuild the same sidecar row without re-running the engine.
///
/// `detected_languages` lists every language pack the engine ran against for
/// this page, with the primary pack first. For the [`Automatic`] policy this
/// is the intersection of the requested packs and the script Tesseract
/// detected; for a [`Fixed`] policy it is the requested packs in the order
/// they were given. `confidence` is the mean per-character confidence
/// Tesseract reported, clamped to `0..=100`. `warnings` collects the
/// non-fatal conditions the run observed; the recognized text is still
/// returned when a warning is present.
///
/// [`Automatic`]: LanguagePolicy::Automatic
/// [`Fixed`]: LanguagePolicy::Fixed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrText {
    text: String,
    detected_languages: Vec<String>,
    confidence: u8,
    warnings: Vec<OcrWarning>,
}

impl OcrText {
    /// Creates OCR text without any warnings.
    #[must_use]
    pub fn new(
        text: impl Into<String>,
        detected_languages: Vec<String>,
        confidence: u8,
    ) -> Self {
        Self {
            text: text.into(),
            detected_languages,
            confidence: confidence.min(100),
            warnings: Vec::new(),
        }
    }

    /// Appends a warning to the run.
    pub fn push_warning(&mut self, warning: OcrWarning) {
        self.warnings.push(warning);
    }

    /// Returns the normalized recognized text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the languages Tesseract ran against, primary first.
    #[must_use]
    pub fn detected_languages(&self) -> &[String] {
        &self.detected_languages
    }

    /// Returns the mean per-character confidence in `0..=100`.
    #[must_use]
    pub const fn confidence(&self) -> u8 {
        self.confidence
    }

    /// Returns the warnings the run collected.
    #[must_use]
    pub fn warnings(&self) -> &[OcrWarning] {
        &self.warnings
    }

    /// Consumes the value, returning the recognized text.
    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }
}

/// A non-fatal condition an OCR run observed.
///
/// The engine produced a result, but the result is incomplete or
/// questionable. The warnings are carried alongside the text so the caller
/// can decide what to do: an [`EmptyResult`] means the FTS index would gain
/// nothing by storing the text; a [`MissingLanguagePack`] tells the user
/// why their library is unsearchable for that page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OcrWarning {
    /// A requested language pack is not installed at the tessdata directory.
    MissingLanguagePack {
        /// The pack that was missing, e.g. `"pol"`.
        language: String,
    },
    /// Tesseract recognized nothing on the page.
    EmptyResult,
}

impl fmt::Display for OcrWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLanguagePack { language } => {
                write!(formatter, "missing language pack: {language}")
            }
            Self::EmptyResult => formatter.write_str("OCR produced no text"),
        }
    }
}

/// Failures produced by the OCR layer.
///
/// A failure aborts one call; the next page or the next PDF can still be
/// processed. The pipeline records these per file, the same way it records
/// a corrupt PDF.
#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    /// The tessdata directory does not exist or cannot be read.
    #[error("tessdata directory is unavailable: {path}")]
    TessdataDirectoryUnavailable {
        /// The directory the provider was configured with.
        path: std::path::PathBuf,
    },
    /// Tesseract refused to initialize with the supplied configuration.
    #[error("tesseract failed to initialize: {reason}")]
    EngineInitializationFailed {
        /// Why Tesseract refused to initialize.
        reason: String,
    },
    /// Recognition failed after initialization.
    #[error("tesseract recognition failed: {reason}")]
    RecognitionFailed {
        /// Why recognition failed.
        reason: String,
    },
    /// The supplied image has a degenerate shape.
    #[error("image is not usable for OCR: {reason}")]
    InvalidImage {
        /// What is wrong with the image.
        reason: String,
    },
}

/// The contract every OCR engine implements.
///
/// The provider does not own the tessdata directory or the language list: a
/// caller configures it once via its concrete type and uses it for every
/// page. The policy argument is per-call so an indexer can run automatic
/// detection on one PDF and force a fixed pack on another without
/// reconfiguring the engine.
pub trait OcrProvider {
    /// Recognizes text on one page under the given language policy.
    ///
    /// # Errors
    /// Returns [`OcrError`] when the engine cannot produce a result — a
    /// missing tessdata directory, a misconfigured language list, or a
    /// degenerate image. Non-fatal conditions such as an unsupported script
    /// or an empty result are returned as [`OcrWarning`]s inside the
    /// successful [`OcrText`].
    fn recognize(
        &self,
        page: &OcrImage,
        policy: &LanguagePolicy,
    ) -> Result<OcrText, OcrError>;
}

/// Page-segmentation mode that runs OCR with orientation/script detection.
///
/// The constant is `TessPageSegMode::PSM_AUTO_OSD` from `tesseract_sys`
/// (value `1`); we keep it as a `u32` so neither the language layer nor
/// the engine layer depends on the engine crate's re-exports.
pub(crate) const PSM_AUTO_OSD: u32 = 1;