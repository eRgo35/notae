//! Drawings on a set's pieces. Each set item has at most one overlay, stored as
//! `<dir>/<set id>/<item id>.json`; the library PDF is never written. Coordinates and widths
//! are fractions (0–1) of the page's width and height, so they survive rotation and zoom.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::favorites::write_atomically;

const SCHEMA_VERSION: u64 = 1;

/// How a stroke is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    /// An opaque pen line.
    Pen,
    /// A wide translucent line.
    Highlighter,
}

/// One pen or highlighter line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Stroke {
    /// Pen or highlighter.
    pub tool: Tool,
    /// ARGB colour.
    pub color: u32,
    /// Line width as a fraction of the page width.
    pub width: f32,
    /// `x0, y0, x1, y1, …` as fractions of the page.
    pub points: Vec<f32>,
}

/// A typed note placed on the page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct TextBox {
    /// Left edge, as a fraction of the page width.
    pub x: f32,
    /// Top edge, as a fraction of the page height.
    pub y: f32,
    /// The text.
    pub text: String,
    /// Text size as a fraction of the page width.
    pub size: f32,
}

/// The drawings on one page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct PageOverlay {
    /// Zero-based page index.
    pub page: u32,
    /// Lines, oldest first.
    pub strokes: Vec<Stroke>,
    /// Text boxes.
    pub texts: Vec<TextBox>,
}

/// The drawings on one set item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Overlay {
    /// The content key of the PDF they were drawn on; a different key means the PDF changed.
    pub content_key: String,
    /// Pages with drawings.
    pub pages: Vec<PageOverlay>,
}

impl Overlay {
    /// No strokes and no text on any page.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pages
            .iter()
            .all(|page| page.strokes.is_empty() && page.texts.is_empty())
    }
}

#[derive(Serialize, Deserialize)]
struct OverlayFile {
    schema_version: u64,
    #[serde(flatten)]
    overlay: Overlay,
}

/// The overlay files under one folder.
#[derive(Debug, Clone)]
pub struct OverlayStore {
    dir: PathBuf,
}

/// Set and item ids are generated hex strings; anything else (from an imported file) could
/// name a path outside the folder.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl OverlayStore {
    /// A store keeping its files under `dir`, created on the first save.
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn set_dir(&self, set_id: &str) -> Option<PathBuf> {
        safe_id(set_id).then(|| self.dir.join(set_id))
    }

    fn file(&self, set_id: &str, item_id: &str) -> Option<PathBuf> {
        if !safe_id(item_id) {
            return None;
        }
        self.set_dir(set_id)
            .map(|dir| dir.join(format!("{item_id}.json")))
    }

    /// The item's drawings; `None` when it has none or its file is unreadable.
    #[must_use]
    pub fn get(&self, set_id: &str, item_id: &str) -> Option<Overlay> {
        let text = fs::read_to_string(self.file(set_id, item_id)?).ok()?;
        let file: OverlayFile = serde_json::from_str(&text).ok()?;
        Some(file.overlay)
    }

    /// Whether the item has drawings.
    #[must_use]
    pub fn has(&self, set_id: &str, item_id: &str) -> bool {
        self.get(set_id, item_id).is_some()
    }

    /// Saves the item's drawings; an empty overlay removes the file.
    ///
    /// # Errors
    ///
    /// An invalid id, or the I/O error from writing the file.
    pub fn put(&self, set_id: &str, item_id: &str, overlay: &Overlay) -> io::Result<()> {
        let path = self
            .file(set_id, item_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid overlay id"))?;
        if overlay.is_empty() {
            return match fs::remove_file(&path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OverlayFile {
            schema_version: SCHEMA_VERSION,
            overlay: overlay.clone(),
        };
        let text = serde_json::to_string(&file).map_err(io::Error::other)?;
        write_atomically(&path, &text)
    }

    /// Deletes the item's drawings, if any.
    pub fn remove_item(&self, set_id: &str, item_id: &str) {
        if let Some(path) = self.file(set_id, item_id) {
            let _ = fs::remove_file(path);
        }
    }

    /// Deletes every drawing in the set.
    pub fn remove_set(&self, set_id: &str) {
        if let Some(dir) = self.set_dir(set_id) {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// Every item of the set with drawings, as `(item id, overlay)`, sorted by item id.
    #[must_use]
    pub fn all_for(&self, set_id: &str) -> Vec<(String, Overlay)> {
        let Some(entries) = self.set_dir(set_id).and_then(|dir| fs::read_dir(dir).ok()) else {
            return Vec::new();
        };
        let mut all: Vec<(String, Overlay)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let item_id = name.strip_suffix(".json")?.to_owned();
                let overlay = self.get(set_id, &item_id)?;
                Some((item_id, overlay))
            })
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }
}
