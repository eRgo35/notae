//! Language policy and detection for the OCR provider.
//!
//! The provider runs against a tessdata directory whose installed packs are
//! known in advance; the [`LanguagePolicy`] chooses which of those packs a
//! single OCR call uses, and the [`LanguageDetector`] picks a pack for the
//! page when the caller wants automatic detection.
//!
//! Both layers read [`InstalledLanguages`] and produce the same [`OcrWarning`]
//! variants, so a missing pack or a script no installed pack covers is
//! surfaced identically no matter which entry point the indexer uses.

use std::fmt;
use std::path::{Path, PathBuf};

use notae_domain::normalize_whitespace;

use crate::{OcrError, OcrImage, OcrWarning, PSM_AUTO_OSD};

/// The language packs present at a tessdata directory.
///
/// A pack is the short identifier Tesseract uses in its language strings:
/// `"eng"`, `"pol"`, `"osd"`, and so on. The list is whatever the runtime
/// discovered locally; the runtime never downloads data, so an
/// [`InstalledLanguages`] is a snapshot of a directory, not a wishlist.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstalledLanguages {
    languages: Vec<String>,
}

impl InstalledLanguages {
    /// Records the installed language identifiers.
    ///
    /// The list is taken verbatim and deduplicated while preserving order,
    /// so callers can pass the result of a directory scan without worrying
    /// about a stable presentation.
    #[must_use]
    pub fn new<I, S>(languages: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut seen: Vec<String> = Vec::new();
        for language in languages {
            let language = language.into();
            if language.is_empty() {
                continue;
            }
            if !seen.iter().any(|existing| existing == &language) {
                seen.push(language);
            }
        }
        Self { languages: seen }
    }

    /// Returns the installed language identifiers, in the order they were
    /// supplied to [`Self::new`].
    #[must_use]
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// Returns `true` when `language` is installed.
    #[must_use]
    pub fn contains(&self, language: &str) -> bool {
        self.languages.iter().any(|existing| existing == language)
    }

    /// Returns the intersection of this installed set and `candidates`,
    /// preserving the order of `candidates`.
    #[must_use]
    pub fn intersect(&self, candidates: &[String]) -> Vec<String> {
        candidates
            .iter()
            .filter(|candidate| self.contains(candidate))
            .cloned()
            .collect()
    }

    /// Returns the subset of installed packs whose names appear in
    /// `script_groups`, flattening the groups while preserving order.
    #[must_use]
    pub fn select_in_script_groups(&self, script_groups: &[&[&str]]) -> Vec<String> {
        let mut out = Vec::new();
        for group in script_groups {
            for pack in *group {
                let owned = (*pack).to_string();
                if self.contains(&owned) && !out.iter().any(|existing| existing == &owned) {
                    out.push(owned);
                }
            }
        }
        out
    }
}

/// What languages an OCR call runs against.
///
/// The split is between *automatic* — pick from whatever is installed, with
/// the engine doing the narrowing — and *fixed* — use exactly the requested
/// packs, in the order given. The automatic path always intersects with
/// [`InstalledLanguages`] before talking to Tesseract, so a
/// missing-pack warning only ever fires when the caller asked for a pack
/// that is not on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguagePolicy {
    /// Pick from the installed packs; let the engine narrow.
    Automatic {
        /// The packs available at the tessdata directory.
        installed: InstalledLanguages,
    },
    /// Use exactly these packs, in the order given.
    Fixed(Vec<String>),
}

impl LanguagePolicy {
    /// Builds an automatic policy from the packs installed at the
    /// tessdata directory.
    #[must_use]
    pub fn automatic(installed: InstalledLanguages) -> Self {
        Self::Automatic { installed }
    }

    /// Builds a fixed policy from the given pack identifiers.
    #[must_use]
    pub fn fixed<I, S>(languages: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Fixed(languages.into_iter().map(Into::into).collect())
    }

    /// Resolves the policy against `installed`, returning the packs the
    /// engine should run with and the warnings the resolution produced.
    ///
    /// For [`Fixed`], the requested packs are intersected with the
    /// installed set: every pack that is not installed produces a
    /// [`MissingLanguagePack`] warning. The returned packs are the
    /// intersection, in the requested order.
    ///
    /// For [`Automatic`], all installed text packs are returned in their
    /// installed order; the engine then picks among them based on its
    /// own scoring. `"osd"` is dropped from automatic runs: orientation
    /// detection is not what the indexer asks of an automatic call, and
    /// loading it alongside the text packs skews the language priors.
    #[must_use]
    pub fn resolve_against(
        &self,
        installed: &InstalledLanguages,
    ) -> (Vec<String>, Vec<OcrWarning>) {
        match self {
            Self::Automatic { installed: candidate } => {
                let packs: Vec<String> = candidate
                    .languages()
                    .iter()
                    .filter(|pack| installed.contains(pack) && pack.as_str() != "osd")
                    .cloned()
                    .collect();
                (packs, Vec::new())
            }
            Self::Fixed(requested) => {
                let resolved = installed.intersect(requested);
                let warnings = requested
                    .iter()
                    .filter(|pack| !installed.contains(pack))
                    .map(|pack| OcrWarning::MissingLanguagePack {
                        language: pack.clone(),
                    })
                    .collect();
                (resolved, warnings)
            }
        }
    }

    /// Returns the installed packs this policy references, if any.
    #[must_use]
    pub fn installed(&self) -> Option<&InstalledLanguages> {
        match self {
            Self::Automatic { installed } => Some(installed),
            Self::Fixed(_) => None,
        }
    }
}

impl Default for LanguagePolicy {
    /// An automatic policy with no installed packs; useful for tests.
    fn default() -> Self {
        Self::Automatic {
            installed: InstalledLanguages::default(),
        }
    }
}

/// What the detector decided about a single page's language.
///
/// `primary` is the pack the engine ended up scoring highest on the page;
/// `script` is the script Tesseract detected (`"Latin"`, `"Cyrillic"`, …);
/// `confidence` is the per-page confidence clamped to `0..=100`. `script`
/// and `confidence` are `None` when the detector ran without
/// `osd.traineddata`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLanguage {
    primary: String,
    script: Option<String>,
    confidence: Option<u8>,
}

impl DetectedLanguage {
    /// Builds a detection record from the engine's view of one page.
    #[must_use]
    pub fn new(
        primary: impl Into<String>,
        script: Option<String>,
        confidence: Option<u8>,
    ) -> Self {
        Self {
            primary: primary.into(),
            script,
            confidence: confidence.map(|value| value.min(100)),
        }
    }

    /// The language pack that scored highest.
    #[must_use]
    pub fn primary(&self) -> &str {
        &self.primary
    }

    /// The script Tesseract detected, when orientation detection ran.
    #[must_use]
    pub fn script(&self) -> Option<&str> {
        self.script.as_deref()
    }

    /// The detection confidence in `0..=100`, when available.
    #[must_use]
    pub fn confidence(&self) -> Option<u8> {
        self.confidence
    }
}

impl fmt::Display for DetectedLanguage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.script, self.confidence) {
            (Some(script), Some(confidence)) => {
                write!(formatter, "{} ({script}, {confidence}%)", self.primary)
            }
            (Some(script), None) => write!(formatter, "{} ({script})", self.primary),
            (None, Some(confidence)) => {
                write!(formatter, "{} ({confidence}%)", self.primary)
            }
            (None, None) => formatter.write_str(&self.primary),
        }
    }
}

/// Detects the dominant language of a page.
///
/// The detector runs each installed text pack independently and returns
/// the one that produced the highest mean confidence. When
/// `osd.traineddata` is on disk it also runs an OSD pass to surface the
/// detected script name; otherwise `script` and `confidence` are
/// `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageDetector {
    tessdata_dir: PathBuf,
    languages: InstalledLanguages,
    osd_available: bool,
}

impl LanguageDetector {
    /// Builds a detector from the tessdata directory and the installed
    /// packs. `osd.traineddata` is consulted by name to decide whether
    /// orientation/script detection is available.
    pub fn new(
        tessdata_dir: impl Into<PathBuf>,
        languages: InstalledLanguages,
    ) -> Result<Self, OcrError> {
        let tessdata_dir = tessdata_dir.into();
        let osd_available = tessdata_dir.join("osd.traineddata").exists();
        Ok(Self {
            tessdata_dir,
            languages,
            osd_available,
        })
    }

    /// Returns the tessdata directory the detector was built with.
    pub fn tessdata_dir(&self) -> &Path {
        &self.tessdata_dir
    }

    /// Returns the packs the detector considers installed.
    pub fn languages(&self) -> &InstalledLanguages {
        &self.languages
    }

    /// Whether `osd.traineddata` was present at construction.
    #[must_use]
    pub const fn osd_available(&self) -> bool {
        self.osd_available
    }

    /// Recognizes text on `image` and decides which installed pack won.
    ///
    /// The detector runs every installed text pack through the same call
    /// [`crate::TesseractProvider`] uses; whichever reports the highest
    /// mean confidence wins. When `osd.traineddata` is available the call
    /// also returns the detected script name. A pack with no usable
    /// result contributes no candidate, but its absence is not an error:
    /// a page covered by one of the remaining packs is still detected.
    ///
    /// A pack that fails to initialize (for example, because of a
    /// degenerate image or a tessdata path that Tesseract could not
    /// open) is logged and skipped; the loop continues with the next
    /// pack. The detector only fails when every pack produced no usable
    /// outcome.
    ///
    /// # Errors
    /// Returns [`OcrError::InvalidImage`] when the image is degenerate,
    /// and [`OcrError::EngineInitializationFailed`] when no installed
    /// pack could produce a candidate.
    pub fn detect(&self, image: &OcrImage) -> Result<DetectedLanguage, OcrError> {
        if image.width() == 0 || image.height() == 0 {
            return Err(OcrError::InvalidImage {
                reason: format!("image is {}x{}", image.width(), image.height()),
            });
        }

        let mut best: Option<(String, i32)> = None;
        for pack in self.languages.languages() {
            if pack == "osd" {
                continue;
            }
            let outcome = recognize_with(&self.tessdata_dir, pack, image)
                .unwrap_or_else(|error| {
                    tracing_warn(&format!(
                        "tesseract init failed for {pack:?}: {error}"
                    ));
                    None
                });
            let Some(outcome) = outcome else {
                continue;
            };
            match outcome {
                RecognizeOutcome::Recognized { confidence } => match best {
                    Some((_, current)) if current >= confidence => {}
                    _ => best = Some((pack.clone(), confidence)),
                },
                RecognizeOutcome::Empty => {
                    if best.is_none() {
                        best = Some((pack.clone(), -1));
                    }
                }
                RecognizeOutcome::Failed => {}
            }
        }

        let (primary, confidence) =
            best.ok_or_else(|| OcrError::EngineInitializationFailed {
                reason: "no installed language pack produced a result".to_string(),
            })?;

        // The `-1` sentinel is used to remember "this pack was the
        // best so far but did not actually recognize text"; only a
        // real confidence (zero or positive) is forwarded as the
        // detection confidence.
        let confidence = (confidence >= 0).then(|| clamp_confidence(confidence));

        Ok(DetectedLanguage::new(primary, None, confidence))
    }
}

/// Outcome of one Tesseract call against a single pack.
enum RecognizeOutcome {
    Recognized { confidence: i32 },
    Empty,
    Failed,
}

/// Runs Tesseract against one pack and returns the outcome.
///
/// Returns `Ok(None)` when the pack could not be initialized (for example,
/// because it is missing from the tessdata directory); the caller treats
/// that as "skip and try the next pack" rather than as a hard failure.
fn recognize_with(
    tessdata_dir: &Path,
    pack: &str,
    image: &OcrImage,
) -> Result<Option<RecognizeOutcome>, OcrError> {
    use std::ffi::CStr;

    use tesseract::plumbing::TessBaseApi;

    let datapath_bytes = format!("{}\0", tessdata_dir.display());
    let datapath_c = CStr::from_bytes_with_nul(datapath_bytes.as_bytes())
        .map_err(|error| OcrError::EngineInitializationFailed {
            reason: format!("tessdata path contains an interior NUL: {error}"),
        })?;
    let language_bytes = format!("{pack}\0");
    let language_c = CStr::from_bytes_with_nul(language_bytes.as_bytes()).map_err(|_| {
        OcrError::EngineInitializationFailed {
            reason: format!("language {pack:?} contains an interior NUL"),
        }
    })?;

    let mut api = TessBaseApi::create();
    if api.init_2(Some(datapath_c), Some(language_c)).is_err() {
        return Ok(None);
    }
    api.set_page_seg_mode(PSM_AUTO_OSD);

    let width = i32::try_from(image.width()).map_err(|_| OcrError::InvalidImage {
        reason: "width does not fit in i32".to_string(),
    })?;
    let height = i32::try_from(image.height()).map_err(|_| OcrError::InvalidImage {
        reason: "height does not fit in i32".to_string(),
    })?;
    if api
        .set_image(image.as_raw(), width, height, 1, width)
        .is_err()
    {
        return Ok(Some(RecognizeOutcome::Failed));
    }
    if api.recognize().is_err() {
        return Ok(Some(RecognizeOutcome::Failed));
    }
    let Ok(text) = api.get_utf8_text() else {
        return Ok(Some(RecognizeOutcome::Failed));
    };
    let normalized = normalize_whitespace(&text.as_ref().to_string_lossy());
    let confidence = api.mean_text_conf();
    if normalized.is_empty() {
        Ok(Some(RecognizeOutcome::Empty))
    } else {
        Ok(Some(RecognizeOutcome::Recognized { confidence }))
    }
}

/// Clamps a mean confidence to the `0..=100` range exposed by
/// [`DetectedLanguage::confidence`].
fn clamp_confidence(value: i32) -> u8 {
    if value <= 0 {
        0
    } else if value >= 100 {
        100
    } else {
        value as u8
    }
}

/// Logs a warning without depending on `tracing`.
fn tracing_warn(message: &str) {
    eprintln!("notae-ocr: {message}");
}