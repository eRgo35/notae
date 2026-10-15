//! Note metadata, per-field provenance, and typed domain errors.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{IndexStatus, NoteId, PdfFingerprint, RelativePdfPath};
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
    /// A record carried neither a `provenance` object nor a legacy `origin`
    /// scalar, so its manual edits cannot be distinguished from never-set
    /// fields.
    #[error("missing metadata provenance: expected `provenance` or legacy `origin`")]
    MissingProvenance,
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
    pub const ALL: [Self; 4] = [Self::Id, Self::Title, Self::ComposerOrArtist, Self::Tags];
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

    /// Returns provenance where every field carries the same origin.
    ///
    /// This is how a legacy record's scalar `origin` migrates onto the
    /// per-field representation: an old `origin: "manual"` means the whole
    /// record was hand-edited, so every field must stay authoritative.
    #[must_use]
    pub fn with_every_field(origin: MetadataOrigin) -> Self {
        Self {
            id: origin,
            title: origin,
            composer_or_artist: origin,
            tags: origin,
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

    /// Records the origin of one field without changing its value, for example
    /// when a reconciliation preserves a value the user edited by hand.
    pub fn set_origin(&mut self, field: MetadataField, origin: MetadataOrigin) {
        self.provenance.set_origin(field, origin);
    }

    /// Marks one field as edited by the user without changing its value.
    pub fn mark_manual(&mut self, field: MetadataField) {
        self.provenance.mark_manual(field);
    }

    /// Replaces the note ID as a user edit.
    ///
    /// The ID field is marked [`MetadataOrigin::Manual`] even when `id` is
    /// `None`: removing a number by hand is exactly the decision that a PC
    /// import must not undo. Use [`NoteMetadata::assign_id`] for values that
    /// come from a filename or an import.
    pub fn set_id(&mut self, id: Option<NoteId>) {
        self.assign_id(id, MetadataOrigin::Manual);
    }

    /// Replaces the note ID with an explicit origin, for filename parsing,
    /// indexing, and PC import.
    ///
    /// Unlike [`NoteMetadata::set_id`], the origin is whatever the caller
    /// passes, so a filename-derived ID is never mistaken for a manual edit.
    pub fn assign_id(&mut self, id: Option<NoteId>, origin: MetadataOrigin) {
        self.id = id;
        self.set_origin(MetadataField::Id, origin);
    }

    /// Replaces the title as a user edit, marking the title field as
    /// [`MetadataOrigin::Manual`].
    ///
    /// # Errors
    /// Returns [`DomainError::EmptyTitle`] when the trimmed title is empty.
    pub fn set_title(&mut self, title: &str) -> Result<(), DomainError> {
        self.assign_title(title, MetadataOrigin::Manual)
    }

    /// Replaces the title with an explicit origin, for filename parsing and PC
    /// import.
    ///
    /// # Errors
    /// Returns [`DomainError::EmptyTitle`] when the trimmed title is empty,
    /// leaving the record untouched.
    pub fn assign_title(&mut self, title: &str, origin: MetadataOrigin) -> Result<(), DomainError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(DomainError::EmptyTitle);
        }
        self.title.clear();
        self.title.push_str(title);
        self.set_origin(MetadataField::Title, origin);
        Ok(())
    }

    /// Replaces the composer or artist as a user edit.
    ///
    /// The field is marked [`MetadataOrigin::Manual`] even when
    /// `composer_or_artist` is `None`, because a composer the user deleted
    /// must not be refilled by an import. Use
    /// [`NoteMetadata::assign_composer_or_artist`] for import and
    /// initialization paths.
    pub fn set_composer_or_artist(&mut self, composer_or_artist: Option<String>) {
        self.assign_composer_or_artist(composer_or_artist, MetadataOrigin::Manual);
    }

    /// Replaces the composer or artist with an explicit origin, for filename
    /// parsing, initialization, and PC import.
    pub fn assign_composer_or_artist(
        &mut self,
        composer_or_artist: Option<String>,
        origin: MetadataOrigin,
    ) {
        self.composer_or_artist = composer_or_artist;
        self.set_origin(MetadataField::ComposerOrArtist, origin);
    }

    /// Replaces the whole tag set as a user edit, marking the tags field as
    /// [`MetadataOrigin::Manual`].
    ///
    /// An empty set is still a manual edit: the user removed those tags.
    pub fn set_tags(&mut self, tags: impl IntoIterator<Item = String>) {
        self.assign_tags(tags, MetadataOrigin::Manual);
    }

    /// Replaces the whole tag set with an explicit origin, for filename
    /// parsing, initialization, and PC import.
    pub fn assign_tags(&mut self, tags: impl IntoIterator<Item = String>, origin: MetadataOrigin) {
        self.tags = normalize_tags(tags);
        self.set_origin(MetadataField::Tags, origin);
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
    ///
    /// A record must carry provenance, either as the `provenance` object or as
    /// the legacy scalar `origin`. A record with neither is rejected rather
    /// than treated as wholly [`MetadataOrigin::Unset`]: absence of a marker
    /// is not evidence that the user never edited the field, and accepting it
    /// would let an import overwrite genuinely manual metadata. The legacy
    /// scalar is migrated uniformly to all four fields, so `origin:
    /// "manual"` keeps every field authoritative.
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
            provenance: Option<FieldProvenance>,
            origin: Option<MetadataOrigin>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let provenance = match (raw.provenance, raw.origin) {
            (Some(provenance), _) => provenance,
            (None, Some(origin)) => FieldProvenance::with_every_field(origin),
            (None, None) => {
                return Err(serde::de::Error::custom(DomainError::MissingProvenance));
            }
        };

        let mut meta = NoteMetadata::new(&raw.title, raw.composer_or_artist, raw.tags)
            .map_err(serde::de::Error::custom)?;
        meta.id = raw.id;
        meta.provenance = provenance;
        Ok(meta)
    }
}

/// One indexed PDF: where it lives, what it looks like, and its metadata.
///
/// This is the record the index stores and the browsing order sorts, so it
/// owns the file identity alongside the user-visible metadata and the
/// [`IndexStatus`] the indexing pipeline reached.
///
/// # Wire form
///
/// Serializes as `{"path", "fingerprint", "metadata", "status"}`, with
/// `status` a snake_case [`IndexStatus`] name. A record without `status`
/// deserializes to [`IndexStatus::Discovered`], so an older sidecar row still
/// loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedNote {
    /// Path relative to the library root.
    path: RelativePdfPath,
    /// Observed content identity.
    fingerprint: PdfFingerprint,
    /// User-visible metadata.
    metadata: NoteMetadata,
    /// Where the record sits in the indexing pipeline.
    #[serde(default)]
    status: IndexStatus,
}

impl IndexedNote {
    /// Builds an indexed record in the [`IndexStatus::Discovered`] state: the
    /// file exists but nothing has been read from it yet.
    #[must_use]
    pub const fn new(
        path: RelativePdfPath,
        fingerprint: PdfFingerprint,
        metadata: NoteMetadata,
    ) -> Self {
        Self {
            path,
            fingerprint,
            metadata,
            status: IndexStatus::Discovered,
        }
    }

    /// Returns the library-relative path.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.path
    }

    /// Returns the content fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &PdfFingerprint {
        &self.fingerprint
    }

    /// Returns the metadata.
    #[must_use]
    pub const fn metadata(&self) -> &NoteMetadata {
        &self.metadata
    }

    /// Returns the note ID, when one is known.
    #[must_use]
    pub const fn id(&self) -> Option<NoteId> {
        self.metadata.id
    }

    /// Returns the title.
    #[must_use]
    pub fn title(&self) -> &str {
        self.metadata.title()
    }

    /// Returns where the record sits in the indexing pipeline.
    #[must_use]
    pub const fn status(&self) -> IndexStatus {
        self.status
    }

    /// Sets the pipeline status, leaving path, fingerprint, and metadata — and
    /// therefore every provenance marker — untouched.
    pub const fn set_status(&mut self, status: IndexStatus) {
        self.status = status;
    }

    /// Returns a copy of this record in the given [`IndexStatus`], for callers
    /// that update status without holding a mutable reference.
    #[must_use]
    pub fn with_status(&self, status: IndexStatus) -> Self {
        Self {
            status,
            path: self.path.clone(),
            fingerprint: self.fingerprint.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

fn normalize_tags(tags: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    tags.into_iter()
        .filter_map(|tag| normalize_tag(&tag))
        .collect()
}

fn normalize_tag(tag: &str) -> Option<String> {
    let collapsed = crate::text::normalize_whitespace(tag);
    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
}
