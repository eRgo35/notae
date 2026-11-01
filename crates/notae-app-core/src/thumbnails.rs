//! Private cache of first-page thumbnails, one PNG per content key.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::pdf_pages::PdfError;

/// Width in pixels thumbnails are rendered at.
pub const THUMBNAIL_WIDTH_PX: i32 = 400;

/// Directory of `<sha256>.png` files under the app's private storage.
#[derive(Debug, Clone)]
pub struct ThumbnailCache {
    dir: PathBuf,
}

fn is_content_key(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

impl ThumbnailCache {
    /// Creates the directory if needed.
    ///
    /// # Errors
    ///
    /// Returns the error from creating the directory.
    pub fn new(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Where the thumbnail for `note_key` lives; `None` for anything but a content key.
    #[must_use]
    pub fn path_for(&self, note_key: &str) -> Option<PathBuf> {
        is_content_key(note_key).then(|| self.dir.join(format!("{note_key}.png")))
    }

    /// The thumbnail file, when it has been rendered.
    #[must_use]
    pub fn existing(&self, note_key: &str) -> Option<PathBuf> {
        self.path_for(note_key).filter(|path| path.is_file())
    }

    /// The content keys whose thumbnail has been rendered, read from the directory in one pass,
    /// so a snapshot of many rows does not check each file. Empty when the directory cannot be
    /// read, as [`Self::existing`] finds nothing then.
    #[must_use]
    pub fn rendered_keys(&self) -> HashSet<String> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return HashSet::new();
        };
        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let key = name.strip_suffix(".png")?;
                is_content_key(key).then(|| key.to_owned())
            })
            .collect()
    }

    /// Returns the cached thumbnail, rendering it with `render` when missing.
    ///
    /// `render` writes a PNG to the path it is given and returns `Ok(true)`, or
    /// `Ok(false)` when the document has nothing to show. Anything but `Ok(true)`
    /// removes the partial file.
    ///
    /// # Errors
    ///
    /// Returns the render error, or a platform error when the file cannot be moved into place.
    pub fn ensure(
        &self,
        note_key: &str,
        render: impl FnOnce(&Path) -> Result<bool, PdfError>,
    ) -> Result<Option<PathBuf>, PdfError> {
        let Some(path) = self.path_for(note_key) else {
            return Ok(None);
        };
        if path.is_file() {
            return Ok(Some(path));
        }
        let partial = path.with_extension("png.part");
        let outcome = render(&partial);
        if !matches!(outcome, Ok(true)) {
            let _ = fs::remove_file(&partial);
            return outcome.map(|_| None);
        }
        if let Err(error) = fs::File::open(&partial).and_then(|file| file.sync_all()) {
            let _ = fs::remove_file(&partial);
            return Err(PdfError::Platform(format!(
                "cannot flush thumbnail: {error}"
            )));
        }
        fs::rename(&partial, &path).map_err(|error| {
            let _ = fs::remove_file(&partial);
            PdfError::Platform(format!("cannot store thumbnail: {error}"))
        })?;
        Ok(Some(path))
    }

    /// Removes an undecodable thumbnail (for example one truncated by an unclean shutdown) so
    /// the next rescan renders it again.
    pub fn discard(&self, note_key: &str) {
        if let Some(path) = self.path_for(note_key) {
            let _ = fs::remove_file(path);
        }
    }

    /// Deletes thumbnails whose key is not in `keep`, and stray partial files.
    ///
    /// # Errors
    ///
    /// Returns the error from reading the directory.
    pub fn prune(&self, keep: &HashSet<String>) -> io::Result<usize> {
        let mut removed = 0;
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let stale = match name.strip_suffix(".png") {
                Some(key) => !keep.contains(key),
                None => name.ends_with(".png.part"),
            };
            if stale && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}
