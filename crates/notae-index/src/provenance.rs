//! Filename-derived metadata and the manual edits that survive it.
//!
//! A rescan re-derives metadata from the filename of every file it sees. That
//! derivation is authoritative only for fields the user has not edited: a
//! [`NoteMetadata`] carries the [`MetadataOrigin`] of each
//! [`MetadataField`] precisely so a filename can refresh one field and leave
//! another alone.
//!
//! [`refresh_from_filename`] is the whole of that rule. It copies the ID and
//! title parsed from the name onto an existing record and reports what it did
//! per field, which is what lets a rescan show the user "these fields are yours,
//! these come from the filename" before anything is written.

use notae_domain::{
    DomainError, FilenameError, MetadataField, MetadataOrigin, NoteMetadata, ParsedFilename,
    parse_note_filename,
};

/// Why metadata could not be derived from a filename.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetadataRefreshError {
    /// The name is not a note filename.
    #[error("filename is not a note filename: {0}")]
    Filename(#[from] FilenameError),
    /// The name parsed, but the value it carries is not valid metadata.
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// What a refresh did to one field.
///
/// The lists name every tracked field, so a caller can show a complete
/// provenance summary rather than only the fields that changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshOutcome {
    refreshed: Vec<MetadataField>,
    preserved: Vec<MetadataField>,
}

impl RefreshOutcome {
    /// Records the outcome of refreshing one field.
    fn record(&mut self, field: MetadataField, refreshed: bool) {
        if refreshed {
            self.refreshed.push(field);
        } else {
            self.preserved.push(field);
        }
    }

    /// The fields whose values were re-derived from the filename.
    #[must_use]
    pub fn refreshed(&self) -> &[MetadataField] {
        &self.refreshed
    }

    /// The fields the refresh left as they were.
    ///
    /// A field is preserved either because the user edited it by hand, which
    /// makes it authoritative, or because a filename says nothing about it:
    /// composer/artist and tags are never derived from a name, so a refresh
    /// always preserves them.
    #[must_use]
    pub fn preserved(&self) -> &[MetadataField] {
        &self.preserved
    }
}

/// Derives fresh metadata for a file Notae has not seen before.
///
/// Every field the filename carries — the ID and the title — is marked
/// [`MetadataOrigin::FilenameDerived`], so a later refresh may overwrite it,
/// while an edit the user makes marks it [`MetadataOrigin::Manual`] and stops
/// that from ever happening again.
///
/// # Errors
/// Returns [`MetadataRefreshError`] when the name is not a PDF name, or when
/// the name yields an empty title.
pub fn metadata_from_filename(
    parsed: ParsedFilename,
) -> Result<NoteMetadata, MetadataRefreshError> {
    parsed.into_note_metadata().map_err(Into::into)
}

/// Refreshes `existing` from the note filename `filename`.
///
/// The ID and the title are re-derived from the name and marked
/// [`MetadataOrigin::FilenameDerived`] — unless the user edited either field,
/// in which case the stored value is kept untouched. Composer/artist and tags
/// are never derived from a filename and are always preserved.
///
/// The record is returned rather than mutated, so a caller can compute the
/// result and decide whether it differs from what it already holds.
///
/// # Errors
/// Returns [`MetadataRefreshError`] when the name is not a PDF name, or when
/// the name yields an empty title. An existing record is never left modified
/// on failure.
pub fn refresh_from_filename(
    existing: &NoteMetadata,
    filename: &str,
) -> Result<(NoteMetadata, RefreshOutcome), MetadataRefreshError> {
    let parsed = parse_note_filename(filename)?;
    let (metadata, outcome) = refresh_from_parsed(existing, &parsed)?;
    Ok((metadata, outcome))
}

/// Refreshes `existing` from an already-parsed name.
///
/// Separated from [`refresh_from_filename`] so a rescan that parses each name
/// once does not parse it again, and so [`refresh_from_filename`] can name its
/// error type in its own signature.
pub(crate) fn refresh_from_parsed(
    existing: &NoteMetadata,
    parsed: &ParsedFilename,
) -> Result<(NoteMetadata, RefreshOutcome), MetadataRefreshError> {
    let mut metadata = existing.clone();
    let mut outcome = RefreshOutcome::default();

    if !existing.provenance().is_manual(MetadataField::Id) {
        metadata.assign_id(parsed.id(), MetadataOrigin::FilenameDerived);
    }
    outcome.record(MetadataField::Id, !existing.provenance().is_manual(MetadataField::Id));

    let title_is_manual = existing.provenance().is_manual(MetadataField::Title);
    if !title_is_manual {
        metadata.assign_title(parsed.title(), MetadataOrigin::FilenameDerived)?;
    }
    outcome.record(MetadataField::Title, !title_is_manual);

    // A filename carries no composer, artist, or tag, so these are preserved
    // by definition rather than by a manual edit.
    outcome.record(MetadataField::ComposerOrArtist, false);
    outcome.record(MetadataField::Tags, false);

    Ok((metadata, outcome))
}

impl Default for RefreshOutcome {
    /// An empty outcome, which every field recorder starts from.
    fn default() -> Self {
        Self {
            refreshed: Vec::new(),
            preserved: Vec::new(),
        }
    }
}