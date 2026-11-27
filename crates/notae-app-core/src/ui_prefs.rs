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

/// Where "Open" shows a score.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum OpenScoresIn {
    /// Notae's own viewer.
    #[default]
    Notae,
    /// The system's default PDF app, as before 0.8.0.
    SystemApp,
}

impl OpenScoresIn {
    fn key(self) -> &'static str {
        match self {
            Self::Notae => "notae",
            Self::SystemApp => "system",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "notae" => Some(Self::Notae),
            "system" => Some(Self::SystemApp),
            _ => None,
        }
    }
}

/// How the viewer lays pages out; `Auto` chooses from the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum ViewerLayout {
    /// From the window: scrolled on a short window, else turned.
    #[default]
    Auto,
    /// One spread at a time, turned.
    Pages,
    /// Pages one under another, scrolled.
    Scroll,
}

impl ViewerLayout {
    fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Pages => "pages",
            Self::Scroll => "scroll",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" => Some(Self::Auto),
            "pages" => Some(Self::Pages),
            "scroll" => Some(Self::Scroll),
            _ => None,
        }
    }
}

/// How many pages the viewer shows side by side; `Auto` chooses from the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum ViewerPages {
    /// From the window: two on a landscape tablet, else one.
    #[default]
    Auto,
    /// One page.
    One,
    /// Two pages side by side.
    Two,
}

impl ViewerPages {
    fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::One => "one",
            Self::Two => "two",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" => Some(Self::Auto),
            "one" => Some(Self::One),
            "two" => Some(Self::Two),
            _ => None,
        }
    }
}

/// The smallest and largest stored viewer percentage.
pub(crate) const VIEWER_PERCENT: std::ops::RangeInclusive<u16> = 25..=400;

/// The viewer's scale; `Auto` chooses from the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum ViewerScale {
    /// From the window: fit width on a short window, else fit page.
    #[default]
    Auto,
    /// The whole page (or spread) on screen.
    FitPage,
    /// The page (or spread) as wide as the screen.
    FitWidth,
    /// The page (or spread) as tall as the screen.
    FitHeight,
    /// A percentage of the printed size.
    Percent {
        /// 25 to 400.
        value: u16,
    },
}

impl ViewerScale {
    fn key(self) -> String {
        match self {
            Self::Auto => "auto".to_owned(),
            Self::FitPage => "fit_page".to_owned(),
            Self::FitWidth => "fit_width".to_owned(),
            Self::FitHeight => "fit_height".to_owned(),
            Self::Percent { value } => value.to_string(),
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" => Some(Self::Auto),
            "fit_page" => Some(Self::FitPage),
            "fit_width" => Some(Self::FitWidth),
            "fit_height" => Some(Self::FitHeight),
            number => number
                .parse::<u16>()
                .ok()
                .filter(|value| VIEWER_PERCENT.contains(value))
                .map(|value| Self::Percent { value }),
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
    /// Where "Open" shows a score.
    pub open_scores_in: OpenScoresIn,
    /// The "Scores now open in Notae" hint was shown. A prefs file from before 0.8.0 lacks the
    /// line, so the hint is due after an update; a new install has no file and never sees it.
    pub viewer_hint_shown: bool,
    /// A stylus has touched the viewer, so from now on a finger moves the page and only the
    /// pen draws.
    pub stylus_seen: bool,
    /// The viewer's layout.
    pub viewer_layout: ViewerLayout,
    /// Pages side by side in the viewer.
    pub viewer_pages: ViewerPages,
    /// The viewer's scale.
    pub viewer_scale: ViewerScale,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            library_view: LibraryView::Grid,
            sidebar_collapsed_portrait: true,
            sidebar_collapsed_landscape: false,
            section: "all".to_owned(),
            selection: None,
            open_scores_in: OpenScoresIn::Notae,
            viewer_hint_shown: false,
            stylus_seen: false,
            viewer_layout: ViewerLayout::Auto,
            viewer_pages: ViewerPages::Auto,
            viewer_scale: ViewerScale::Auto,
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
                "open_scores_in" => {
                    if let Some(choice) = OpenScoresIn::from_key(value) {
                        prefs.open_scores_in = choice;
                    }
                }
                "viewer_hint_shown" => {
                    if let Some(shown) = parse_bool(value) {
                        prefs.viewer_hint_shown = shown;
                    }
                }
                "stylus_seen" => {
                    if let Some(seen) = parse_bool(value) {
                        prefs.stylus_seen = seen;
                    }
                }
                "viewer_layout" => {
                    if let Some(layout) = ViewerLayout::from_key(value) {
                        prefs.viewer_layout = layout;
                    }
                }
                "viewer_pages" => {
                    if let Some(pages) = ViewerPages::from_key(value) {
                        prefs.viewer_pages = pages;
                    }
                }
                "viewer_scale" => {
                    if let Some(scale) = ViewerScale::from_key(value) {
                        prefs.viewer_scale = scale;
                    }
                }
                _ => {}
            }
        }
        prefs
    }

    /// The stored representation.
    #[must_use]
    pub fn to_text(&self) -> String {
        format!(
            "library_view={}\nsidebar_collapsed_portrait={}\nsidebar_collapsed_landscape={}\nsection={}\nselection={}\nopen_scores_in={}\nviewer_hint_shown={}\nstylus_seen={}\nviewer_layout={}\nviewer_pages={}\nviewer_scale={}\n",
            self.library_view.key(),
            self.sidebar_collapsed_portrait,
            self.sidebar_collapsed_landscape,
            self.section,
            self.selection.as_deref().unwrap_or_default(),
            self.open_scores_in.key(),
            self.viewer_hint_shown,
            self.stylus_seen,
            self.viewer_layout.key(),
            self.viewer_pages.key(),
            self.viewer_scale.key(),
        )
    }

    /// Reads `path`; a missing or unreadable file yields the defaults. A missing file is a new
    /// install, which has nothing to be told about, so its viewer hint counts as shown.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_text(&text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self {
                viewer_hint_shown: true,
                ..Self::default()
            },
            Err(_) => Self::default(),
        }
    }

    /// Writes through a temporary file so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from writing or renaming the file. The caller
    /// keeps its in-memory preferences either way.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        crate::favorites::write_atomically(path, &self.to_text())
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
