//! The row every library list shows, and the errors library operations report.

use crate::text::RowStatus;
use notae_domain::{IndexStatus, IndexedNote, MetadataField, MetadataOrigin};
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
    /// The row's indexing state, or missing from the library.
    pub status: RowStatus,
    /// Library-relative path used to resolve the SAF document for PDF viewing.
    pub relative_path: String,
    /// Stable content key used by open and edit actions.
    pub note_key: String,
    /// Whether the PDF has no extractable text (OCR pending or incomplete). It stays in the
    /// library and is searchable by title and metadata; only the Settings count surfaces it.
    pub text_pending: bool,
    /// Where the title came from: the file name, the desktop, or an edit.
    pub title_origin: MetadataOrigin,
    /// Where the composer came from; [`MetadataOrigin::Unset`] when there is no composer.
    pub composer_origin: MetadataOrigin,
    /// FTS5 snippet of matching lyrics/page text, with `\u{2}`/`\u{3}` around the match.
    pub text_snippet: Option<String>,
    /// The set item this row shows; `None` outside a set.
    pub item_id: Option<String>,
    /// The item's one-based number in its set, counting notes; `None` outside a set.
    pub position: Option<usize>,
    /// A set's piece whose PDF is no longer in the library.
    pub missing: bool,
}

impl SearchRow {
    pub(crate) fn from_note(note: IndexedNote) -> Self {
        let metadata = note.metadata();
        let composer = metadata.composer_or_artist().map(str::to_owned);
        Self {
            id: note.id().map(|id| id.to_string()),
            title: note.title().to_owned(),
            composer_origin: if composer.is_some() {
                metadata.origin_of(MetadataField::ComposerOrArtist)
            } else {
                MetadataOrigin::Unset
            },
            composer_or_artist: composer,
            tags: metadata.tags().iter().cloned().collect(),
            status: row_status(note.status()),
            text_pending: matches!(
                note.status(),
                IndexStatus::OcrPending | IndexStatus::OcrIncomplete
            ),
            title_origin: metadata.origin_of(MetadataField::Title),
            note_key: note.fingerprint().sha256().to_string(),
            relative_path: note.path().as_str().to_owned(),
            text_snippet: None,
            item_id: None,
            position: None,
            missing: false,
        }
    }
}

const fn row_status(status: IndexStatus) -> RowStatus {
    match status {
        IndexStatus::Discovered => RowStatus::Discovered,
        IndexStatus::Indexing => RowStatus::Indexing,
        IndexStatus::Ready => RowStatus::Ready,
        IndexStatus::OcrPending | IndexStatus::OcrIncomplete => RowStatus::TextPending,
        IndexStatus::Stale => RowStatus::Stale,
        IndexStatus::DuplicateIdConflict => RowStatus::IdConflict,
        IndexStatus::Inaccessible => RowStatus::Unreadable,
        IndexStatus::InvalidPdf => RowStatus::InvalidPdf,
        IndexStatus::ImportMismatch => RowStatus::ImportMismatch,
    }
}

/// An error returned while projecting search results or applying a library operation.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// The local index could not produce the requested rows.
    #[error("library search failed: {0}")]
    Index(#[from] IndexError),
    /// A metadata value failed domain validation.
    #[error(transparent)]
    Domain(#[from] notae_domain::DomainError),
    /// The requested content key is not in the local index.
    #[error("indexed note not found: {0}")]
    NoteNotFound(String),
    /// Another index operation is already running.
    #[error("a rescan is already in progress")]
    OperationAlreadyInProgress,
    /// Cancellation was requested while no operation was active.
    #[error("no operation is in progress")]
    NoOperationInProgress,
    /// Reconciliation has committed and the remaining text extraction cannot be canceled.
    #[error("the rescan has already committed and can no longer be canceled")]
    RescanNotCancellable,
    /// The background operation thread has stopped.
    #[error("the index worker has stopped")]
    WorkerStopped,
    /// The background operation thread could not be created.
    #[error("failed to start the index worker: {0}")]
    WorkerStart(#[source] std::io::Error),
}

/// Splits an FTS5 snippet into (before, match, after) around its first marked match.
#[must_use]
pub fn split_snippet(snippet: &str) -> (String, String, String) {
    let clean = |text: &str| text.replace(['\u{2}', '\u{3}'], "");
    let Some((before, rest)) = snippet.split_once('\u{2}') else {
        return (clean(snippet), String::new(), String::new());
    };
    let (hit, after) = rest.split_once('\u{3}').unwrap_or((rest, ""));
    let before = clean(before);
    let count = before.chars().count();
    let before = if count > 48 {
        format!("…{}", before.chars().skip(count - 47).collect::<String>())
    } else {
        before
    };
    (before, hit.to_owned(), clean(after))
}
