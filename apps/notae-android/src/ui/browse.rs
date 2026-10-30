//! Library sections, composer navigation and section filtering over projected rows.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;

use crate::favorites::PieceRef;
use crate::language::{pick, polish_plural};
use crate::sets::{ItemKind, Set, SetFieldError, SetStore};
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
    pub fn title(&self, sets: &SetStore) -> String {
        match self {
            Self::All => pick("All scores", "Wszystkie utwory").to_owned(),
            Self::Recent => pick("Recent", "Ostatnie").to_owned(),
            Self::Favorites => pick("Favorites", "Ulubione").to_owned(),
            Self::Set(id) => sets.get(id).map_or_else(
                || pick("Sets", "Zestawy").to_owned(),
                |set| set.name.clone(),
            ),
            Self::Sets => pick("Sets", "Zestawy").to_owned(),
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
                    SetEntry::Piece { row, .. } => Some(row),
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
    /// A piece; a PDF no longer in the library is a placeholder row with `missing` set.
    Piece { position: usize, row: SearchRow },
    /// A text note between pieces.
    Note {
        position: usize,
        item_id: String,
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
                SetEntry::Piece { position, row }
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
        status: pick("Missing from library", "Brak w bibliotece").to_owned(),
        relative_path: piece.path.clone(),
        note_key: piece.key.clone(),
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
        item_id: None,
        missing: true,
    }
}

/// An open set's subtitle: its date, then how many pieces and notes it holds; empty parts are
/// left out.
#[must_use]
pub fn set_subtitle(set: &Set) -> String {
    let notes = set
        .items
        .iter()
        .filter(|item| matches!(item.kind, ItemKind::Note { .. }))
        .count();
    let pieces = set.items.len() - notes;
    let mut parts: Vec<String> = set
        .date
        .as_deref()
        .map(format_set_date)
        .into_iter()
        .collect();
    if pieces > 0 {
        parts.push(pick(
            format!("{pieces} {}", if pieces == 1 { "piece" } else { "pieces" }),
            format!(
                "{pieces} {}",
                polish_plural(pieces, "utwór", "utwory", "utworów")
            ),
        ));
    }
    if notes > 0 {
        parts.push(pick(
            format!("{notes} {}", if notes == 1 { "note" } else { "notes" }),
            format!(
                "{notes} {}",
                polish_plural(notes, "notatka", "notatki", "notatek")
            ),
        ));
    }
    parts.join(" · ")
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

/// The folder cards of the Sets section, most recently edited first. `now` is Unix seconds.
#[must_use]
pub fn set_cards(sets: &SetStore, now: u64) -> Vec<crate::SetCard> {
    sets.sets_by_modified()
        .into_iter()
        .map(|set| {
            let count = set.items.len();
            crate::SetCard {
                id: set.id.as_str().into(),
                name: set.name.as_str().into(),
                date: set
                    .date
                    .as_deref()
                    .map_or_else(|| pick("No date", "Bez daty").to_owned(), format_set_date)
                    .into(),
                count: pick(
                    format!("{count} {}", if count == 1 { "item" } else { "items" }),
                    format!(
                        "{count} {}",
                        polish_plural(count, "pozycja", "pozycje", "pozycji")
                    ),
                )
                .into(),
                edited: format_edited(set.modified, now).into(),
            }
        })
        .collect()
}

/// The Sets section's subtitle: how many sets there are and how they are ordered.
#[must_use]
pub fn sets_subtitle(count: usize) -> String {
    pick(
        format!(
            "{count} {} · recently edited first",
            if count == 1 { "set" } else { "sets" }
        ),
        format!(
            "{count} {} · ostatnio edytowane najpierw",
            polish_plural(count, "zestaw", "zestawy", "zestawów")
        ),
    )
}

/// The set dialog's message for rejected fields.
#[must_use]
pub fn set_field_error_text(error: SetFieldError) -> String {
    match error {
        SetFieldError::EmptyName => pick("Name is required", "Nazwa jest wymagana"),
        SetFieldError::InvalidDate => pick(
            "Use a real date as YYYY-MM-DD",
            "Podaj prawdziwą datę jako RRRR-MM-DD",
        ),
    }
    .to_owned()
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const POLISH_MONTHS: [&str; 12] = [
    "sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paź", "lis", "gru",
];

fn format_day(year: i64, month: u32, day: u32) -> String {
    let index = usize::try_from(month.saturating_sub(1))
        .unwrap_or(0)
        .min(11);
    let name = pick(MONTHS[index], POLISH_MONTHS[index]);
    format!("{day} {name} {year}")
}

/// A stored `YYYY-MM-DD` date as "1 Nov 2026" (Polish "1 lis 2026"). Anything else is returned
/// as written.
#[must_use]
pub fn format_set_date(iso: &str) -> String {
    let mut parts = iso.splitn(3, '-');
    let parsed = (|| {
        let year: i64 = parts.next()?.parse().ok()?;
        let month: u32 = parts.next()?.parse().ok()?;
        let day: u32 = parts.next()?.parse().ok()?;
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some((year, month, day))
    })();
    parsed.map_or_else(
        || iso.to_owned(),
        |(year, month, day)| format_day(year, month, day),
    )
}

/// "Edited today", "Edited yesterday", "Edited 3 days ago" within a week, else "Edited 1 Nov 2026".
/// `modified` and `now` are Unix seconds; days are counted in UTC.
#[must_use]
pub fn format_edited(modified: u64, now: u64) -> String {
    const DAY: u64 = 86_400;
    let days = (now / DAY).saturating_sub(modified / DAY);
    match days {
        0 => pick("Edited today", "Edytowano dziś").to_owned(),
        1 => pick("Edited yesterday", "Edytowano wczoraj").to_owned(),
        2..=6 => pick(
            format!("Edited {days} days ago"),
            format!(
                "Edytowano {days} {} temu",
                polish_plural(
                    usize::try_from(days).unwrap_or(usize::MAX),
                    "dzień",
                    "dni",
                    "dni"
                )
            ),
        ),
        _ => {
            let (year, month, day) = civil_from_days(modified / DAY);
            let date = format_day(year, month, day);
            pick(format!("Edited {date}"), format!("Edytowano {date}"))
        }
    }
}

/// The calendar date `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: u64) -> (i64, u32, u32) {
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
