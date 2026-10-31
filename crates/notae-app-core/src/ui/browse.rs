//! Library sections, composer navigation and section filtering over projected rows.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;

use crate::favorites::PieceRef;
use crate::sets::{ItemKind, Set, SetStore};
use crate::text::{Edited, PositionOf, RowStatus, SectionTitle, SetSubtitle};
use crate::ui::search::SearchRow;

/// One entry of the library side panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LibrarySection {
    /// Every indexed score.
    #[default]
    All,
    /// Scores opened recently, newest first.
    Recent,
    /// Scores the user starred, newest first.
    Favorites,
    /// The list of singing sets, shown as folder cards or rows.
    Sets,
    /// One singing set's items, by set id.
    Set(String),
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
            Self::Favorites => "favorites".to_owned(),
            Self::Sets => "sets".to_owned(),
            Self::Set(id) => format!("set:{id}"),
            Self::Composer(name) => format!("composer:{name}"),
        }
    }

    /// Parses a key; anything unknown, including the removed `needs-details` and `text-pending`
    /// sections, is the All section.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "recent" => Self::Recent,
            "favorites" => Self::Favorites,
            "sets" => Self::Sets,
            _ => {
                if let Some(id) = key.strip_prefix("set:") {
                    if id.trim().is_empty() {
                        Self::All
                    } else {
                        Self::Set(id.to_owned())
                    }
                } else {
                    key.strip_prefix("composer:")
                        .filter(|name| !name.trim().is_empty())
                        .map_or(Self::All, |name| Self::Composer(name.to_owned()))
                }
            }
        }
    }

    /// Large title shown above the score grid. A set's title is its name; a set that is not in
    /// `sets` is titled like the list of sets.
    #[must_use]
    pub fn title(&self, sets: &SetStore) -> SectionTitle {
        match self {
            Self::All => SectionTitle::All,
            Self::Recent => SectionTitle::Recent,
            Self::Favorites => SectionTitle::Favorites,
            Self::Set(id) => sets.get(id).map_or(SectionTitle::Sets, |set| {
                SectionTitle::Set(set.name.clone())
            }),
            Self::Sets => SectionTitle::Sets,
            Self::Composer(name) => SectionTitle::Composer(name.clone()),
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
    /// The section's label.
    pub label: SectionTitle,
    /// Number of scores in the section.
    pub count: usize,
    /// The section's glyph kind, as the UI's `Glyph` component names it.
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

/// What the user-kept lists contribute to the sections.
#[derive(Debug, Clone, Copy)]
pub struct SectionContext<'a> {
    /// Content keys of recently opened scores, newest first.
    pub recent: &'a [String],
    /// Starred scores, newest first.
    pub favorites: &'a [PieceRef],
    /// Singing sets.
    pub sets: &'a SetStore,
}

impl Default for SectionContext<'_> {
    fn default() -> Self {
        static NO_SETS: SetStore = SetStore::empty();
        Self {
            recent: &[],
            favorites: &[],
            sets: &NO_SETS,
        }
    }
}

/// Rows that belong to `section`, in the section's own order.
#[must_use]
pub fn section_rows(
    rows: &[SearchRow],
    section: &LibrarySection,
    context: &SectionContext,
) -> Vec<SearchRow> {
    match section {
        LibrarySection::All => rows.to_vec(),
        LibrarySection::Recent => context
            .recent
            .iter()
            .filter_map(|key| rows.iter().find(|row| &row.note_key == key))
            .cloned()
            .collect(),
        LibrarySection::Favorites => context
            .favorites
            .iter()
            .filter_map(|piece| rows.iter().find(|row| row.note_key == piece.key))
            .cloned()
            .collect(),
        // Sets are listed as folder cards (see [`set_cards`]).
        LibrarySection::Sets => Vec::new(),
        LibrarySection::Set(id) => context.sets.get(id).map_or_else(Vec::new, |set| {
            set_entries(rows, set)
                .into_iter()
                .filter_map(|entry| match entry {
                    SetEntry::Piece(row) => Some(row),
                    SetEntry::Note { .. } => None,
                })
                .collect()
        }),
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

/// One item of a set, numbered from 1 across pieces and notes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // A short list built per redraw; boxing the row buys nothing.
pub enum SetEntry {
    /// A piece, numbered by `row.position`; a PDF no longer in the library is a placeholder row
    /// with `missing` set.
    Piece(SearchRow),
    /// A text note between pieces.
    Note {
        /// One-based position in the set.
        position: usize,
        /// The set item's id.
        item_id: String,
        /// The note's text.
        text: String,
    },
}

/// The set's items in set order, each piece matched to its library row by content key.
#[must_use]
pub fn set_entries(rows: &[SearchRow], set: &Set) -> Vec<SetEntry> {
    set.items
        .iter()
        .zip(1..)
        .map(|(item, position)| match &item.kind {
            ItemKind::Piece(piece) => {
                let mut row = rows
                    .iter()
                    .find(|row| row.note_key == piece.key)
                    .cloned()
                    .unwrap_or_else(|| missing_row(piece));
                row.item_id = Some(item.id.clone());
                row.position = Some(position);
                SetEntry::Piece(row)
            }
            ItemKind::Note { text } => SetEntry::Note {
                position,
                item_id: item.id.clone(),
                text: text.clone(),
            },
        })
        .collect()
}

/// The stand-in for a piece whose PDF is not in the library, titled by its stored file name.
fn missing_row(piece: &PieceRef) -> SearchRow {
    let title = Path::new(&piece.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&piece.path);
    SearchRow {
        id: None,
        title: title.to_owned(),
        composer_or_artist: None,
        tags: Vec::new(),
        status: RowStatus::MissingFromLibrary,
        relative_path: piece.path.clone(),
        note_key: piece.key.clone(),
        text_pending: false,
        title_origin: notae_domain::MetadataOrigin::Unset,
        composer_origin: notae_domain::MetadataOrigin::Unset,
        text_snippet: None,
        item_id: None,
        position: None,
        missing: true,
    }
}

/// An open set's subtitle: its date, and how many pieces and notes it holds.
#[must_use]
pub fn set_subtitle(set: &Set) -> SetSubtitle {
    let notes = set
        .items
        .iter()
        .filter(|item| matches!(item.kind, ItemKind::Note { .. }))
        .count();
    SetSubtitle {
        date: set.date.clone(),
        pieces: set.items.len() - notes,
        notes,
    }
}

/// The id that selects `row`: its set item id inside a set, so each copy of a repeated piece is
/// its own row, and its note key everywhere else.
/// The row's selection id: its set item id inside a set, otherwise its content key.
#[must_use]
pub fn row_id(row: &SearchRow) -> &str {
    row.item_id.as_deref().unwrap_or(&row.note_key)
}

/// Where a row sits among its section's rows, with the selection ids ([`row_id`]) of the rows
/// either side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbours {
    /// The previous row's selection id.
    pub previous: Option<String>,
    /// The next row's selection id.
    pub next: Option<String>,
    /// One-based index of the row.
    pub position: usize,
    /// Rows in the section.
    pub total: usize,
}

impl Neighbours {
    /// The row's place, shown in the details pane as "2 of 5".
    #[must_use]
    pub const fn position_of(&self) -> PositionOf {
        PositionOf {
            position: self.position,
            total: self.total,
        }
    }
}

/// The neighbours of the row selected by `id` in `rows`, without wrapping and skipping rows
/// missing from the library; `None` when no row has that [`row_id`].
#[must_use]
pub fn neighbours(rows: &[SearchRow], id: &str) -> Option<Neighbours> {
    let index = rows.iter().position(|row| row_id(row) == id)?;
    let selectable = |row: &&SearchRow| !row.missing;
    Some(Neighbours {
        previous: rows
            .iter()
            .take(index)
            .rev()
            .find(selectable)
            .map(|row| row_id(row).to_owned()),
        next: rows
            .iter()
            .skip(index + 1)
            .find(selectable)
            .map(|row| row_id(row).to_owned()),
        position: index + 1,
        total: rows.len(),
    })
}

/// The fixed side-panel sections with their counts.
#[must_use]
pub fn sections(rows: &[SearchRow], context: &SectionContext) -> Vec<SectionView> {
    [
        (LibrarySection::All, "library"),
        (LibrarySection::Recent, "schedule"),
        (LibrarySection::Favorites, "star"),
        (LibrarySection::Sets, "folder"),
    ]
    .into_iter()
    .map(|(section, kind)| SectionView {
        key: section.key(),
        label: section.title(context.sets),
        count: if section == LibrarySection::Sets {
            context.sets.sets_by_modified().len()
        } else {
            section_rows(rows, &section, context).len()
        },
        kind,
    })
    .collect()
}

/// One folder card of the Sets section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCard {
    /// The set's id.
    pub id: String,
    /// The set's name.
    pub name: String,
    /// The set's ISO `YYYY-MM-DD` date, if it has one.
    pub date: Option<String>,
    /// How many items (pieces and notes) the set holds.
    pub count: usize,
    /// When the set was last edited, relative to now.
    pub edited: Edited,
}

/// The folder cards of the Sets section, most recently edited first. `now` is Unix seconds.
#[must_use]
pub fn set_cards(sets: &SetStore, now: u64) -> Vec<SetCard> {
    sets.sets_by_modified()
        .into_iter()
        .map(|set| SetCard {
            id: set.id.clone(),
            name: set.name.clone(),
            date: set.date.clone(),
            count: set.items.len(),
            edited: edited(set.modified, now),
        })
        .collect()
}

/// Today, yesterday, 2 to 6 days ago, or else the day it happened. `modified` and `now` are Unix
/// seconds; days are counted in UTC.
#[must_use]
pub fn edited(modified: u64, now: u64) -> Edited {
    const DAY: u64 = 86_400;
    let days = (now / DAY).saturating_sub(modified / DAY);
    match days {
        0 => Edited::Today,
        1 => Edited::Yesterday,
        2..=6 => Edited::DaysAgo(u32::try_from(days).unwrap_or(u32::MAX)),
        _ => {
            let (year, month, day) = civil_from_days(modified / DAY);
            Edited::On(format!("{year:04}-{month:02}-{day:02}"))
        }
    }
}

/// The calendar date `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
pub(crate) fn civil_from_days(days: u64) -> (i64, u32, u32) {
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}
