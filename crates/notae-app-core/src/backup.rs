//! The export file that carries favorites, sets and score notes to another install: build it, parse it, and
//! plan and apply an import.

#![forbid(unsafe_code)]

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::favorites::{Favorites, PieceRef};
use crate::recent::is_content_key;
use crate::score_notes::{ScoreNote, ScoreNotes};
use crate::sets::{Set, SetStore};
use crate::ui::browse::civil_from_days;

const SCHEMA_VERSION: u64 = 2;
/// The first export version, which has no score notes.
const FIRST_SCHEMA_VERSION: u64 = 1;
const APP: &str = "notae";

/// The export file's MIME type.
pub const MIME_TYPE: &str = "application/json";

/// Reserved in the Activity request-code namespace for the "Export sets" save-file picker, so
/// its result is never mistaken for a folder pick, even after Android restarted Notae.
pub const EXPORT_REQUEST_CODE: i32 = 0x4e45;

#[derive(Serialize)]
struct ExportOut<'a> {
    schema_version: u64,
    app: &'a str,
    favorites: &'a [PieceRef],
    sets: &'a [Set],
    score_notes: &'a [ScoreNote],
}

#[derive(Deserialize)]
struct ExportIn {
    favorites: Vec<PieceRef>,
    sets: Vec<Set>,
    #[serde(default)]
    score_notes: Vec<ScoreNote>,
}

/// The contents of a parsed export file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// The exported favorites.
    pub favorites: Favorites,
    /// The exported sets.
    pub sets: Vec<Set>,
    /// The exported score notes; empty for a version 1 file.
    pub score_notes: ScoreNotes,
}

/// Why a file is not an importable export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BackupError {
    /// The file is not a Notae export.
    #[error("this is not a Notae sets file")]
    NotNotae,
    /// The file was written by a newer version of Notae.
    #[error("this sets file is from a newer version of Notae")]
    UnsupportedVersion,
    /// The file is a Notae export but cannot be read.
    #[error("this sets file is damaged")]
    Invalid,
}

/// What importing a backup would do to the current sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    /// Sets whose id is not in the current store.
    pub new_sets: usize,
    /// Current names of the sets the backup replaces.
    pub replaced: Vec<String>,
}

/// The export file text: `{"schema_version":2,"app":"notae","favorites":[…],"sets":[…],"score_notes":[…]}`.
#[must_use]
pub fn export_json(favorites: &Favorites, sets: &SetStore, score_notes: &ScoreNotes) -> String {
    let file = ExportOut {
        schema_version: SCHEMA_VERSION,
        app: APP,
        favorites: favorites.items(),
        sets: sets.sets(),
        score_notes: score_notes.items(),
    };
    serde_json::to_string(&file).unwrap_or_default()
}

/// `notae-sets-YYYY-MM-DD.json` for the UTC date of `now` (Unix seconds).
#[must_use]
pub fn suggested_file_name(now: u64) -> String {
    let (year, month, day) = civil_from_days(now / 86_400);
    format!("notae-sets-{year:04}-{month:02}-{day:02}.json")
}

/// Parses an export file. Invalid keys, blank score notes, repeated favorites, repeated set ids and repeated item
/// ids within a set are dropped.
///
/// # Errors
///
/// `NotNotae` for JSON that is not a Notae export, `UnsupportedVersion` for another schema
/// version, `Invalid` for text that is not JSON or does not have the export's shape.
pub fn parse_export(text: &str) -> Result<Backup, BackupError> {
    let value: Value = serde_json::from_str(text).map_err(|_| BackupError::Invalid)?;
    if value.get("app").and_then(Value::as_str) != Some(APP) {
        return Err(BackupError::NotNotae);
    }
    match value.get("schema_version").and_then(Value::as_u64) {
        Some(FIRST_SCHEMA_VERSION | SCHEMA_VERSION) => {}
        Some(_) => return Err(BackupError::UnsupportedVersion),
        None => return Err(BackupError::Invalid),
    }
    let file: ExportIn = serde_json::from_value(value).map_err(|_| BackupError::Invalid)?;
    let mut ids = HashSet::new();
    let mut sets = file.sets;
    sets.retain(|set| ids.insert(set.id.clone()));
    for set in &mut sets {
        set.drop_invalid_items();
    }
    let mut score_notes = ScoreNotes::default();
    for note in file.score_notes {
        if is_content_key(&note.key) {
            score_notes.set(
                PieceRef {
                    key: note.key,
                    path: note.path,
                },
                &note.text,
            );
        }
    }
    Ok(Backup {
        favorites: Favorites::from_items(file.favorites),
        sets,
        score_notes,
    })
}

/// Counts the backup's new sets and names the current sets it would replace.
#[must_use]
pub fn plan_import(current: &SetStore, backup: &Backup) -> ImportPlan {
    let mut plan = ImportPlan {
        new_sets: 0,
        replaced: Vec::new(),
    };
    for set in &backup.sets {
        match current.get(&set.id) {
            Some(existing) => plan.replaced.push(existing.name.clone()),
            None => plan.new_sets += 1,
        }
    }
    plan
}

/// Adds the backup's new sets, replaces sets with the same id, adds favorites not yet listed
/// at the end, and merges score notes (an imported note replaces the one on the same score).
pub fn apply_import(
    favorites: &mut Favorites,
    sets: &mut SetStore,
    score_notes: &mut ScoreNotes,
    backup: Backup,
) {
    favorites.merge(&backup.favorites);
    score_notes.merge(backup.score_notes);
    for set in backup.sets {
        sets.put(set);
    }
}
