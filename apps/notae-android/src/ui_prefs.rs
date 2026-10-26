//! Private UI preferences: library layout, per-orientation sidebar visibility and language.
//!
//! Stored as `key=value` lines next to the recent list, using the same
//! temporary-file-and-rename write as [`crate::recent`].

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;

use crate::language::LanguageChoice;

/// How the library lists scores. Applies to every section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LibraryView {
    /// Thumbnail cards.
    #[default]
    Grid,
    /// Dense rows.
    List,
}

impl LibraryView {
    fn key(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "grid" => Some(Self::Grid),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

/// Persisted user choices that are not library data.
///
/// Portrait means the logical window height exceeds its width. Sidebar
/// choices are stored per orientation so collapsing it in portrait never
/// silently changes landscape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPrefs {
    pub library_view: LibraryView,
    pub sidebar_collapsed_portrait: bool,
    pub sidebar_collapsed_landscape: bool,
    /// Used only where Android has no per-app language setting (before Android 13).
    pub language: LanguageChoice,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            library_view: LibraryView::Grid,
            sidebar_collapsed_portrait: true,
            sidebar_collapsed_landscape: false,
            language: LanguageChoice::System,
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

impl UiPrefs {
    /// Parses `key=value` lines. Unknown keys and invalid values are ignored
    /// one by one, so each preference falls back to its own default.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut prefs = Self::default();
        for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
            let (key, value) = (key.trim(), value.trim());
            match key {
                "library_view" => {
                    if let Some(view) = LibraryView::from_key(value) {
                        prefs.library_view = view;
                    }
                }
                "sidebar_collapsed_portrait" => {
                    if let Some(collapsed) = parse_bool(value) {
                        prefs.sidebar_collapsed_portrait = collapsed;
                    }
                }
                "sidebar_collapsed_landscape" => {
                    if let Some(collapsed) = parse_bool(value) {
                        prefs.sidebar_collapsed_landscape = collapsed;
                    }
                }
                "language" => prefs.language = LanguageChoice::from_tag(value),
                _ => {}
            }
        }
        prefs
    }

    /// The stored representation.
    #[must_use]
    pub fn to_text(&self) -> String {
        format!(
            "library_view={}\nsidebar_collapsed_portrait={}\nsidebar_collapsed_landscape={}\nlanguage={}\n",
            self.library_view.key(),
            self.sidebar_collapsed_portrait,
            self.sidebar_collapsed_landscape,
            self.language.tag(),
        )
    }

    /// Reads `path`; a missing or unreadable file yields the defaults.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map_or_else(|_| Self::default(), |text| Self::from_text(&text))
    }

    /// Writes through a temporary file so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from writing or renaming the file. The caller
    /// keeps its in-memory preferences either way.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, self.to_text())?;
        fs::rename(temporary, path)
    }
}
