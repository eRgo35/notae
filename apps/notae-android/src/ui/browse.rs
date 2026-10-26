//! Library sections, composer navigation and section filtering over projected rows.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use crate::ui::search::SearchRow;

/// One entry of the library side panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LibrarySection {
    /// Every indexed score.
    #[default]
    All,
    /// Scores opened recently, newest first.
    Recent,
    /// Scores by one composer, matched case-insensitively.
    Composer(String),
}

impl LibrarySection {
    /// Stable key passed through the Slint layer.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::Recent => "recent".to_owned(),
            Self::Composer(name) => format!("composer:{name}"),
        }
    }

    /// Parses a key; anything unknown, including the removed `needs-details` and `text-pending`
    /// sections, is the All section.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "recent" => Self::Recent,
            _ => key
                .strip_prefix("composer:")
                .filter(|name| !name.trim().is_empty())
                .map_or(Self::All, |name| Self::Composer(name.to_owned())),
        }
    }

    /// Large title shown above the score grid.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::All => crate::language::pick("All scores", "Wszystkie utwory").to_owned(),
            Self::Recent => crate::language::pick("Recent", "Ostatnie").to_owned(),
            Self::Composer(name) => name.clone(),
        }
    }
}

/// A composer and how many scores carry that name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerCount {
    /// Spelling from the first score in library order.
    pub name: String,
    /// Number of scores.
    pub count: usize,
}

/// A fixed side-panel entry with its count and icon kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionView {
    /// Value of [`LibrarySection::key`].
    pub key: String,
    /// Visible label.
    pub label: String,
    /// Number of scores in the section.
    pub count: usize,
    /// `Glyph` kind in `components.slint`.
    pub kind: &'static str,
}

/// Composers sorted case-insensitively; case variants of one name are merged.
#[must_use]
pub fn composers(rows: &[SearchRow]) -> Vec<ComposerCount> {
    let mut by_name: BTreeMap<String, ComposerCount> = BTreeMap::new();
    for name in rows
        .iter()
        .filter_map(|row| row.composer_or_artist.as_deref())
    {
        by_name
            .entry(name.to_lowercase())
            .or_insert_with(|| ComposerCount {
                name: name.to_owned(),
                count: 0,
            })
            .count += 1;
    }
    by_name.into_values().collect()
}

/// Rows that belong to `section`; `recent` holds content keys, newest first.
#[must_use]
pub fn section_rows(
    rows: &[SearchRow],
    section: &LibrarySection,
    recent: &[String],
) -> Vec<SearchRow> {
    match section {
        LibrarySection::All => rows.to_vec(),
        LibrarySection::Recent => recent
            .iter()
            .filter_map(|key| rows.iter().find(|row| &row.note_key == key))
            .cloned()
            .collect(),
        LibrarySection::Composer(name) => {
            let name = name.to_lowercase();
            rows.iter()
                .filter(|row| {
                    row.composer_or_artist
                        .as_deref()
                        .is_some_and(|composer| composer.to_lowercase() == name)
                })
                .cloned()
                .collect()
        }
    }
}

/// Where a row sits among its section's rows, with the keys of the rows either side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbours {
    pub previous: Option<String>,
    pub next: Option<String>,
    /// One-based index of the row.
    pub position: usize,
    pub total: usize,
}

/// The neighbours of `key` in `rows`, without wrapping; `None` when `key` is not in `rows`.
#[must_use]
pub fn neighbours(rows: &[SearchRow], key: &str) -> Option<Neighbours> {
    let index = rows.iter().position(|row| row.note_key == key)?;
    Some(Neighbours {
        previous: index
            .checked_sub(1)
            .map(|previous| rows[previous].note_key.clone()),
        next: rows.get(index + 1).map(|row| row.note_key.clone()),
        position: index + 1,
        total: rows.len(),
    })
}

/// The fixed side-panel sections with their counts.
#[must_use]
pub fn sections(rows: &[SearchRow], recent: &[String]) -> Vec<SectionView> {
    [
        (LibrarySection::All, "library"),
        (LibrarySection::Recent, "schedule"),
    ]
    .into_iter()
    .map(|(section, kind)| SectionView {
        key: section.key(),
        label: section.title(),
        count: section_rows(rows, &section, recent).len(),
        kind,
    })
    .collect()
}
