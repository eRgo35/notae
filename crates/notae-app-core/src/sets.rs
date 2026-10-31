//! Named, ordered singing sets, stored in a private JSON file.

#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::favorites::{PieceRef, write_atomically};
use crate::recent::is_content_key;

const SCHEMA_VERSION: u32 = 1;

/// What a set item is: a score or a free-text note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ItemKind {
    /// A score from the library.
    Piece(PieceRef),
    /// A free-text note.
    Note {
        /// The note's text.
        text: String,
    },
}

/// One row of a set. Its own `id` keeps repeated pieces apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetItem {
    /// The item's id, unique within its set.
    pub id: String,
    /// What the item is.
    #[serde(flatten)]
    pub kind: ItemKind,
}

/// A singing set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Set {
    /// The set's id.
    pub id: String,
    /// The set's name.
    pub name: String,
    /// ISO `YYYY-MM-DD`.
    pub date: Option<String>,
    /// Free-text notes about the set.
    pub notes: Option<String>,
    /// Unix seconds of the last change to the set or its items.
    pub modified: u64,
    /// The set's items, in order.
    pub items: Vec<SetItem>,
    /// The id of the item opened last.
    pub last_opened_item: Option<String>,
}

impl Set {
    /// Drops piece items whose content key is not valid.
    pub(crate) fn drop_invalid_pieces(&mut self) {
        self.items.retain(|item| match &item.kind {
            ItemKind::Piece(piece) => is_content_key(&piece.key),
            ItemKind::Note { .. } => true,
        });
    }
}

/// The user-editable fields of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetFields {
    /// The set's name.
    pub name: String,
    /// ISO `YYYY-MM-DD`.
    pub date: Option<String>,
    /// Free-text notes about the set.
    pub notes: Option<String>,
}

/// Why a set's fields were rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, uniffi::Enum)]
pub enum SetFieldError {
    /// The name is empty.
    #[error("the set needs a name")]
    EmptyName,
    /// The date is not a real `YYYY-MM-DD` date.
    #[error("the date must be a real YYYY-MM-DD date")]
    InvalidDate,
}

impl SetFields {
    /// Trims the name, turns empty date and notes into `None`, and checks the date.
    ///
    /// # Errors
    ///
    /// `EmptyName` for a blank name, `InvalidDate` for a date that is not a real calendar day.
    pub fn validate(self) -> Result<SetFields, SetFieldError> {
        let name = self.name.trim().to_owned();
        if name.is_empty() {
            return Err(SetFieldError::EmptyName);
        }
        let date = self
            .date
            .map(|date| date.trim().to_owned())
            .filter(|date| !date.is_empty());
        if let Some(date) = &date
            && !is_real_date(date)
        {
            return Err(SetFieldError::InvalidDate);
        }
        let notes = self.notes.filter(|notes| !notes.trim().is_empty());
        Ok(SetFields { name, date, notes })
    }
}

fn is_real_date(date: &str) -> bool {
    // Non-ASCII text could split a character when sliced below.
    if !date.is_ascii() {
        return false;
    }
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = &date[range];
        if part.bytes().all(|b| b.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// A fresh 16-hex-character id from the clock, a process counter and `seed`.
#[must_use]
pub fn new_id(seed: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(count.to_le_bytes());
    hasher.update(seed.as_bytes());
    hasher
        .finalize()
        .iter()
        .take(8)
        .fold(String::with_capacity(16), |mut id, byte| {
            let _ = write!(id, "{byte:02x}");
            id
        })
}

#[derive(Serialize, Deserialize)]
struct SetsFile {
    schema_version: u32,
    sets: Vec<Set>,
}

/// All sets, in creation order; use [`SetStore::sets_by_modified`] for display order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetStore {
    sets: Vec<Set>,
}

impl SetStore {
    /// A store with no sets, usable in a `static`.
    #[must_use]
    pub const fn empty() -> Self {
        Self { sets: Vec::new() }
    }

    /// Parses the JSON file; corrupt text or an unknown schema version is an empty store.
    /// Piece items with invalid keys are dropped.
    #[must_use]
    pub fn from_json(text: &str) -> Self {
        let Ok(file) = serde_json::from_str::<SetsFile>(text) else {
            return Self::default();
        };
        if file.schema_version != SCHEMA_VERSION {
            return Self::default();
        }
        let mut sets = file.sets;
        for set in &mut sets {
            set.drop_invalid_pieces();
        }
        Self { sets }
    }

    /// The JSON file text.
    #[must_use]
    pub fn to_json(&self) -> String {
        let file = SetsFile {
            schema_version: SCHEMA_VERSION,
            sets: self.sets.clone(),
        };
        serde_json::to_string(&file).unwrap_or_default()
    }

    /// Reads `path`; a missing or unreadable file is an empty store. Never writes.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map_or_else(|_| Self::default(), |text| Self::from_json(&text))
    }

    /// Writes through a temporary file so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from writing or renaming the file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_atomically(path, &self.to_json())
    }

    /// Sets, most recently modified first; ties by name.
    #[must_use]
    pub fn sets_by_modified(&self) -> Vec<&Set> {
        let mut sets: Vec<&Set> = self.sets.iter().collect();
        sets.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.name.cmp(&b.name))
        });
        sets
    }

    /// All sets, in creation order.
    #[must_use]
    pub fn sets(&self) -> &[Set] {
        &self.sets
    }

    /// Replaces the set with the same id in place, or adds it at the end.
    pub fn put(&mut self, set: Set) {
        match self.get_mut(&set.id) {
            Some(existing) => *existing = set,
            None => self.sets.push(set),
        }
    }

    /// The set with this id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Set> {
        self.sets.iter().find(|set| set.id == id)
    }

    /// Points each piece whose key is not in `library` at the entry with the same path.
    /// `library` holds `(key, path)` pairs. Returns whether anything changed; `modified` stays.
    pub fn relink(&mut self, library: &[(String, String)]) -> bool {
        let mut changed = false;
        for item in self.sets.iter_mut().flat_map(|set| &mut set.items) {
            if let ItemKind::Piece(piece) = &mut item.kind {
                changed |= piece.relink(library);
            }
        }
        changed
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut Set> {
        self.sets.iter_mut().find(|set| set.id == id)
    }

    /// Adds an empty set and returns its id.
    pub fn create(&mut self, fields: SetFields, now: u64) -> String {
        let id = new_id(&fields.name);
        self.sets.push(Set {
            id: id.clone(),
            name: fields.name,
            date: fields.date,
            notes: fields.notes,
            modified: now,
            items: Vec::new(),
            last_opened_item: None,
        });
        id
    }

    /// Replaces the set's fields. False if the set does not exist.
    pub fn update(&mut self, id: &str, fields: SetFields, now: u64) -> bool {
        let Some(set) = self.get_mut(id) else {
            return false;
        };
        set.name = fields.name;
        set.date = fields.date;
        set.notes = fields.notes;
        set.modified = now;
        true
    }

    /// Removes the set. False if it does not exist.
    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.sets.len();
        self.sets.retain(|set| set.id != id);
        self.sets.len() != before
    }

    /// Appends a piece (repeats allowed) and returns the new item's id.
    pub fn add_piece(&mut self, set_id: &str, piece: PieceRef, now: u64) -> Option<String> {
        let seed = piece.key.clone();
        self.push_item(set_id, ItemKind::Piece(piece), &seed, now)
    }

    /// Appends a text note and returns the new item's id.
    pub fn add_note(&mut self, set_id: &str, text: String, now: u64) -> Option<String> {
        let seed = text.clone();
        self.push_item(set_id, ItemKind::Note { text }, &seed, now)
    }

    fn push_item(&mut self, set_id: &str, kind: ItemKind, seed: &str, now: u64) -> Option<String> {
        let set = self.get_mut(set_id)?;
        let id = new_id(seed);
        set.items.push(SetItem {
            id: id.clone(),
            kind,
        });
        set.modified = now;
        Some(id)
    }

    /// Removes one item. False if the set or item does not exist.
    pub fn remove_item(&mut self, set_id: &str, item_id: &str, now: u64) -> bool {
        let Some(set) = self.get_mut(set_id) else {
            return false;
        };
        let before = set.items.len();
        set.items.retain(|item| item.id != item_id);
        if set.items.len() == before {
            return false;
        }
        if set.last_opened_item.as_deref() == Some(item_id) {
            set.last_opened_item = None;
        }
        set.modified = now;
        true
    }

    /// Moves an item by `delta` places, clamped to the list. False if it did not move.
    pub fn move_item(&mut self, set_id: &str, item_id: &str, delta: i32, now: u64) -> bool {
        let Some(set) = self.get_mut(set_id) else {
            return false;
        };
        let Some(from) = set.items.iter().position(|item| item.id == item_id) else {
            return false;
        };
        let last = set.items.len() - 1;
        let to = i64::try_from(from)
            .unwrap_or(0)
            .saturating_add(i64::from(delta))
            .clamp(0, i64::try_from(last).unwrap_or(0));
        let to = usize::try_from(to).unwrap_or(from);
        if to == from {
            return false;
        }
        let item = set.items.remove(from);
        set.items.insert(to, item);
        set.modified = now;
        true
    }

    /// Remembers the item last opened. Does not change `modified`.
    pub fn mark_opened(&mut self, set_id: &str, item_id: &str) -> bool {
        let Some(set) = self.get_mut(set_id) else {
            return false;
        };
        if !set.items.iter().any(|item| item.id == item_id) {
            return false;
        }
        set.last_opened_item = Some(item_id.to_owned());
        true
    }

    /// Ids of the sets holding a piece with this content key.
    #[must_use]
    pub fn sets_containing(&self, key: &str) -> Vec<String> {
        self.sets
            .iter()
            .filter(|set| {
                set.items
                    .iter()
                    .any(|item| matches!(&item.kind, ItemKind::Piece(piece) if piece.key == key))
            })
            .map(|set| set.id.clone())
            .collect()
    }
}
