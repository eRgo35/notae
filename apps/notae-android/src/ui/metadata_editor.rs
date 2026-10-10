use notae_domain::{IndexedNote, NoteId, NoteMetadata, Sha256Digest};
use notae_index::IndexStore;

use crate::ui::search::UiError;

/// Values displayed and submitted by the metadata editor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditableMetadata {
    /// Stable SHA-256 key of the PDF being edited.
    pub note_key: String,
    /// Decimal note ID, or empty when the user removes it.
    pub id: String,
    /// Required display title.
    pub title: String,
    /// Composer or artist, or empty when unset.
    pub composer_or_artist: String,
    /// Tag values; the domain layer normalizes whitespace and duplicates.
    pub tags: Vec<String>,
}

/// Loads and saves metadata while retaining the indexed file identity.
pub struct MetadataEditor<'a> {
    local: &'a IndexStore,
}

impl<'a> MetadataEditor<'a> {
    /// Edits metadata in the supplied index.
    #[must_use]
    pub const fn new(local: &'a IndexStore) -> Self {
        Self { local }
    }

    /// Loads editable fields by the stable content key used by the library UI.
    pub fn load(&self, note_key: &str) -> Result<EditableMetadata, UiError> {
        let note = self.find_note(note_key)?;
        Ok(EditableMetadata {
            note_key: note.fingerprint().sha256().to_string(),
            id: note.id().map_or_else(String::new, |id| id.to_string()),
            title: note.title().to_owned(),
            composer_or_artist: note
                .metadata()
                .composer_or_artist()
                .unwrap_or_default()
                .to_owned(),
            tags: note.metadata().tags().iter().cloned().collect(),
        })
    }

    /// Validates and saves the supplied fields as user-authored metadata.
    pub fn save(&self, input: EditableMetadata) -> Result<(), UiError> {
        let original = self.find_note(&input.note_key)?;
        let id = if input.id.trim().is_empty() {
            None
        } else {
            Some(NoteId::parse(input.id.trim())?)
        };
        let composer_or_artist = normalized_optional(input.composer_or_artist);

        let mut metadata: NoteMetadata = original.metadata().clone();
        metadata.set_id(id);
        metadata.set_title(&input.title)?;
        metadata.set_composer_or_artist(composer_or_artist);
        metadata.set_tags(input.tags);
        let edited = IndexedNote::new(
            original.path().clone(),
            original.fingerprint().clone(),
            metadata,
        )
        .with_status(original.status());
        self.local.reindex_note(&edited)?;
        Ok(())
    }

    fn find_note(&self, note_key: &str) -> Result<IndexedNote, UiError> {
        let key = Sha256Digest::parse(note_key)?;
        self.local
            .get_by_hash(&key)?
            .ok_or_else(|| UiError::NoteNotFound(note_key.to_owned()))
    }
}

fn normalized_optional(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
