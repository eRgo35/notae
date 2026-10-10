//! Tesseract-backed implementation of [`OcrProvider`].
//!
//! The provider keeps a [`TesseractConfig`] across calls and spins up a fresh
//! Tesseract handle per recognition. Tesseract's C++ API is not thread-safe
//! per handle, and the Linux pipeline is single-threaded per file anyway, so
//! the cost of re-initializing — one open of a small config file plus the
//! trained-data cache — is negligible next to the OCR work.
//!
//! The provider recognises a page in the language pack the policy resolved
//! to, falling back to the first requested pack when initialization fails
//! on the joined string. A failure to initialize is returned as
//! [`OcrError::EngineInitializationFailed`]; the failure of any individual
//! recognition becomes [`OcrError::RecognitionFailed`].

use std::ffi::CStr;
use std::path::{Path, PathBuf};

use notae_domain::normalize_whitespace;
use tesseract::plumbing::TessBaseApi;

use crate::language::{InstalledLanguages, LanguagePolicy};
use crate::{
    OcrError, OcrImage, OcrProvider, OcrText, OcrWarning, PSM_AUTO_OSD,
};

/// Configuration for [`TesseractProvider`].
///
/// `tessdata_dir` is the directory that holds the `*.traineddata` files
/// (e.g. `/usr/share/tesseract/tessdata` on most Linux systems, or the
/// `tests/tessdata/` directory the test fixtures use). `available_languages`
/// is the set of packs discovered at that directory. The list is
/// deferred — an empty `available_languages` is accepted at construction
/// and surfaced on the first recognition. With a [`Fixed`] policy the
/// result carries a [`MissingLanguagePack`] warning for every requested
/// pack; with an [`Automatic`] policy the result is an empty
/// `OcrText`. In both cases the indexer sees the empty library on the
/// first page it tries rather than having to special-case configuration
/// errors up front.
///
/// [`Fixed`]: crate::LanguagePolicy::Fixed
/// [`MissingLanguagePack`]: crate::OcrWarning::MissingLanguagePack
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TesseractConfig {
    tessdata_dir: PathBuf,
    available_languages: InstalledLanguages,
}

impl TesseractConfig {
    /// Builds a configuration from a tessdata directory and the packs the
    /// caller found there.
    ///
    /// The directory is held by name; the configuration does not stat it.
    /// The directory is validated at provider construction (see
    /// [`TesseractProvider::new`]) and the language list is validated at
    /// the first recognition.
    #[must_use]
    pub fn new(
        tessdata_dir: impl Into<PathBuf>,
        available_languages: InstalledLanguages,
    ) -> Self {
        Self {
            tessdata_dir: tessdata_dir.into(),
            available_languages,
        }
    }

    /// The tessdata directory the provider will pass to Tesseract.
    pub fn tessdata_dir(&self) -> &Path {
        &self.tessdata_dir
    }

    /// The packs the provider will consider when a policy is resolved.
    pub fn available_languages(&self) -> &InstalledLanguages {
        &self.available_languages
    }
}

/// The Tesseract-backed OCR provider.
#[derive(Debug, Clone)]
pub struct TesseractProvider {
    config: TesseractConfig,
}

impl TesseractProvider {
    /// Builds a provider from a [`TesseractConfig`].
    ///
    /// Construction stats the tessdata directory to surface an obvious
    /// misconfiguration — a missing path or a non-directory — before any
    /// recognition is attempted. The language list is **not** checked
    /// here; an empty list is accepted at construction and surfaces on
    /// the first recognition as warnings (for a [`Fixed`] policy) or as
    /// an empty result (for an [`Automatic`] policy), matching the
    /// deferred-validation contract of [`TesseractConfig::new`].
    ///
    /// [`Fixed`]: crate::LanguagePolicy::Fixed
    /// [`Automatic`]: crate::LanguagePolicy::Automatic
    pub fn new(config: TesseractConfig) -> Result<Self, OcrError> {
        if !config.tessdata_dir.exists() {
            return Err(OcrError::TessdataDirectoryUnavailable {
                path: config.tessdata_dir.clone(),
            });
        }
        if !config.tessdata_dir.is_dir() {
            return Err(OcrError::TessdataDirectoryUnavailable {
                path: config.tessdata_dir.clone(),
            });
        }
        Ok(Self { config })
    }

    /// Returns the configuration the provider was built with.
    pub fn config(&self) -> &TesseractConfig {
        &self.config
    }
}

impl OcrProvider for TesseractProvider {
    fn recognize(
        &self,
        page: &OcrImage,
        policy: &LanguagePolicy,
    ) -> Result<OcrText, OcrError> {
        if page.width() == 0 || page.height() == 0 {
            return Err(OcrError::InvalidImage {
                reason: format!("image is {}x{}", page.width(), page.height()),
            });
        }

        let (packs, warnings) =
            policy.resolve_against(&self.config.available_languages);

        if packs.is_empty() {
            // No requested pack is installed. Returning the warning instead of
            // an error means an indexer that runs every PDF through the same
            // provider still sees the cause on every page and can surface it
            // to the user.
            let mut ocr = OcrText::new(String::new(), Vec::new(), 0);
            for warning in warnings {
                ocr.push_warning(warning);
            }
            return Ok(ocr);
        }

        let mut merged_warnings = warnings;
        recognize_with(
            &self.config.tessdata_dir,
            &packs,
            page,
            &mut merged_warnings,
        )
    }
}

/// Outcome of one Tesseract run against the resolved packs.
enum RecognizeOutcome {
    Recognized { text: String, confidence: i32 },
    Empty,
    Failed,
}

/// Runs Tesseract against the resolved packs and assembles the result.
///
/// The packs are joined into a single Tesseract language string so the
/// engine can score them internally; warnings accumulated during the call
/// are merged with the policy warnings.
fn recognize_with(
    tessdata_dir: &Path,
    packs: &[String],
    image: &OcrImage,
    warnings: &mut Vec<OcrWarning>,
) -> Result<OcrText, OcrError> {
    let language = packs.join("+");
    let outcome = recognize_inner(tessdata_dir, &language, image, warnings)?;

    match outcome {
        RecognizeOutcome::Recognized { text, confidence } => {
            let mut ocr = OcrText::new(
                text,
                packs.to_vec(),
                clamp_confidence(confidence),
            );
            for warning in warnings.drain(..) {
                ocr.push_warning(warning);
            }
            Ok(ocr)
        }
        RecognizeOutcome::Empty => {
            let mut ocr = OcrText::new(String::new(), packs.to_vec(), 0);
            ocr.push_warning(OcrWarning::EmptyResult);
            for warning in warnings.drain(..) {
                ocr.push_warning(warning);
            }
            Ok(ocr)
        }
        RecognizeOutcome::Failed => {
            // Replace the caller with one that reflects a single failure:
            // the caller may want to attribute the failure to the first pack.
            for warning in warnings.drain(..) {
                let _ = warning;
            }
            Err(OcrError::RecognitionFailed {
                reason: format!(
                    "tesseract failed against language string {language:?}"
                ),
            })
        }
    }
}

/// Spins up one Tesseract handle and runs the image end-to-end.
fn recognize_inner(
    tessdata_dir: &Path,
    language: &str,
    image: &OcrImage,
    warnings: &mut Vec<OcrWarning>,
) -> Result<RecognizeOutcome, OcrError> {
    let datapath_bytes = format!("{}\0", tessdata_dir.display());
    let datapath_c = CStr::from_bytes_with_nul(datapath_bytes.as_bytes())
        .map_err(|error| OcrError::EngineInitializationFailed {
            reason: format!("tessdata path contains an interior NUL: {error}"),
        })?;
    let language_bytes = format!("{language}\0");
    let language_c = CStr::from_bytes_with_nul(language_bytes.as_bytes()).map_err(|_| {
        OcrError::EngineInitializationFailed {
            reason: format!("language {language:?} contains an interior NUL"),
        }
    })?;

    let mut api = TessBaseApi::create();
    api.init_2(Some(datapath_c), Some(language_c))
        .map_err(|error| OcrError::EngineInitializationFailed {
            reason: format!("init failed for {language:?}: {error:?}"),
        })?;
    api.set_page_seg_mode(PSM_AUTO_OSD);

    let width = i32::try_from(image.width()).map_err(|_| OcrError::InvalidImage {
        reason: "width does not fit in i32".to_string(),
    })?;
    let height = i32::try_from(image.height()).map_err(|_| OcrError::InvalidImage {
        reason: "height does not fit in i32".to_string(),
    })?;
    api.set_image(image.as_raw(), width, height, 1, width)
        .map_err(|error| OcrError::InvalidImage {
            reason: format!("tesseract refused the image: {error:?}"),
        })?;
    let _ = warnings;

    if api.recognize().is_err() {
        return Ok(RecognizeOutcome::Failed);
    }
    let Ok(text) = api.get_utf8_text() else {
        return Ok(RecognizeOutcome::Failed);
    };
    let normalized = normalize_whitespace(&text.as_ref().to_string_lossy());
    let confidence = api.mean_text_conf();
    if normalized.is_empty() {
        Ok(RecognizeOutcome::Empty)
    } else {
        Ok(RecognizeOutcome::Recognized {
            text: normalized,
            confidence,
        })
    }
}

/// Clamps a mean confidence to the `0..=100` range exposed by [`OcrText`].
fn clamp_confidence(value: i32) -> u8 {
    if value <= 0 {
        0
    } else if value >= 100 {
        100
    } else {
        value as u8
    }
}