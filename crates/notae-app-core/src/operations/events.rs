//! What the worker reports back to the UI thread.

use crate::text::Failure;
use crate::ui::metadata_editor::EditableMetadata;
use crate::ui::search::SearchRow;

/// Events emitted by background library operations.
#[derive(Debug)]
pub enum OperationEvent {
    /// A rescan has started and the catalog is enumerating PDFs; the total is unknown.
    Started,
    /// Enumeration found `total` PDFs and the index is being reconciled against them.
    ///
    /// The total is known here, but reconciliation reports no per-file progress.
    Indexing {
        /// PDFs the catalog found.
        total: usize,
    },
    /// One catalogued PDF has finished embedded-text extraction and thumbnail rendering.
    ///
    /// Only valid after [`OperationEvent::RescanFinalizing`].
    Progress {
        /// PDFs finished so far.
        completed: usize,
        /// PDFs in the pass.
        total: usize,
    },
    /// Reconciliation is committed; the per-file text and thumbnail pass over `total` PDFs
    /// starts now and cannot be canceled. Not sent for an empty library.
    RescanFinalizing {
        /// PDFs in the pass.
        total: usize,
    },
    /// One PDF was reconciled successfully.
    FileSucceeded {
        /// The PDF's library-relative path.
        path: String,
    },
    /// One PDF could not be reconciled; other files continue independently.
    FileFailed {
        /// The PDF's library-relative path.
        path: String,
        /// Why it failed.
        reason: String,
    },
    /// All scan work has finished. This is the last
    /// event of a successful rescan.
    Completed {
        /// What the scan changed.
        summary: RescanSummary,
        /// The library rows after the scan.
        rows: Vec<SearchRow>,
    },
    /// Cancellation was observed before any plan updates were applied.
    Canceled,
    /// A worker-side search completed.
    SearchRows(Vec<SearchRow>),
    /// The add picker's search for `query` completed.
    PickerRows {
        /// The query the picker searched for.
        query: String,
        /// The matching rows.
        rows: Vec<SearchRow>,
    },
    /// The worker loaded one note into the metadata editor.
    MetadataLoaded(EditableMetadata),
    /// A metadata edit committed and the library rows were refreshed.
    MetadataSaved {
        /// The library rows after the edit.
        rows: Vec<SearchRow>,
    },
    /// A metadata load or save was rejected without changing the index.
    MetadataFailed {
        /// Why it was rejected.
        failure: Failure,
    },
    /// An operation failed before it could complete.
    Failed {
        /// Why it failed.
        failure: Failure,
    },
    /// The complete library size, independent of the active query or scope.
    IndexedCount(usize),
}

/// Reconciliation counts shown after a scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RescanSummary {
    /// PDFs new to the index.
    pub added: usize,
    /// PDFs whose content changed.
    pub updated: usize,
    /// PDFs found at a new path.
    pub moved: usize,
    /// PDFs no longer in the library.
    pub deleted: usize,
    /// PDFs whose note number clashes with another.
    pub conflicts: usize,
    /// PDFs that could not be read.
    pub failures: usize,
}
