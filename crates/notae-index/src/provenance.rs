//! Filename-derived metadata and the manual edits that survive it.
//!
//! A rescan re-derives metadata from the filename of every file it sees. That
//! derivation is authoritative only for fields the user has not edited: a
//! [`NoteMetadata`] carries the [`MetadataOrigin`] of each
//! [`MetadataField`] precisely so a filename can refresh one field and leave
//! another alone.
//!
//! [`refresh_from_filename`] is the filename half of that rule; it copies the ID and
//! title parsed from the name onto an existing record and reports what it did
//! per field, which is what lets a rescan show the user "these fields are yours,
//! these come from the filename" before anything is written.
//! [`refresh_from_sidecar`] is the other half: a value from the PC sidecar
//! outranks a filename-derived one, yields to a manual edit, and is itself
//! kept by the filename refresh.

use notae_domain::{
    DomainError, FilenameError, MetadataField, MetadataOrigin, NoteId, NoteMetadata,
    ParsedFilename, SidecarEntry, parse_note_filename,
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
/// [`MetadataOrigin::FilenameDerived`] — unless the field is manual or was
/// imported from the PC sidecar ([`MetadataOrigin::PcImported`]), in which case
/// the stored value is kept untouched. Composer/artist and tags
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
    let title = parsed.title().trim();
    // The name is validated before any field is consulted. A name that carries
    // no usable title is a bad file wherever it turns up, and a stored manual
    // title must not excuse it: the file would otherwise be re-indexed or
    // relocated under metadata its own name cannot justify.
    if title.is_empty() {
        return Err(MetadataRefreshError::Domain(DomainError::EmptyTitle));
    }

    let mut metadata = existing.clone();
    let mut outcome = RefreshOutcome::default();

    let id_open = filename_may_overwrite(existing, MetadataField::Id);
    if id_open {
        metadata.assign_id(parsed.id(), MetadataOrigin::FilenameDerived);
    }
    outcome.record(MetadataField::Id, id_open);

    let title_open = filename_may_overwrite(existing, MetadataField::Title);
    if title_open {
        metadata.assign_title(title, MetadataOrigin::FilenameDerived)?;
    }
    outcome.record(MetadataField::Title, title_open);

    // A filename carries no composer, artist, or tag, so these are preserved
    // by definition rather than by a manual edit.
    outcome.record(MetadataField::ComposerOrArtist, false);
    outcome.record(MetadataField::Tags, false);

    Ok((metadata, outcome))
}

/// Whether a filename may overwrite `field`: only a value that is itself
/// filename-derived or never set.
fn filename_may_overwrite(existing: &NoteMetadata, field: MetadataField) -> bool {
    matches!(
        existing.origin_of(field),
        MetadataOrigin::FilenameDerived | MetadataOrigin::Unset
    )
}

/// Applies a PC sidecar entry to `existing`.
///
/// Every field the user edited by hand is kept. Of the rest, the number and
/// title are assigned only when the entry carries one (a title is trimmed and
/// must be non-empty to count), while the composer and tags are always
/// assigned, so an entry without them clears an earlier import. Assigned
/// fields are marked [`MetadataOrigin::PcImported`].
///
/// # Errors
/// Returns [`MetadataRefreshError`] when the metadata rejects a value; the
/// existing record is never modified.
pub fn refresh_from_sidecar(
    existing: &NoteMetadata,
    entry: &SidecarEntry,
) -> Result<(NoteMetadata, RefreshOutcome), MetadataRefreshError> {
    let mut metadata = existing.clone();
    let mut outcome = RefreshOutcome::default();
    let open = |field| !existing.provenance().is_manual(field);

    let number = entry.number.filter(|_| open(MetadataField::Id));
    if let Some(number) = number {
        metadata.assign_id(Some(NoteId::new(number)), MetadataOrigin::PcImported);
    }
    outcome.record(MetadataField::Id, number.is_some());

    let title = entry
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty() && open(MetadataField::Title));
    if let Some(title) = title {
        metadata.assign_title(title, MetadataOrigin::PcImported)?;
    }
    outcome.record(MetadataField::Title, title.is_some());

    let composer_open = open(MetadataField::ComposerOrArtist);
    if composer_open {
        metadata.assign_composer_or_artist(entry.composer.clone(), MetadataOrigin::PcImported);
    }
    outcome.record(MetadataField::ComposerOrArtist, composer_open);

    let tags_open = open(MetadataField::Tags);
    if tags_open {
        metadata.assign_tags(entry.tags.iter().cloned(), MetadataOrigin::PcImported);
    }
    outcome.record(MetadataField::Tags, tags_open);

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

#[cfg(test)]
mod tests {
    use super::*;
    use notae_domain::{NoteId, SidecarEntry};

    fn entry() -> SidecarEntry {
        SidecarEntry {
            path: "a.pdf".to_owned(),
            size: 1,
            sha256: "00".repeat(32),
            number: None,
            title: None,
            composer: None,
            tags: Vec::new(),
            text: None,
        }
    }

    fn tags(m: &NoteMetadata) -> Vec<String> {
        m.tags().iter().cloned().collect()
    }

    #[test]
    fn filename_refresh_keeps_pc_imported_fields() {
        let mut m = NoteMetadata::new("Adeste Fideles (PC)", None, []).expect("title");
        m.assign_title("Adeste Fideles (PC)", MetadataOrigin::PcImported)
            .expect("title");
        m.assign_id(Some(NoteId::new(1)), MetadataOrigin::PcImported);
        let (out, _) = refresh_from_filename(&m, "001 - Adeste Fideles.pdf").expect("refresh");
        assert_eq!(out.title(), "Adeste Fideles (PC)");
        assert_eq!(
            out.origin_of(MetadataField::Title),
            MetadataOrigin::PcImported
        );
        assert_eq!(out.id(), Some(NoteId::new(1)));
        assert_eq!(out.origin_of(MetadataField::Id), MetadataOrigin::PcImported);
    }

    #[test]
    fn sidecar_overrides_filename_but_not_manual() {
        let mut m = NoteMetadata::new("Old", None, []).expect("title");
        m.assign_title("Old", MetadataOrigin::FilenameDerived)
            .expect("title");
        m.assign_composer_or_artist(Some("Bach".to_owned()), MetadataOrigin::Manual);
        let e = SidecarEntry {
            title: Some("T".to_owned()),
            composer: Some("Taizé".to_owned()),
            tags: vec!["Wielkanoc".to_owned()],
            number: Some(7),
            ..entry()
        };
        let (out, _) = refresh_from_sidecar(&m, &e).expect("sidecar");
        assert_eq!(out.title(), "T");
        assert_eq!(
            out.origin_of(MetadataField::Title),
            MetadataOrigin::PcImported
        );
        assert_eq!(out.composer_or_artist(), Some("Bach"));
        assert_eq!(
            out.origin_of(MetadataField::ComposerOrArtist),
            MetadataOrigin::Manual
        );
        assert_eq!(tags(&out), vec!["Wielkanoc".to_owned()]);
        assert_eq!(
            out.origin_of(MetadataField::Tags),
            MetadataOrigin::PcImported
        );
        assert_eq!(out.id(), Some(NoteId::new(7)));
        assert_eq!(out.origin_of(MetadataField::Id), MetadataOrigin::PcImported);
    }

    #[test]
    fn sidecar_none_keeps_number_and_title_but_clears_composer_and_tags() {
        let mut m =
            NoteMetadata::new("Kept", Some("X".to_owned()), ["a".to_owned()]).expect("metadata");
        m.assign_id(Some(NoteId::new(3)), MetadataOrigin::PcImported);
        m.assign_title("Kept", MetadataOrigin::PcImported)
            .expect("title");
        m.assign_composer_or_artist(Some("X".to_owned()), MetadataOrigin::PcImported);
        m.assign_tags(["a".to_owned()], MetadataOrigin::PcImported);
        let (out, _) = refresh_from_sidecar(&m, &entry()).expect("sidecar");
        assert_eq!(out.id(), Some(NoteId::new(3)));
        assert_eq!(out.title(), "Kept");
        assert_eq!(out.composer_or_artist(), None);
        assert!(tags(&out).is_empty());
        assert_eq!(
            out.origin_of(MetadataField::ComposerOrArtist),
            MetadataOrigin::PcImported
        );
        assert_eq!(
            out.origin_of(MetadataField::Tags),
            MetadataOrigin::PcImported
        );
    }
}
