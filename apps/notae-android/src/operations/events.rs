use crate::ui::metadata_editor::EditableMetadata;
use crate::ui::search::SearchRow;

/// Events emitted by background library operations.
#[derive(Debug)]
pub enum OperationEvent {
    /// A rescan has started; the total is unknown until the catalog finishes enumerating.
    Started,
    /// One catalogued PDF has been classified by the reconciliation plan.
    Progress { completed: usize, total: usize },
    /// One PDF was reconciled successfully.
    FileSucceeded { path: String },
    /// One PDF could not be reconciled; other files continue independently.
    FileFailed { path: String, reason: String },
    /// The scan and index update completed.
    Completed {
        summary: RescanSummary,
        rows: Vec<SearchRow>,
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
