//! Text normalization helpers shared across the workspace.
//!
//! These helpers live in the domain crate so every other crate that
//! produces or stores text can agree on its shape. The OCR layer,
//! the indexer, and the metadata helpers all reduce to the same
//! canonical form so the sidecar, the search index, and the OCR
//! provider never disagree on what a piece of text "is".

/// Collapses internal whitespace runs to a single space and trims the
/// ends.
///
/// Mirrors the normalization the OCR layer applies to the text
/// Tesseract returns, so a piece of recognized text round-trips
/// through the sidecar, the search index, and the OCR provider
/// unchanged. Whitespace is whatever [`str::split_whitespace`] treats
/// as a boundary: ASCII spaces, tabs, newlines, carriage returns, and
/// form feeds. An all-whitespace input collapses to the empty string.
///
/// [`str::split_whitespace`]: str::split_whitespace
#[must_use]
pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
