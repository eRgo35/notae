//! Note metadata, provenance, and typed domain errors.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::NoteId;

/// Typed failures produced by domain validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    /// A note ID string was not a non-negative decimal integer.
    #[error("invalid note id: {0}")]
    InvalidNoteId(String),
    /// A title was empty after trimming.
    #[error("title must not be empty")]
    EmptyTitle,
    /// A SHA-256 digest was not 64 lowercase hex characters.
    #[error("invalid sha256 digest")]
    InvalidSha256Digest,
    /// A PDF path was not a valid root-relative path.
    #[error("invalid root-relative path: {0}")]
    InvalidRelativePath(String),
}

/// Where a metadata field's value came from.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MetadataOrigin {
    /// Parsed from the filename convention `<ID> - <Title>.pdf`.
    FilenameDerived,
    /// Imported from a Notae sidecar written on PC.
    PcImported,
    /// Edited by the user; authoritative over imported values.
    Manual,
    /// Not yet known.
    #[default]
    Unset,
}

/// The user-visible metadata for one note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMetadata {
    /// Optional globally unique non-negative number.
    pub id: Option<NoteId>,
    /// Required non-empty title.
    pub title: String,
    /// Optional composer or artist.
    #[serde(default)]
    pub composer_or_artist: Option<String>,
    /// Normalized, sorted, deduplicated tags.
    #[serde(default)]
    pub tags: BTreeSet<String>,
    /// Provenance of the metadata as a whole.
    #[serde(default)]
    pub origin: MetadataOrigin,
}

impl NoteMetadata {
    /// Builds metadata, trimming the title and normalizing tags.
    ///
    /// # Errors
    /// Returns [`DomainError::EmptyTitle`] when the trimmed title is empty.
    pub fn new(
        title: &str,
        composer_or_artist: Option<String>,
        tags: impl IntoIterator<Item = String>,
    ) -> Result<Self, DomainError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(DomainError::EmptyTitle);
        }
        Ok(Self {
            id: None,
            title: title.to_string(),
            composer_or_artist,
            tags: normalize_tags(tags),
            origin: MetadataOrigin::Unset,
        })
    }

    /// Adds a tag, applying the same normalization as the constructor.
    pub fn add_tag(&mut self, tag: &str) {
        if let Some(normalized) = normalize_tag(tag) {
            self.tags.insert(normalized);
        }
    }
}

fn normalize_tags(tags: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    tags.into_iter().filter_map(|tag| normalize_tag(&tag)).collect()
}

fn normalize_tag(tag: &str) -> Option<String> {
    let collapsed = tag.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
}