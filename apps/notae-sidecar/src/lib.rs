//! Writes the `.notae/` sidecar (`library.json` and thumbnails) next to a score library.

mod build;
mod index_csv;
mod thumbnails;

use std::fmt;

pub use build::{Report, run};
pub use index_csv::{CsvRow, read_index};
pub use thumbnails::{Pdftoppm, Renderer};

/// A failure that stops the tool before it can report per-file problems.
#[derive(Debug)]
pub struct ToolError(String);

impl ToolError {
    pub(crate) fn new(context: &str, cause: impl fmt::Display) -> Self {
        Self(format!("{context}: {cause}"))
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolError {}
