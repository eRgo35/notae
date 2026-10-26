use crate::language::pick;
use notae_domain::{MetadataField, NoteId, NoteMetadata};
use notae_index::{
    EntryChange, ImportEntry, ImportPreview, ImportReport, IndexStore, SidecarImporter,
};

use crate::operations::events::{ImportPreviewRow, ImportPreviewSummary};
use crate::ui::search::UiError;

/// Holds a read-only sidecar preview until the user confirms it.
#[derive(Default)]
pub struct ImportPreviewViewModel {
    preview: Option<ImportPreview>,
    confirmed: bool,
}

impl ImportPreviewViewModel {
    /// Replaces the current preview and clears any prior confirmation.
    pub fn load(&mut self, preview: ImportPreview) {
        self.preview = Some(preview);
        self.confirmed = false;
    }

    /// Marks the visible preview as explicitly confirmed.
    pub fn confirm(&mut self) {
        self.confirmed = true;
    }

    /// Returns counts and the per-entry details for the preview surface.
    #[must_use]
    pub fn summary(&self) -> Option<ImportPreviewSummary> {
        self.preview.as_ref().map(|preview| {
            let mut rows: Vec<_> = preview.entries().iter().map(preview_row).collect();
            rows.extend(preview.missing().iter().map(|note| ImportPreviewRow {
                path: note.path().as_str().to_owned(),
                change: pick("Not in sidecar", "Brak w pliku sidecar").to_owned(),
                local_title: Some(note.title().to_owned()),
                proposed_title: String::new(),
                local_metadata: Some(format_metadata(note)),
                proposed_metadata: String::new(),
                imported_fields: Vec::new(),
                preserved_fields: Vec::new(),
                ocr_available: false,
                conflict: None,
            }));
            ImportPreviewSummary {
                new: preview.new_entries().len(),
                changed: preview.changed_entries().len(),
                unchanged: preview.unchanged_entries().len(),
                missing: preview.missing().len() + preview.absent_entries().len(),
                mismatches: preview.mismatches().len(),
                unreadable: preview.unreadable_entries().len(),
                conflicts: preview.conflicts().len(),
                ocr_available: preview
                    .entries()
                    .iter()
                    .filter(|entry| entry.sidecar().ocr().is_some())
                    .count(),
                rows,
            }
        })
    }

    /// Applies the preview only after [`confirm`](Self::confirm).
    ///
    /// # Errors
    /// Returns [`UiError::ConfirmationRequired`] until confirmed, or a core
    /// import error if the index cannot be updated.
    pub fn apply(&mut self, local: &IndexStore) -> Result<ImportReport, UiError> {
        if !self.confirmed {
            return Err(UiError::ConfirmationRequired);
        }
        let Some(preview) = self.preview.take() else {
            return Err(UiError::NoImportPreview);
        };
        self.confirmed = false;
        Ok(SidecarImporter::apply(preview, local)?)
    }
}

/// Names the metadata fields included in a core import preview.
#[must_use]
pub fn metadata_field_names(fields: &[MetadataField]) -> Vec<&'static str> {
    fields
        .iter()
        .map(|field| match field {
            MetadataField::Id => pick("ID", "numer"),
            MetadataField::Title => pick("title", "tytuł"),
            MetadataField::ComposerOrArtist => pick("composer/artist", "kompozytor/wykonawca"),
            MetadataField::Tags => pick("tags", "tagi"),
        })
        .collect()
}

fn preview_row(entry: &ImportEntry) -> ImportPreviewRow {
    let change = match entry.change() {
        EntryChange::New => pick("New", "Nowy").to_owned(),
        EntryChange::Unchanged => pick("Unchanged", "Bez zmian").to_owned(),
        EntryChange::Changed => pick("Changed", "Zmieniony").to_owned(),
        EntryChange::Moved { from } => pick(
            format!("Moved from {from}"),
            format!("Przeniesiony z {from}"),
        ),
        EntryChange::HashMismatch { .. } => {
            pick("Hash mismatch", "Niezgodna suma kontrolna").to_owned()
        }
        EntryChange::Missing => pick("Missing", "Brak pliku").to_owned(),
        EntryChange::Unverified { .. } => pick("Unreadable", "Nieczytelny").to_owned(),
    };
    let sidecar = entry.sidecar();
    let local = entry.local();
    let metadata = entry.metadata();
    let conflict = entry.conflict().map(|conflict| {
        format!(
            "ID {}: {} conflicts with {}",
            conflict.id(),
            conflict.existing(),
            conflict.conflicting()
        )
    });
    ImportPreviewRow {
        path: entry.path().as_str().to_owned(),
        change,
        local_title: local.map(|note| note.title().to_owned()),
        proposed_title: sidecar.metadata().title().to_owned(),
        local_metadata: local.map(format_metadata),
        proposed_metadata: format_metadata_values(sidecar.metadata(), sidecar.to_note().id()),
        imported_fields: metadata_field_names(metadata.imported()),
        preserved_fields: metadata_field_names(metadata.preserved()),
        ocr_available: sidecar.ocr().is_some(),
        conflict,
    }
}

fn format_metadata(note: &notae_domain::IndexedNote) -> String {
    format_metadata_values(note.metadata(), note.id())
}

fn format_metadata_values(metadata: &NoteMetadata, id: Option<NoteId>) -> String {
    let id = id.map_or_else(|| "—".to_owned(), |value| value.to_string());
    let composer = metadata.composer_or_artist().unwrap_or("—");
    let tags = if metadata.tags().is_empty() {
        "—".to_owned()
    } else {
        metadata
            .tags()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "ID: {id}; title: {}; composer/artist: {composer}; tags: {tags}",
        metadata.title()
    )
}
