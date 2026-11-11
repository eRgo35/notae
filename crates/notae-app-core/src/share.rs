//! The shareable set file (`.notae-set`) and the plain-text list of a set.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};

use crate::backup::{APP, BackupError, SCHEMA_VERSION};
use crate::sets::{ItemKind, Set};
use crate::ui::search::SearchRow;

/// What the sender's library knew about a shared score, for matching it in the recipient's library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPiece {
    /// The score's number in the sender's library.
    pub number: Option<String>,
    /// The score's title.
    pub title: String,
    /// The score's composer.
    pub composer: Option<String>,
}

/// A parsed shared set file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedSet {
    /// The shared set.
    pub set: Set,
    /// Per piece item (by item id), what the sender knew about the score.
    pub pieces: HashMap<String, SharedPiece>,
}

/// The title shown for a score: the library's, or the stored path's file stem when the library lacks it.
fn piece_info(rows: &[SearchRow], key: &str, path: &str) -> SharedPiece {
    match rows.iter().find(|row| row.note_key == key) {
        Some(row) => SharedPiece {
            number: row.id.clone(),
            title: row.title.clone(),
            composer: row.composer_or_artist.clone(),
        },
        None => SharedPiece {
            number: None,
            title: Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(path)
                .to_owned(),
            composer: None,
        },
    }
}

/// The shared set file text: `{"schema_version":2,"app":"notae","kind":"set","set":{…}}`. Each piece carries its
/// number, title and composer. Set notes are included only with `with_notes`; score notes, favorites and the
/// last opened item are never included.
#[must_use]
pub fn share_json(set: &Set, rows: &[SearchRow], with_notes: bool) -> String {
    let mut shared = set.clone();
    shared.last_opened_item = None;
    if !with_notes {
        for item in &mut shared.items {
            item.set_note = None;
        }
    }
    let mut value = serde_json::to_value(&shared).unwrap_or(Value::Null);
    if let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) {
        for (out, item) in items.iter_mut().zip(&set.items) {
            let (ItemKind::Piece(piece), Some(out)) = (&item.kind, out.as_object_mut()) else {
                continue;
            };
            let info = piece_info(rows, &piece.key, &piece.path);
            if let Some(number) = info.number {
                out.insert("number".to_owned(), Value::String(number));
            }
            out.insert("title".to_owned(), Value::String(info.title));
            if let Some(composer) = info.composer {
                out.insert("composer".to_owned(), Value::String(composer));
            }
        }
    }
    json!({"schema_version": SCHEMA_VERSION, "app": APP, "kind": "set", "set": value}).to_string()
}

/// A plain list of the set for people without Notae: a header line, then pieces numbered by their order among
/// pieces (`1. 123 – Title (Composer)`) and text notes unnumbered, indented. With `with_notes`, a piece's set note
/// follows it as an indented `– note` line.
#[must_use]
pub fn share_text(set: &Set, rows: &[SearchRow], with_notes: bool) -> String {
    let mut out = set.name.clone();
    if let Some(date) = &set.date {
        let _ = write!(out, " · {date}");
    }
    let mut position = 0;
    for item in &set.items {
        match &item.kind {
            ItemKind::Piece(piece) => {
                position += 1;
                let info = piece_info(rows, &piece.key, &piece.path);
                let _ = write!(out, "\n{position}. ");
                if let Some(number) = &info.number {
                    let _ = write!(out, "{number} – ");
                }
                out.push_str(&info.title);
                if let Some(composer) = &info.composer {
                    let _ = write!(out, " ({composer})");
                }
                if with_notes && let Some(note) = &item.set_note {
                    let _ = write!(out, "\n   – {}", note.replace('\n', "\n     "));
                }
            }
            ItemKind::Note { text } => {
                let _ = write!(out, "\n   {}", text.replace('\n', "\n   "));
            }
        }
    }
    out
}

/// `<name>.notae-set`, with `/\:*?"<>|` and control characters in the name replaced by `_`; `set.notae-set` when the
/// name is blank after that.
#[must_use]
pub fn share_file_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        "set.notae-set".to_owned()
    } else {
        format!("{clean}.notae-set")
    }
}

/// Reads the `set` of a shared set file whose header was already checked. Invalid keys and repeated item ids are
/// dropped.
pub(crate) fn parse_shared(value: &Value) -> Result<SharedSet, BackupError> {
    let object = value.get("set").ok_or(BackupError::Invalid)?;
    let mut set: Set = serde_json::from_value(object.clone()).map_err(|_| BackupError::Invalid)?;
    set.drop_invalid_items();
    let text =
        |item: &Value, field: &str| item.get(field).and_then(Value::as_str).map(str::to_owned);
    let mut pieces = HashMap::new();
    for item in object
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(id), Some(title)) = (text(item, "id"), text(item, "title")) else {
            continue;
        };
        if set.items.iter().any(|kept| kept.id == id) {
            pieces.entry(id).or_insert(SharedPiece {
                number: text(item, "number"),
                title,
                composer: text(item, "composer"),
            });
        }
    }
    Ok(SharedSet { set, pieces })
}
