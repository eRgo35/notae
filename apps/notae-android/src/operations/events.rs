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
    Indexing { total: usize },
    /// One catalogued PDF has finished embedded-text extraction and thumbnail rendering.
    ///
    /// Only valid after [`OperationEvent::RescanFinalizing`].
    Progress { completed: usize, total: usize },
    /// Reconciliation is committed; the per-file text and thumbnail pass over `total` PDFs
    /// starts now and cannot be canceled. Not sent for an empty library.
    RescanFinalizing { total: usize },
    /// One PDF was reconciled successfully.
    FileSucceeded { path: String },
    /// One PDF could not be reconciled; other files continue independently.
    FileFailed { path: String, reason: String },
    /// All scan work has finished. This is the last
    /// event of a successful rescan.
    Completed {
        summary: RescanSummary,
        rows: Vec<SearchRow>,
    },
    /// Cancellation was observed before any plan updates were applied.
    Canceled,
    /// A worker-side search completed.
    SearchRows(Vec<SearchRow>),
    /// The add picker's search for `query` completed.
    PickerRows { query: String, rows: Vec<SearchRow> },
    /// The worker loaded one note into the metadata editor.
    MetadataLoaded(EditableMetadata),
    /// A metadata edit committed and the library rows were refreshed.
    MetadataSaved { rows: Vec<SearchRow> },
    /// A metadata load or save was rejected without changing the index.
    MetadataFailed { reason: String },
    /// An operation failed before it could complete.
    Failed { reason: String },
    /// The complete library size, independent of the active query or scope.
    IndexedCount(usize),
}

/// Reconciliation counts shown after a scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RescanSummary {
    pub added: usize,
    pub updated: usize,
    pub moved: usize,
    pub deleted: usize,
    pub conflicts: usize,
    pub failures: usize,
}
