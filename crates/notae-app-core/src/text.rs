//! Typed values for the text a screen shows.
//!
//! The core never builds display text. It returns these values, with their names, numbers and
//! ISO `YYYY-MM-DD` dates as fields, and each UI formats them in its own language.

#![forbid(unsafe_code)]

use notae_domain::DomainError;
use notae_index::{CatalogError, IndexError, ReconcileError};

use crate::operations::events::RescanSummary;
use crate::ui::search::UiError;

/// The large title above the score grid, and the side panel's labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionTitle {
    /// Every indexed score.
    All,
    /// Scores opened recently.
    Recent,
    /// Starred scores.
    Favorites,
    /// The list of singing sets; also the title of a set that no longer exists.
    Sets,
    /// One composer's scores, titled by the composer's name.
    Composer(String),
    /// An open set, titled by its name.
    Set(String),
}

/// An open set's subtitle: its date, then how many pieces and notes it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSubtitle {
    /// The set's ISO `YYYY-MM-DD` date, if it has one.
    pub date: Option<String>,
    /// Pieces in the set, counting each copy of a repeated piece.
    pub pieces: usize,
    /// Text notes in the set.
    pub notes: usize,
}

/// When a set was last edited, relative to now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edited {
    /// Earlier today (or, with a clock that went backwards, "in the future").
    Today,
    /// Yesterday.
    Yesterday,
    /// Two to six days ago.
    DaysAgo(u32),
    /// A week ago or more, on this ISO `YYYY-MM-DD` date.
    On(String),
}

/// A library row's state, shown in the details pane and on missing set pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RowStatus {
    /// Fully indexed.
    Ready,
    /// The PDF has no extractable text yet (OCR pending or incomplete).
    TextPending,
    /// A set's piece whose PDF is no longer in the library.
    MissingFromLibrary,
    /// The file could not be read.
    Unreadable,
    /// The file is not a readable PDF.
    InvalidPdf,
    /// Seen on disk but not yet processed.
    Discovered,
    /// Being fingerprinted and read.
    Indexing,
    /// The file changed since it was indexed.
    Stale,
    /// Two records claim the same note number.
    IdConflict,
    /// A record imported by an earlier version does not match the local file.
    ImportMismatch,
}

/// The operation banner's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusMessage {
    /// A rescan is enumerating the library; no total is known.
    ScanningLibrary,
    /// The index is being reconciled against `total` PDFs.
    Indexing {
        /// PDFs the catalog found.
        total: usize,
    },
    /// The per-file text and thumbnail pass is running.
    ReadingText {
        /// PDFs finished so far.
        completed: usize,
        /// PDFs in the pass.
        total: usize,
    },
    /// A rescan finished; the summary holds what it changed (all zero for no changes).
    Finished(RescanSummary),
    /// An operation failed.
    Failed {
        /// What failed.
        failure: Failure,
    },
}

/// Where a row sits among its section's rows: "2 of 5".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionOf {
    /// One-based index of the row.
    pub position: usize,
    /// Rows in the section.
    pub total: usize,
}

/// Why a library operation or a metadata edit failed: [`UiError`], with the domain, index and
/// catalog errors it wraps flattened into it. `detail` fields hold the platform's or the
/// database's own message.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Failure {
    /// The index refused a query or a write made for the library screens ([`UiError::Index`]).
    IndexQuery {
        /// What the index reported.
        index: IndexFailure,
    },
    /// The index refused the rescan's reconciliation ([`ReconcileError::Index`]).
    IndexUpdate {
        /// What the index reported.
        index: IndexFailure,
    },
    /// A note number was not a non-negative decimal integer.
    InvalidNoteId {
        /// The rejected text.
        value: String,
    },
    /// A title was empty after trimming.
    EmptyTitle,
    /// A content key was not 64 lowercase hex characters.
    InvalidSha256Digest,
    /// A PDF path was not a valid root-relative path.
    InvalidRelativePath {
        /// The rejected path.
        path: String,
    },
    /// A stored record had no metadata provenance.
    MissingProvenance,
    /// The content key is not in the index.
    NoteNotFound {
        /// The content key.
        key: String,
    },
    /// Another rescan is already running.
    OperationAlreadyInProgress,
    /// Cancel was asked for while nothing was running.
    NoOperationInProgress,
    /// The rescan has committed and can no longer be canceled.
    RescanNotCancellable,
    /// The background worker has stopped.
    WorkerStopped,
    /// The background worker could not be started.
    WorkerStart {
        /// The operating system's message.
        detail: String,
    },
    /// The library root could not be opened or traversed.
    LibraryUnreadable {
        /// The platform's message.
        detail: String,
    },
    /// The platform revoked the permission to read the library root.
    LibraryPermissionLost,
    /// Enumerating the library failed part way through.
    EnumerationFailed {
        /// The platform's message.
        detail: String,
    },
    /// The rescan's cancellation state closed unexpectedly (an internal error).
    CancellationStateClosed,
}

/// What the local index reported ([`IndexError`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum IndexFailure {
    /// The database rejected a statement or returned an unreadable row.
    Database {
        /// The database's message.
        detail: String,
    },
    /// The note number is already used by another record.
    DuplicateNoteId {
        /// The note number.
        id: u32,
    },
    /// The index was written by a newer Notae.
    UnsupportedSchemaVersion {
        /// The version found.
        found: u32,
        /// The newest version this build reads.
        supported: u32,
    },
    /// The index claims a version whose tables are not all present.
    IncompleteSchema {
        /// The claimed version.
        version: u32,
        /// The missing objects, comma-separated.
        missing: String,
    },
    /// No record has the content hash.
    UnknownNote {
        /// The content hash.
        hash: String,
    },
}

impl From<&IndexError> for IndexFailure {
    fn from(error: &IndexError) -> Self {
        match error {
            IndexError::Sqlite(error) => Self::Database {
                detail: error.to_string(),
            },
            IndexError::DuplicateNoteId(id) => Self::DuplicateNoteId { id: id.get() },
            IndexError::UnsupportedSchemaVersion { found, supported } => {
                Self::UnsupportedSchemaVersion {
                    found: *found,
                    supported: *supported,
                }
            }
            IndexError::IncompleteSchema { version, missing } => Self::IncompleteSchema {
                version: *version,
                missing: missing.clone(),
            },
            IndexError::UnknownNote { hash } => Self::UnknownNote {
                hash: hash.to_string(),
            },
        }
    }
}

impl From<&UiError> for Failure {
    fn from(error: &UiError) -> Self {
        match error {
            UiError::Index(error) => Self::IndexQuery {
                index: error.into(),
            },
            UiError::Domain(error) => error.into(),
            UiError::NoteNotFound(key) => Self::NoteNotFound { key: key.clone() },
            UiError::OperationAlreadyInProgress => Self::OperationAlreadyInProgress,
            UiError::NoOperationInProgress => Self::NoOperationInProgress,
            UiError::RescanNotCancellable => Self::RescanNotCancellable,
            UiError::WorkerStopped => Self::WorkerStopped,
            UiError::WorkerStart(error) => Self::WorkerStart {
                detail: error.to_string(),
            },
        }
    }
}

impl From<&DomainError> for Failure {
    fn from(error: &DomainError) -> Self {
        match error {
            DomainError::InvalidNoteId(value) => Self::InvalidNoteId {
                value: value.clone(),
            },
            DomainError::EmptyTitle => Self::EmptyTitle,
            DomainError::InvalidSha256Digest => Self::InvalidSha256Digest,
            DomainError::InvalidRelativePath(path) => {
                Self::InvalidRelativePath { path: path.clone() }
            }
            DomainError::MissingProvenance => Self::MissingProvenance,
        }
    }
}

impl From<&CatalogError> for Failure {
    fn from(error: &CatalogError) -> Self {
        match error {
            CatalogError::Unreadable(detail) => Self::LibraryUnreadable {
                detail: detail.clone(),
            },
            CatalogError::Failed(detail) => Self::EnumerationFailed {
                detail: detail.clone(),
            },
            CatalogError::PermissionLost => Self::LibraryPermissionLost,
        }
    }
}

impl From<&ReconcileError> for Failure {
    fn from(error: &ReconcileError) -> Self {
        match error {
            ReconcileError::Index(error) => Self::IndexUpdate {
                index: error.into(),
            },
            ReconcileError::Catalog(error) => error.into(),
        }
    }
}
