//! Typed values for the text a screen shows.
//!
//! The core never builds display text. It returns these values, with their names, numbers and
//! ISO `YYYY-MM-DD` dates as fields, and each UI formats them in its own language.

#![forbid(unsafe_code)]

use crate::operations::events::RescanSummary;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// An operation failed. The reason is the error's own detail, not translated text.
    Failed {
        /// The error detail.
        reason: String,
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
