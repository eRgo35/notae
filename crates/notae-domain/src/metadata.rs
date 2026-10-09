//! Note metadata, per-field provenance, and typed domain errors.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::NoteId;
use thiserror::Error;

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

/// One editable field whose provenance is tracked independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataField {
    /// The numeric note ID.
    Id,
    /// The required title.
    Title,
    /// The optional composer or artist.
    ComposerOrArtist,
    /// The normalized tag set.
    Tags,
}

impl MetadataField {
    /// Every tracked field, in display order.
    pub const ALL: [Self; 4] = [
        Self::Id,
        Self::Title,
        Self::ComposerOrArtist,
        Self::Tags,
    ];
}

/// The [`MetadataOrigin`] of each editable field.
///
/// Per-field provenance is what lets a PC import replace only the fields the
/// user has not edited: importing a title must not silently overwrite a
/// manually edited composer, and vice versa.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldProvenance {
    /// Provenance of the note ID.
    #[serde(default)]
    id: MetadataOrigin,
    /// Provenance of the title.
    #[serde(default)]
    title: MetadataOrigin,
    /// Provenance of the composer or artist.
    #[serde(default)]
    composer_or_artist: MetadataOrigin,
    /// Provenance of the tags.
    #[serde(default)]
    tags: MetadataOrigin,
}

impl FieldProvenance {
    /// Returns provenance where every field is [`MetadataOrigin::Unset`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            id: MetadataOrigin::Unset,
            title: MetadataOrigin::Unset,
            composer_or_artist: MetadataOrigin::Unset,
            tags: MetadataOrigin::Unset,
        }
    }

    /// Returns the origin of one field.
    #[must_use]
    pub const fn origin_of(&self, field: MetadataField) -> MetadataOrigin {
        match field {
            MetadataField::Id => self.id,
            MetadataField::Title => self.title,
            MetadataField::ComposerOrArtist => self.composer_or_artist,
            MetadataField::Tags => self.tags,
        }
    }

    /// Records the origin of one field.
    pub fn set_origin(&mut self, field: MetadataField, origin: MetadataOrigin) {
        let slot = match field {
            MetadataField::Id => &mut self.id,
            MetadataField::Title => &mut self.title,
            MetadataField::ComposerOrArtist => &mut self.composer_or_artist,
            MetadataField::Tags => &mut self.tags,
        };
        *slot = origin;
    }

    /// Marks one field as edited by the user.
    pub fn mark_manual(&mut self, field: MetadataField) {
        self.set_origin(field, MetadataOrigin::Manual);
    }

    /// Returns `true` when the user has edited this field.
    #[must_use]
    pub fn is_manual(&self, field: MetadataField) -> bool {
        self.origin_of(field) == MetadataOrigin::Manual
    }
}

/// The user-visible metadata for one note.
///
/// Fields are private so every value reaching a `NoteMetadata` has passed
/// [`NoteMetadata::new`] or a validating mutator, whether it came from the
/// filename parser, SQLite, or a sidecar file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteMetadata {
    /// Optional globally unique non-negative number.
    id: Option<NoteId>,
    /// Required non-empty title.
    title: String,
    /// Optional composer or artist.
    composer_or_artist: Option<String>,
    /// Normalized, sorted, deduplicated tags.
    tags: BTreeSet<String>,
    /// Provenance of each field.
    provenance: FieldProvenance,
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
            provenance: FieldProvenance::new(),
        })
    }

    /// Returns the note ID, when one is known.
    #[must_use]
    pub const fn id(&self) -> Option<NoteId> {
        self.id
    }

    /// Returns the title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the composer or artist.
    #[must_use]
    pub fn composer_or_artist(&self) -> Option<&str> {
        self.composer_or_artist.as_deref()
    }

    /// Returns the normalized tags.
    #[must_use]
    pub const fn tags(&self) -> &BTreeSet<String> {
        &self.tags
    }

    /// Returns the per-field provenance.
    #[must_use]
    pub const fn provenance(&self) -> &FieldProvenance {
        &self.provenance
    }

    /// Returns the origin of one field.
    #[must_use]
    pub const fn origin_of(&self, field: MetadataField) -> MetadataOrigin {
        self.provenance.origin_of(field)
    }

    /// Records the origin of one field, for example after a PC import.
    pub fn set_origin(&mut self, field: MetadataField, origin: MetadataOrigin) {
        self.provenance.set_origin(field, origin);
    }

    /// Marks one field as edited by the user.
    pub fn mark_manual(&mut self, field: MetadataField) {
        self.provenance.mark_manual(field);
    }

    /// Replaces the note ID, keeping the ID's provenance unchanged so that
    /// filename-derived IDs are not mistaken for manual edits.
    pub fn set_id(&mut self, id: Option<NoteId>) {
        self.id = id;
    }

    /// Replaces the title, marking the title field as
    /// [`MetadataOrigin::Manual`].
    ///
    /// # Errors
    /// Returns [`DomainError::EmptyTitle`] when the trimmed title is empty.
    pub fn set_title(&mut self, title: &str) -> Result<(), DomainError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(DomainError::EmptyTitle);
        }
        self.title.clear();
        self.title.push_str(title);
        self.mark_manual(MetadataField::Title);
        Ok(())
    }

    /// Replaces the composer or artist.
    ///
    /// Passing `None` clears the field and clears its manual marker, because a
    /// cleared field holds no user intent to preserve.
    pub fn set_composer_or_artist(&mut self, composer_or_artist: Option<String>) {
        self.composer_or_artist = composer_or_artist;
        if self.composer_or_artist.is_some() {
            self.mark_manual(MetadataField::ComposerOrArtist);
        } else {
            self.set_origin(MetadataField::ComposerOrArtist, MetadataOrigin::Unset);
        }
    }

    /// Replaces the whole tag set, marking the tags field as
    /// [`MetadataOrigin::Manual`].
    pub fn set_tags(&mut self, tags: impl IntoIterator<Item = String>) {
        self.tags = normalize_tags(tags);
        self.mark_manual(MetadataField::Tags);
    }

    /// Adds a tag, applying the same normalization as the constructor and
    /// marking the tags field as [`MetadataOrigin::Manual`].
    pub fn add_tag(&mut self, tag: &str) {
        if let Some(normalized) = normalize_tag(tag) {
            self.tags.insert(normalized);
            self.mark_manual(MetadataField::Tags);
        }
    }

    /// Removes a tag, marking the tags field as [`MetadataOrigin::Manual`].
    pub fn remove_tag(&mut self, tag: &str) {
        if let Some(normalized) = normalize_tag(tag) {
            self.tags.remove(&normalized);
            self.mark_manual(MetadataField::Tags);
        }
    }
}

impl<'de> Deserialize<'de> for NoteMetadata {
    /// Routes every inbound record through [`NoteMetadata::new`], so a sidecar
    /// cannot produce metadata that the constructor would have rejected.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            id: Option<NoteId>,
            title: String,
            #[serde(default)]
            composer_or_artist: Option<String>,
            #[serde(default)]
            tags: BTreeSet<String>,
            #[serde(default)]
            provenance: FieldProvenance,
        }

        let raw = Raw::deserialize(deserializer)?;
        let mut meta = NoteMetadata::new(&raw.title, raw.composer_or_artist, raw.tags)
            .map_err(serde::de::Error::custom)?;
        meta.id = raw.id;
        meta.provenance = raw.provenance;
        Ok(meta)
    }
}

fn normalize_tags(tags: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    tags.into_iter()
        .filter_map(|tag| normalize_tag(&tag))
        .collect()
}

fn normalize_tag(tag: &str) -> Option<String> {
    let collapsed = tag.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
}