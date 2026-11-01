//! Private UI preferences: library layout, per-orientation sidebar visibility, and the library
//! section and details score on display. The language is not one of them: Android keeps the
//! per-app language (AppCompat), and a `language` line from an older build is ignored.
//!
//! Stored as `key=value` lines next to the recent list, using the same
//! temporary-file-and-rename write as [`crate::recent`].

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;

/// How the library lists scores. Applies to every section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
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
    /// Grid or list.
    pub library_view: LibraryView,
    /// Whether the sidebar is collapsed in portrait.
    pub sidebar_collapsed_portrait: bool,
    /// Whether the sidebar is collapsed in landscape.
    pub sidebar_collapsed_landscape: bool,
    /// [`crate::ui::browse::LibrarySection::key`] of the section on display, so Notae comes back
    /// to the same set after Android closes it while the PDF app is in front.
    pub section: String,
    /// [`crate::ui::browse::row_id`] of the score in the details pane, restored with
    /// [`Self::section`] for the same reason; `None` when the details are closed.
    pub selection: Option<String>,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            library_view: LibraryView::Grid,
            sidebar_collapsed_portrait: true,
            sidebar_collapsed_landscape: false,
            section: "all".to_owned(),
            selection: None,
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
                "section" if !value.is_empty() => value.clone_into(&mut prefs.section),
                "selection" => prefs.selection = Some(value.to_owned()).filter(|id| !id.is_empty()),
                _ => {}
            }
        }
        prefs
    }

    /// The stored representation.
    #[must_use]
    pub fn to_text(&self) -> String {
        format!(
            "library_view={}\nsidebar_collapsed_portrait={}\nsidebar_collapsed_landscape={}\nsection={}\nselection={}\n",
            self.library_view.key(),
            self.sidebar_collapsed_portrait,
            self.sidebar_collapsed_landscape,
            self.section,
            self.selection.as_deref().unwrap_or_default(),
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

/// The language picked in Settings, or in Android's per-app language setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LanguageChoice {
    /// Follow the system language.
    #[default]
    System,
    /// English, whatever the system language.
    English,
    /// Polish, whatever the system language.
    Polish,
}

impl LanguageChoice {
    /// The BCP 47 tag Android stores for this choice; empty for [`Self::System`].
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::System => "",
            Self::English => "en",
            Self::Polish => "pl",
        }
    }

    /// Reads an Android locale tag. Unsupported languages mean "follow the system".
    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        match primary_language(tag) {
            "en" => Self::English,
            "pl" => Self::Polish,
            _ => Self::System,
        }
    }

    /// Position in the Settings list.
    #[must_use]
    pub const fn index(self) -> i32 {
        match self {
            Self::System => 0,
            Self::English => 1,
            Self::Polish => 2,
        }
    }

    /// Reads a Settings list position; unknown positions mean "follow the system".
    #[must_use]
    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::English,
            2 => Self::Polish,
            _ => Self::System,
        }
    }
}

fn primary_language(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or_default()
}
