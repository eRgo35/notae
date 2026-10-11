use notae_domain::IndexStatus;

use crate::pdf_intent::ContentUri;

/// PDF failures that can be attached to one library record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PdfError {
    /// The provider opened a document with no readable pages.
    #[error("PDF has no readable pages")]
    EmptyDocument,
    /// The Android PDF provider rejected an operation.
    #[error("PDF platform operation failed: {0}")]
    Platform(String),
}

/// Metadata gathered when a PDF is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfDocumentMetadata {
    /// Number of pages in the document.
    pub page_count: u32,
    /// Extracted text layer, when the platform API supports it and the PDF has text.
    pub text_layer: Option<String>,
}

/// Platform operations used by [`AndroidPdfDocument`].
///
/// Android implements this with `PdfRenderer`; host tests use a deterministic
/// fake so text normalization and typed failures remain testable.
pub trait PdfPageBackend: Send {
    /// Opens the document and extracts its text layer when supported.
    fn inspect(&mut self, uri: &ContentUri) -> Result<PdfDocumentMetadata, PdfError>;
}

/// A PDF document backed by Android's local PDF API or a host test backend.
pub struct AndroidPdfDocument {
    metadata: PdfDocumentMetadata,
}

impl AndroidPdfDocument {
    /// Opens a PDF through the supplied platform backend.
    pub fn open_with_backend(
        uri: ContentUri,
        mut backend: Box<dyn PdfPageBackend>,
    ) -> Result<Self, PdfError> {
        let mut metadata = backend.inspect(&uri)?;
        if metadata.page_count == 0 {
            return Err(PdfError::EmptyDocument);
        }
        metadata.text_layer = metadata
            .text_layer
            .take()
            .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|text| !text.is_empty());
        Ok(Self { metadata })
    }

    /// Opens a PDF using Android's `PdfRenderer` backend.
    #[cfg(target_os = "android")]
    pub fn open(app: android_activity::AndroidApp, uri: ContentUri) -> Result<Self, PdfError> {
        Self::open_with_backend(
            uri,
            Box::new(crate::android::pdf_pages::AndroidPdfPageBackend::new(app)),
        )
    }

    /// Extracted searchable text, if the platform exposed a non-empty text layer.
    #[must_use]
    pub fn extract_text(&self) -> Option<&str> {
        self.metadata.text_layer.as_deref()
    }
}

/// Result of deciding a record's OCR status after local text extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfTextOutcome {
    /// Status to persist for this PDF.
    pub status: IndexStatus,
    /// New text to store; `None` preserves existing OCR text.
    pub text: Option<String>,
    /// Extraction failure to show in the per-file operation report.
    pub failure: Option<PdfError>,
}

/// Maps local PDF text extraction to an index status without discarding sidecar OCR.
#[must_use]
pub fn classify_text_layer(
    extracted: Result<Option<String>, PdfError>,
    has_existing_searchable_text: bool,
) -> PdfTextOutcome {
    match extracted {
        Ok(Some(text)) => {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                return PdfTextOutcome {
                    status: IndexStatus::Ready,
                    text: Some(text),
                    failure: None,
                };
            }
            missing_text_outcome(has_existing_searchable_text)
        }
        Ok(None) => missing_text_outcome(has_existing_searchable_text),
        Err(error) => PdfTextOutcome {
            status: IndexStatus::OcrIncomplete,
            text: None,
            failure: Some(error),
        },
    }
}

fn missing_text_outcome(has_existing_searchable_text: bool) -> PdfTextOutcome {
    PdfTextOutcome {
        status: if has_existing_searchable_text {
            IndexStatus::Ready
        } else {
            IndexStatus::OcrPending
        },
        text: None,
        failure: None,
    }
}
