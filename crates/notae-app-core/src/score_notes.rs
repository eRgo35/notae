//! Score notes: one free text per score, stored by content key with the library path in a
//! private JSON file.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::favorites::{PieceRef, write_atomically};
use crate::recent::is_content_key;

const SCHEMA_VERSION: u32 = 1;

/// The text note on one score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreNote {
    /// Lowercase SHA-256 hex of the PDF.
    pub key: String,
    /// Library-relative path, used to re-link the note after a rescan.
    pub path: String,
    /// The trimmed, non-blank text.
    pub text: String,
}

#[derive(Serialize, Deserialize)]
struct ScoreNotesFile {
    schema_version: u32,
    notes: Vec<ScoreNote>,
}

/// The score notes, at most one per content key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScoreNotes {
    notes: Vec<ScoreNote>,
}

impl ScoreNotes {
    /// Parses the JSON file; corrupt text or an unknown schema version is an empty list.
    /// Invalid keys, blank texts and repeated keys are dropped.
    #[must_use]
    pub fn from_json(text: &str) -> Self {
        let Ok(file) = serde_json::from_str::<ScoreNotesFile>(text) else {
            return Self::default();
        };
        if file.schema_version != SCHEMA_VERSION {
            return Self::default();
        }
        let mut notes = Self::default();
        for mut note in file.notes {
            note.text = note.text.trim().to_owned();
            if is_content_key(&note.key) && !note.text.is_empty() && notes.get(&note.key).is_none()
            {
                notes.notes.push(note);
            }
        }
        notes
    }

    /// The JSON file text.
    #[must_use]
    pub fn to_json(&self) -> String {
        let file = ScoreNotesFile {
            schema_version: SCHEMA_VERSION,
            notes: self.notes.clone(),
        };
        serde_json::to_string(&file).unwrap_or_default()
    }

    /// Reads `path`; a missing or unreadable file is an empty list.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map_or_else(|_| Self::default(), |text| Self::from_json(&text))
    }

    /// Writes through a temporary file so a crash never leaves half a list.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from writing or renaming the file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_atomically(path, &self.to_json())
    }

    /// The note text of the score with this content key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.notes
            .iter()
            .find(|note| note.key == key)
            .map(|note| note.text.as_str())
    }

    /// Stores the trimmed `text` for `piece`; blank text removes the note. Returns whether
    /// anything changed.
    pub fn set(&mut self, piece: PieceRef, text: &str) -> bool {
        let text = text.trim();
        let existing = self.notes.iter().position(|note| note.key == piece.key);
        match (existing, text.is_empty()) {
            (None, true) => false,
            (Some(index), true) => {
                self.notes.remove(index);
                true
            }
            (None, false) => {
                self.notes.push(ScoreNote {
                    key: piece.key,
                    path: piece.path,
                    text: text.to_owned(),
                });
                true
            }
            (Some(index), false) => {
                let note = &mut self.notes[index];
                let changed = note.text != text || note.path != piece.path;
                text.clone_into(&mut note.text);
                note.path = piece.path;
                changed
            }
        }
    }

    /// The notes.
    #[must_use]
    pub fn items(&self) -> &[ScoreNote] {
        &self.notes
    }

    /// Adds the other notes; an incoming note replaces the one with the same key.
    pub fn merge(&mut self, other: ScoreNotes) {
        for note in other.notes {
            match self.notes.iter_mut().find(|own| own.key == note.key) {
                Some(own) => *own = note,
                None => self.notes.push(note),
            }
        }
    }

    /// Points each note whose key is not in `library` at the entry with the same path.
    /// `library` holds `(key, path)` pairs. A note that ends up with a key already listed
    /// earlier is dropped. Returns whether anything changed.
    pub fn relink(&mut self, library: &[(String, String)]) -> bool {
        let mut changed = false;
        for note in &mut self.notes {
            let mut piece = PieceRef {
                key: note.key.clone(),
                path: note.path.clone(),
            };
            if piece.relink(library) {
                note.key = piece.key;
                changed = true;
            }
        }
        let before = self.notes.len();
        let mut seen = HashSet::new();
        self.notes.retain(|note| seen.insert(note.key.clone()));
        changed || self.notes.len() != before
    }
}
