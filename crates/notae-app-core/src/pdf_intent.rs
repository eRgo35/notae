//! Handing a PDF to an external viewer: the content URI and the view-intent contract.

use std::fmt;

/// A SAF-backed content URI safe to pass to another Android activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentUri(String);

impl ContentUri {
    /// Accepts only a non-empty `content://` URI with an authority.
    pub fn new(value: impl Into<String>) -> Result<Self, ViewerError> {
        let value = value.into();
        let Some(authority_and_path) = value.strip_prefix("content://") else {
            return Err(ViewerError::InvalidContentUri);
        };
        let authority = authority_and_path.split('/').next().unwrap_or_default();
        if authority.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(ViewerError::InvalidContentUri);
        }
        Ok(Self(value))
    }

    /// Returns the URI string supplied by the SAF document provider.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The external activity chosen for a PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchResult {
    /// Samsung Notes resolved the PDF view intent and was preferred.
    SamsungNotes,
    /// No Samsung Notes handler resolved; Android presented its chooser.
    SystemChooser,
}

/// Failures while validating or launching an external PDF viewer.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ViewerError {
    /// Only SAF content URIs may cross the app boundary.
    #[error("external PDF access requires a SAF content URI")]
    InvalidContentUri,
    /// No installed application can view a PDF.
    #[error("no installed application can open PDFs")]
    NoCompatibleViewer,
    /// Android rejected the launch request.
    #[error("could not launch PDF viewer: {0}")]
    Platform(String),
}

/// Android view-intent data for handing a PDF to an external activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfIntentSpec {
    uri: ContentUri,
}

impl PdfIntentSpec {
    /// URI to attach as the intent's data.
    #[must_use]
    pub fn uri(&self) -> &ContentUri {
        &self.uri
    }

    /// MIME type used for PDF activity resolution.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        "application/pdf"
    }

    /// Android URI flags; read is granted without write access.
    #[must_use]
    pub const fn grant_flags(&self) -> i32 {
        0x0000_0001
    }
}

/// Builds the read-only PDF view-intent contract used by Android handoff.
#[must_use]
pub fn pdf_intent_spec(uri: ContentUri) -> PdfIntentSpec {
    PdfIntentSpec { uri }
}

/// Chooses Samsung Notes when it resolves the PDF intent, otherwise the system chooser.
pub const fn launch_result(
    samsung_notes_resolves: bool,
    compatible_viewers: usize,
) -> Result<LaunchResult, ViewerError> {
    if samsung_notes_resolves {
        Ok(LaunchResult::SamsungNotes)
    } else if compatible_viewers > 0 {
        Ok(LaunchResult::SystemChooser)
    } else {
        Err(ViewerError::NoCompatibleViewer)
    }
}
