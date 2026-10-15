//! Index lifecycle status for a discovered PDF.

use serde::{Deserialize, Serialize};

/// Where a record sits in the indexing pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    /// Seen on disk but not yet processed.
    Discovered,
    /// Currently being fingerprinted and read.
    Indexing,
    /// Fully indexed.
    Ready,
    /// OCR text extraction has not finished.
    OcrPending,
    /// OCR ran but produced no usable text.
    OcrIncomplete,
    /// The file changed since it was indexed.
    Stale,
    /// Two records claim the same note ID.
    DuplicateIdConflict,
    /// The file could not be read.
    Inaccessible,
    /// The file is not a readable PDF.
    InvalidPdf,
    /// An imported sidecar record does not match the local file.
    ImportMismatch,
}

impl Default for IndexStatus {
    /// A file that has been seen on disk and not yet read, which is the only
    /// state a record can be in before the pipeline has observed it.
    fn default() -> Self {
        Self::Discovered
    }
}
