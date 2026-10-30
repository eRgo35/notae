//! Favorite scores, stored as content keys with their library paths in a private JSON file.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::recent::is_content_key;

const SCHEMA_VERSION: u32 = 1;

/// A score a favorite or a set item points at: its content key and its last known library path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceRef {
    /// Lowercase SHA-256 hex of the PDF.
    pub key: String,
    /// Library-relative path, used to re-link the piece after a rescan.
    pub path: String,
}

#[derive(Serialize, Deserialize)]
struct FavoritesFile {
    schema_version: u32,
    items: Vec<PieceRef>,
}

/// Favorite scores, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Favorites {
    items: Vec<PieceRef>,
}

impl Favorites {
    /// Parses the JSON file; corrupt text or an unknown schema version is an empty list.
    /// Invalid keys and repeats are dropped.
    #[must_use]
    pub fn from_json(text: &str) -> Self {
        let mut favorites = Self::default();
        let Ok(file) = serde_json::from_str::<FavoritesFile>(text) else {
            return favorites;
        };
        if file.schema_version != SCHEMA_VERSION {
            return favorites;
        }
        for piece in file.items {
            if is_content_key(&piece.key) && !favorites.contains(&piece.key) {
                favorites.items.push(piece);
            }
        }
        favorites
    }

    /// The JSON file text.
    #[must_use]
    pub fn to_json(&self) -> String {
        let file = FavoritesFile {
            schema_version: SCHEMA_VERSION,
            items: self.items.clone(),
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

    /// Adds `piece` at the front, or removes it if already present. Returns the new state.
    pub fn toggle(&mut self, piece: PieceRef) -> bool {
        if self.contains(&piece.key) {
            self.items.retain(|existing| existing.key != piece.key);
            false
        } else {
            self.items.insert(0, piece);
            true
        }
    }

    /// Whether the score with this content key is a favorite.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.items.iter().any(|piece| piece.key == key)
    }

    /// Favorites, newest first.
    #[must_use]
    pub fn items(&self) -> &[PieceRef] {
        &self.items
    }

    /// Adds the other list's missing keys at the end; existing order is kept.
    pub fn merge(&mut self, other: &Favorites) {
        for piece in &other.items {
            if !self.contains(&piece.key) {
                self.items.push(piece.clone());
            }
        }
    }

    /// Points each favorite whose key is not in `library` at the entry with the same path.
    /// `library` holds `(key, path)` pairs. Returns whether anything changed.
    pub fn relink(&mut self, library: &[(String, String)]) -> bool {
        let mut changed = false;
        for piece in &mut self.items {
            changed |= piece.relink(library);
        }
        changed
    }
}

impl PieceRef {
    /// Takes the key of the library entry with this path if the current key is gone.
    pub(crate) fn relink(&mut self, library: &[(String, String)]) -> bool {
        if library.iter().any(|(key, _)| *key == self.key) {
            return false;
        }
        match library.iter().find(|(_, path)| *path == self.path) {
            Some((key, _)) => {
                self.key.clone_from(key);
                true
            }
            None => false,
        }
    }
}

/// Writes `text` to `path` through a sibling `.tmp` file and a rename.
pub(crate) fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text)?;
    fs::rename(temporary, path)
}
