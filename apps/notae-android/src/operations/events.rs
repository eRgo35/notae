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
    /// Every PDF is processed; the worker is now looking for a desktop sidecar and, if one
    /// exists, comparing it with the index. There is no entry-level progress.
    CheckingDesktopImport,
    /// One PDF was reconciled successfully.
    FileSucceeded { path: String },
    /// One PDF could not be reconciled; other files continue independently.
    FileFailed { path: String, reason: String },
    /// All scan work, including the desktop sidecar check, has finished. This is the last
    /// event of a successful rescan.
    Completed {
        summary: RescanSummary,
        rows: Vec<SearchRow>,
        /// What the post-scan sidecar check found.
        desktop_import: DesktopImportCheck,
    },
    /// Cancellation was observed before any plan updates were applied.
    Canceled,
    /// A sidecar preview was computed without writing to the index.
    ImportPreviewReady { summary: ImportPreviewSummary },
    /// A confirmed sidecar import completed.
    ImportApplied {
        summary: ImportSummary,
        rows: Vec<SearchRow>,
    },
    /// A worker-side search completed.
    SearchRows(Vec<SearchRow>),
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

/// Outcome of the sidecar check that closes a rescan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopImportCheck {
    /// The library has no desktop sidecar.
    NoSidecar,
    /// A sidecar was compared read-only; the user decides whether to import it.
    PreviewReady(ImportPreviewSummary),
    /// The scan itself is committed but the sidecar could not be read or compared.
    Failed { reason: String },
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

/// Counts and per-entry details displayed in the read-only sidecar preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportPreviewSummary {
    pub new: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub missing: usize,
    pub mismatches: usize,
    pub unreadable: usize,
    pub conflicts: usize,
    pub ocr_available: usize,
    pub rows: Vec<ImportPreviewRow>,
}

/// One sidecar record and the local metadata it would affect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportPreviewRow {
    pub path: String,
    pub change: String,
    pub local_title: Option<String>,
    pub proposed_title: String,
    pub local_metadata: Option<String>,
    pub proposed_metadata: String,
    pub imported_fields: Vec<&'static str>,
    pub preserved_fields: Vec<&'static str>,
    pub ocr_available: bool,
    pub conflict: Option<String>,
}

/// Counts displayed after a confirmed sidecar import.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: usize,
    pub unchanged: usize,
    pub mismatches: usize,
    pub missing: usize,
    pub unreadable: usize,
    pub conflicts: usize,
    pub failures: usize,
}
