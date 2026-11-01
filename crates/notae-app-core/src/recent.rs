//! The most recently opened scores, stored as content keys in a private text file.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;

/// How many scores the Recent section remembers.
pub const RECENT_LIMIT: usize = 30;

/// Content keys (lowercase SHA-256 hex) of recently opened scores, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentScores {
    keys: Vec<String>,
}

pub(crate) fn is_content_key(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

impl RecentScores {
    /// Parses one key per line; invalid lines and repeats are dropped.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut recent = Self::default();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|line| is_content_key(line))
        {
            if !recent.keys.iter().any(|key| key == line) && recent.keys.len() < RECENT_LIMIT {
                recent.keys.push(line.to_owned());
            }
        }
        recent
    }

    /// One key per line, newest first.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.keys.iter().map(|key| format!("{key}\n")).collect()
    }

    /// Reads `path`; a missing or unreadable file is an empty list.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map_or_else(|_| Self::default(), |text| Self::from_text(&text))
    }

    /// Writes through a temporary file so a crash never leaves half a list.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from writing or renaming the file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, self.to_text())?;
        fs::rename(temporary, path)
    }

    /// Moves `key` to the front. Keys that are not content keys are ignored.
    pub fn record(&mut self, key: &str) {
        if !is_content_key(key) {
            return;
        }
        self.keys.retain(|existing| existing != key);
        self.keys.insert(0, key.to_owned());
        self.keys.truncate(RECENT_LIMIT);
    }

    /// Keys, newest first.
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}
