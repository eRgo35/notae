use notae_domain::{IndexStatus, IndexedNote};
use notae_index::IndexError;

/// One library record projected for search and display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRow {
    /// Note number, when one is assigned.
    pub id: Option<String>,
    /// Display title.
    pub title: String,
    /// Composer or artist, when known.
    pub composer_or_artist: Option<String>,
    /// Normalized tags in their stable order.
    pub tags: Vec<String>,
    /// User-facing indexing/OCR status.
    pub status: String,
    /// Stable content key used by open and edit actions.
    pub note_key: String,
}

impl SearchRow {
    pub(crate) fn from_note(note: IndexedNote) -> Self {
        let metadata = note.metadata();
        Self {
            id: note.id().map(|id| id.to_string()),
            title: note.title().to_owned(),
            composer_or_artist: metadata.composer_or_artist().map(str::to_owned),
            tags: metadata.tags().iter().cloned().collect(),
            status: status_label(note.status()).to_owned(),
            note_key: note.fingerprint().sha256().to_string(),
        }
    }
}

#[cfg(target_os = "android")]
impl From<SearchRow> for crate::LibraryRow {
    fn from(row: SearchRow) -> Self {
        Self {
            id: row.id.unwrap_or_default().into(),
            title: row.title.into(),
            composer_or_artist: row.composer_or_artist.unwrap_or_default().into(),
            tags: row.tags.join(", ").into(),
            status: row.status.into(),
            note_key: row.note_key.into(),
        }
    }
}

fn status_label(status: IndexStatus) -> &'static str {
    match status {
        IndexStatus::Discovered => "Discovered",
        IndexStatus::Indexing => "Indexing",
        IndexStatus::Ready => "Ready",
        IndexStatus::OcrPending => "OCR pending",
        IndexStatus::OcrIncomplete => "OCR incomplete",
        IndexStatus::Stale => "Stale",
        IndexStatus::DuplicateIdConflict => "ID conflict",
        IndexStatus::Inaccessible => "Inaccessible",
        IndexStatus::InvalidPdf => "Invalid PDF",
        IndexStatus::ImportMismatch => "Import mismatch",
    }
}

/// An error returned while projecting search results for the library UI.
/// An error returned while projecting search results or applying a library operation.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// The local index could not produce the requested rows.
    #[error("library search failed: {0}")]
    Index(#[from] IndexError),
    /// A sidecar preview could not be read or applied.
    #[error(transparent)]
    Import(#[from] notae_index::SidecarError),
    /// The user has not confirmed the visible import preview.
    #[error("sidecar import requires explicit confirmation")]
    ConfirmationRequired,
    /// No preview is available to apply.
    #[error("there is no sidecar preview to apply")]
    NoImportPreview,
    /// Another index operation is already running.
    #[error("a rescan or import is already in progress")]
    OperationAlreadyInProgress,
    /// Cancellation was requested while no operation was active.
    #[error("no operation is in progress")]
    NoOperationInProgress,
    /// The background operation thread has stopped.
    #[error("the index worker has stopped")]
    WorkerStopped,
}
