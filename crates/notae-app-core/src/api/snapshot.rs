//! What Kotlin renders: one [`Snapshot`] of everything on screen.
//!
//! The records here mirror the core's own view types in the shape UniFFI carries: named fields
//! on every enum variant, counts as `u32`, no lifetimes. Core types that already have that shape
//! ([`RowStatus`], [`Failure`], [`LibraryView`], [`SetFieldError`], [`PickerRow`]) are exported
//! as they are. Nothing here is display text: Kotlin formats every title, count, date and status
//! from these values.

use crate::sets::SetFieldError;
use crate::state::OperationState;
use crate::storage::StorageError;
use crate::text::{self, Failure, RowStatus};
use crate::ui::browse::{self, LibrarySection};
use crate::ui::search::{SearchRow, split_snippet};
use crate::ui::{OperationBanner, PickerRow, StartupPresentation, StorageStateKindView, UiModel};
use crate::ui_prefs::LibraryView;

/// Everything Kotlin needs to draw the app.
///
/// Snapshots reach Kotlin from more than one thread. Each carries a [`Snapshot::revision`]
/// that grows with every change; Kotlin ignores a snapshot older than the one it shows.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Snapshot {
    /// Grows by one with every change to the state; two snapshots with the same revision are
    /// equal.
    pub revision: u64,
    /// The screen on display.
    pub screen: Screen,
    /// The section's key, as in [`SectionEntry::key`]; `set:<id>` inside a set.
    pub section_key: String,
    /// The large title above the rows.
    pub title: SectionTitle,
    /// The line under the title.
    pub subtitle: Subtitle,
    /// The open set's notes.
    pub set_notes: Option<String>,
    /// The fixed side-panel sections, with their counts.
    pub sections: Vec<SectionEntry>,
    /// The composers, sorted ignoring case.
    pub composers: Vec<ComposerEntry>,
    /// The rows to show: the search results on [`Screen::Search`], the section's scores
    /// otherwise (inside a set, its pieces in set order).
    pub rows: Vec<Row>,
    /// The open set's items, pieces and notes, in set order; empty outside a set and on
    /// [`Screen::Search`].
    pub set_entries: Vec<SetEntry>,
    /// The Sets section's folder cards, most recently edited first.
    pub set_cards: Vec<SetCard>,
    /// The details screen's score, when it is open.
    pub selection: Option<Selection>,
    /// The search field and scope.
    pub search: SearchBox,
    /// The operation banner.
    pub banner: Banner,
    /// Whether "Scan library" is enabled.
    pub rescan_enabled: bool,
    /// What the Settings screen shows.
    pub settings: SettingsInfo,
    /// The saved layout choices.
    pub layout: LayoutPrefs,
    /// The set dialog, while it is open.
    pub set_dialog: Option<SetDialog>,
    /// The delete confirmation, while it is open.
    pub delete_set: Option<DeleteSetConfirmation>,
    /// The metadata editor, while it is open.
    pub metadata_editor: Option<MetadataDraft>,
    /// The "Add to set…" sheet, while it is open.
    pub add_to_set: Option<AddToSetSheet>,
    /// The add picker, while it is open over a set.
    pub add_picker: Option<AddPicker>,
    /// Why the last PDF could not be opened; cleared by the next successful open.
    pub pdf_open_error: Option<PdfOpenError>,
    /// Neither favorites nor sets exist yet, so the setup screen may offer "Import from a
    /// previous Notae".
    pub import_available: bool,
}

/// The screen on display. The details screen is [`Snapshot::selection`] over the library.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Screen {
    /// A saved library and its first rows are being restored; show "Loading library…".
    Loading,
    /// No library is open.
    Setup {
        /// Why.
        setup: SetupScreen,
    },
    /// The library.
    Library,
    /// The search screen.
    Search,
    /// The settings screen.
    Settings,
}

/// Why no library is open.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SetupScreen {
    /// No folder was chosen yet.
    Unconfigured,
    /// The saved folder can no longer be read; offer to pick it again.
    PermissionLost {
        /// The folder's display name.
        folder: String,
    },
    /// Opening the library failed; show the error above the folder button.
    Error {
        /// What failed.
        error: SetupError,
    },
}

/// Why opening the library failed ([`StorageError`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SetupError {
    /// The picker was dismissed.
    Cancelled,
    /// The choice was not a folder.
    NotADirectory,
    /// The folder's permission was revoked.
    PermissionLost,
    /// The platform refused the permission.
    PermissionDenied,
    /// Anything else, such as an unreadable local index.
    Other {
        /// The platform's or the database's own message.
        detail: String,
    },
}

impl From<&StorageError> for SetupError {
    fn from(error: &StorageError) -> Self {
        match error {
            StorageError::Cancelled => Self::Cancelled,
            StorageError::NotADirectory => Self::NotADirectory,
            StorageError::PermissionLost => Self::PermissionLost,
            StorageError::PermissionDenied => Self::PermissionDenied,
            StorageError::Other(detail) => Self::Other {
                detail: detail.clone(),
            },
        }
    }
}

/// A section's title ([`text::SectionTitle`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SectionTitle {
    /// "All scores".
    All,
    /// "Recent".
    Recent,
    /// "Favorites".
    Favorites,
    /// "Sets"; also the title of a set that no longer exists.
    Sets,
    /// A composer's section.
    Composer {
        /// The composer's name.
        name: String,
    },
    /// An open set.
    Set {
        /// The set's name.
        name: String,
    },
}

impl From<text::SectionTitle> for SectionTitle {
    fn from(title: text::SectionTitle) -> Self {
        match title {
            text::SectionTitle::All => Self::All,
            text::SectionTitle::Recent => Self::Recent,
            text::SectionTitle::Favorites => Self::Favorites,
            text::SectionTitle::Sets => Self::Sets,
            text::SectionTitle::Composer(name) => Self::Composer { name },
            text::SectionTitle::Set(name) => Self::Set { name },
        }
    }
}

/// The line under the title.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Subtitle {
    /// "N scores".
    Scores {
        /// The number of rows.
        count: u32,
    },
    /// "N sets · recently edited first".
    Sets {
        /// The number of sets.
        count: u32,
    },
    /// An open set's date, then its pieces and notes ([`text::SetSubtitle`]).
    Set {
        /// ISO `YYYY-MM-DD`, if the set has a date.
        date: Option<String>,
        /// Pieces, counting each copy of a repeated piece.
        pieces: u32,
        /// Text notes.
        notes: u32,
    },
}

/// A fixed side-panel section.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SectionEntry {
    /// The key to pass back in [`Action::SelectSection`](super::Action::SelectSection):
    /// `all`, `recent`, `favorites` or `sets`.
    pub key: String,
    /// The section's label.
    pub title: SectionTitle,
    /// Scores in the section; sets for `sets`.
    pub count: u32,
}

/// A composer in the side panel.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComposerEntry {
    /// The key to pass back in [`Action::SelectSection`](super::Action::SelectSection).
    pub key: String,
    /// The composer's name, as the first score in library order spells it.
    pub name: String,
    /// The composer's scores.
    pub count: u32,
}

/// One score row or card.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Row {
    /// The id that selects this row: its set item id inside a set, so each copy of a repeated
    /// piece is its own row, and its content key elsewhere. Stable; use it as the list key.
    pub selection_id: String,
    /// The score's content key.
    pub note_key: String,
    /// The score number, when one is assigned.
    pub number: Option<String>,
    /// The title.
    pub title: String,
    /// The composer or artist, when known.
    pub composer: Option<String>,
    /// The tags, in their stable order.
    pub tags: Vec<String>,
    /// The indexing state, or missing from the library.
    pub status: RowStatus,
    /// The PDF has no extractable text; counted in Settings, not shown on the row.
    pub text_pending: bool,
    /// The PDF's path below the library root.
    pub relative_path: String,
    /// Where the title came from.
    pub title_origin: Origin,
    /// Where the composer came from.
    pub composer_origin: Origin,
    /// The matching text around a search hit.
    pub snippet: Option<Snippet>,
    /// The set item this row shows; `None` outside a set.
    pub item_id: Option<String>,
    /// The item's one-based number in its set, counting notes; `None` outside a set.
    pub position: Option<u32>,
    /// A set's piece whose PDF is no longer in the library.
    pub missing: bool,
    /// Whether the score is starred.
    pub favorite: bool,
    /// The first-page PNG, once a scan has rendered it.
    pub thumbnail: Option<String>,
}

/// Where a metadata value came from ([`notae_domain::MetadataOrigin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Origin {
    /// Parsed from the file name.
    FilenameDerived,
    /// Imported from a desktop sidecar by an earlier version.
    PcImported,
    /// Edited in the app.
    Manual,
    /// Not known.
    Unset,
}

impl From<notae_domain::MetadataOrigin> for Origin {
    fn from(origin: notae_domain::MetadataOrigin) -> Self {
        match origin {
            notae_domain::MetadataOrigin::FilenameDerived => Self::FilenameDerived,
            notae_domain::MetadataOrigin::PcImported => Self::PcImported,
            notae_domain::MetadataOrigin::Manual => Self::Manual,
            notae_domain::MetadataOrigin::Unset => Self::Unset,
        }
    }
}

/// A search hit's text, split around its first match ([`split_snippet`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Snippet {
    /// The text before the match; a long lead starts with "…".
    pub before: String,
    /// The matched text, shown highlighted.
    pub hit: String,
    /// The text after the match.
    pub after: String,
}

/// One item of the open set.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
#[allow(
    clippy::large_enum_variant,
    reason = "UniFFI lowers the row by value; a short list per snapshot"
)]
pub enum SetEntry {
    /// A piece; [`Row::position`] numbers it.
    Piece {
        /// The piece's row.
        row: Row,
    },
    /// A text note between pieces.
    Note {
        /// One-based position in the set.
        position: u32,
        /// The set item's id.
        item_id: String,
        /// The note's text.
        text: String,
    },
}

/// One folder card of the Sets section.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SetCard {
    /// The set's id.
    pub id: String,
    /// The set's name.
    pub name: String,
    /// ISO `YYYY-MM-DD`, if the set has a date.
    pub date: Option<String>,
    /// Items, pieces and notes.
    pub count: u32,
    /// When the set was last edited.
    pub edited: Edited,
}

/// When a set was last edited ([`text::Edited`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Edited {
    /// Earlier today.
    Today,
    /// Yesterday.
    Yesterday,
    /// Two to six days ago.
    DaysAgo {
        /// How many days.
        days: u32,
    },
    /// A week ago or more.
    On {
        /// ISO `YYYY-MM-DD`.
        date: String,
    },
}

impl From<text::Edited> for Edited {
    fn from(edited: text::Edited) -> Self {
        match edited {
            text::Edited::Today => Self::Today,
            text::Edited::Yesterday => Self::Yesterday,
            text::Edited::DaysAgo(days) => Self::DaysAgo { days },
            text::Edited::On(date) => Self::On { date },
        }
    }
}

/// The details screen's score and its place in the section.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Selection {
    /// The score.
    pub row: Row,
    /// The previous row's selection id; `None` on the first row or outside the section.
    pub previous: Option<String>,
    /// The next row's selection id; `None` on the last row or outside the section.
    pub next: Option<String>,
    /// "X of Y"; `None` when the score is not in the section (a search result).
    pub position: Option<PositionOf>,
}

/// "X of Y" ([`text::PositionOf`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct PositionOf {
    /// One-based index of the row.
    pub position: u32,
    /// Rows in the section.
    pub total: u32,
}

/// The search field and scope.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchBox {
    /// The text.
    pub query: String,
    /// The selected chip.
    pub scope: SearchScope,
}

/// Which fields a search looks in ([`notae_index::SearchScope`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SearchScope {
    /// All.
    Everything,
    /// Titles, numbers and file names.
    Titles,
    /// Composers.
    Composers,
    /// Tags.
    Tags,
    /// Lyrics and other page text.
    Text,
}

impl From<notae_index::SearchScope> for SearchScope {
    fn from(scope: notae_index::SearchScope) -> Self {
        match scope {
            notae_index::SearchScope::Everything => Self::Everything,
            notae_index::SearchScope::Titles => Self::Titles,
            notae_index::SearchScope::Composers => Self::Composers,
            notae_index::SearchScope::Tags => Self::Tags,
            notae_index::SearchScope::Text => Self::Text,
        }
    }
}

impl From<SearchScope> for notae_index::SearchScope {
    fn from(scope: SearchScope) -> Self {
        match scope {
            SearchScope::Everything => Self::Everything,
            SearchScope::Titles => Self::Titles,
            SearchScope::Composers => Self::Composers,
            SearchScope::Tags => Self::Tags,
            SearchScope::Text => Self::Text,
        }
    }
}

/// The operation banner ([`OperationBanner`]).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Banner {
    /// The work in progress or its outcome; `None` hides the banner.
    pub message: Option<StatusMessage>,
    /// Work is in progress.
    pub busy: bool,
    /// The banner reports a failure.
    pub failed: bool,
    /// Progress in `0.0..=1.0`; `None` for an indeterminate bar.
    pub fraction: Option<f32>,
    /// Cancel is enabled.
    pub cancel_enabled: bool,
    /// Dismiss is enabled.
    pub acknowledge_enabled: bool,
}

/// The banner's message ([`text::StatusMessage`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StatusMessage {
    /// "Scanning library…".
    ScanningLibrary,
    /// "Indexing N PDFs…".
    Indexing {
        /// PDFs found.
        total: u32,
    },
    /// "Reading PDF text and thumbnails: X of N".
    ReadingText {
        /// PDFs finished.
        completed: u32,
        /// PDFs in the pass.
        total: u32,
    },
    /// "Finished: …".
    Finished {
        /// What the scan changed.
        summary: ScanSummary,
    },
    /// The operation failed.
    Failed {
        /// Why.
        failure: Failure,
    },
}

/// What a scan changed ([`RescanSummary`](crate::operations::events::RescanSummary)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ScanSummary {
    /// PDFs new to the index.
    pub added: u32,
    /// PDFs whose content changed.
    pub updated: u32,
    /// PDFs found at a new path.
    pub moved: u32,
    /// PDFs no longer in the library.
    pub deleted: u32,
    /// PDFs whose note number clashes with another.
    pub conflicts: u32,
    /// PDFs that could not be read or indexed, one per file.
    pub failures: u32,
}

impl From<crate::operations::events::RescanSummary> for ScanSummary {
    fn from(summary: crate::operations::events::RescanSummary) -> Self {
        Self {
            added: count(summary.added),
            updated: count(summary.updated),
            moved: count(summary.moved),
            deleted: count(summary.deleted),
            conflicts: count(summary.conflicts),
            failures: count(summary.failures),
        }
    }
}

/// What the Settings screen shows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SettingsInfo {
    /// The open library folder's display name.
    pub library_name: Option<String>,
    /// Scores in the index.
    pub indexed_count: u32,
    /// PDFs without extractable text.
    pub no_text_count: u32,
    /// The last scan this session.
    pub latest_scan: Option<ScanSummary>,
    /// The outcome of the last "Export sets", until Settings is opened again.
    pub export: Option<ExportStatus>,
}

/// The outcome of an export.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ExportStatus {
    /// "Favorites and sets exported".
    Exported,
    /// "Could not export sets: …" followed by `detail`.
    Failed {
        /// The platform's own message.
        detail: String,
    },
}

/// The saved layout choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct LayoutPrefs {
    /// Grid or list, for every section.
    pub library_view: LibraryView,
    /// The sidebar is collapsed in portrait.
    pub sidebar_collapsed_portrait: bool,
    /// The sidebar is collapsed in landscape.
    pub sidebar_collapsed_landscape: bool,
}

/// The new or edit set dialog. The fields are what it opened with, or what the last rejected
/// save held.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SetDialog {
    /// The set being edited; `None` for a new set.
    pub set_id: Option<String>,
    /// The name.
    pub name: String,
    /// ISO `YYYY-MM-DD`, or `None`.
    pub date: Option<String>,
    /// The notes, or `None`.
    pub notes: Option<String>,
    /// Why the last save was rejected.
    pub error: Option<SetFieldError>,
}

/// The delete confirmation.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DeleteSetConfirmation {
    /// The set's name.
    pub name: String,
}

/// The metadata editor's fields. They are empty until the score's record has loaded.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MetadataDraft {
    /// The score's content key.
    pub note_key: String,
    /// The score number, or empty.
    pub number: String,
    /// The title.
    pub title: String,
    /// The composer, or empty.
    pub composer: String,
    /// The tags.
    pub tags: Vec<String>,
    /// Why the last load or save failed.
    pub error: Option<Failure>,
}

/// The "Add to set…" sheet.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddToSetSheet {
    /// The score's content key.
    pub note_key: String,
    /// The score's title.
    pub title: String,
    /// Every set, most recently edited when the sheet opened first.
    pub choices: Vec<SetChoice>,
}

/// One set in the sheet ([`crate::ui::SetChoice`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SetChoice {
    /// The set's id.
    pub id: String,
    /// The set's name.
    pub name: String,
    /// ISO `YYYY-MM-DD`, if the set has a date.
    pub date: Option<String>,
    /// Items in the set.
    pub count: u32,
    /// The set holds the score.
    pub checked: bool,
}

/// The add picker over the open set.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddPicker {
    /// The search text.
    pub query: String,
    /// The hits, or the whole library for a blank query.
    pub rows: Vec<PickerRow>,
    /// Pieces this session added, for "N added at the end of the set".
    pub added: u32,
}

/// Why a PDF could not be opened ([`ViewerError`](crate::pdf_intent::ViewerError)).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum PdfOpenError {
    /// The document's URI was not a SAF content URI.
    InvalidContentUri,
    /// No installed app can view a PDF.
    NoCompatibleViewer,
    /// Android refused the launch.
    Platform {
        /// Android's message.
        detail: String,
    },
}

impl From<PdfOpenError> for crate::pdf_intent::ViewerError {
    fn from(error: PdfOpenError) -> Self {
        match error {
            PdfOpenError::InvalidContentUri => Self::InvalidContentUri,
            PdfOpenError::NoCompatibleViewer => Self::NoCompatibleViewer,
            PdfOpenError::Platform { detail } => Self::Platform(detail),
        }
    }
}

impl From<&crate::pdf_intent::ViewerError> for PdfOpenError {
    fn from(error: &crate::pdf_intent::ViewerError) -> Self {
        match error {
            crate::pdf_intent::ViewerError::InvalidContentUri => Self::InvalidContentUri,
            crate::pdf_intent::ViewerError::NoCompatibleViewer => Self::NoCompatibleViewer,
            crate::pdf_intent::ViewerError::Platform(detail) => Self::Platform {
                detail: detail.clone(),
            },
        }
    }
}

/// A count as `u32`; a count past `u32::MAX` shows as `u32::MAX`.
pub(super) fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The parts of a snapshot the core keeps outside [`UiModel`].
pub(super) struct Extras<'a> {
    pub(super) revision: u64,
    pub(super) set_dialog: Option<&'a SetDialog>,
    pub(super) export: Option<&'a ExportStatus>,
}

/// Builds the snapshot of `model`.
pub(super) fn build(model: &UiModel, extras: &Extras<'_>) -> Snapshot {
    let state = model.state();
    let on_search = matches!(state.screen(), crate::state::Screen::Search);
    let visible = model.visible_rows();
    let open_set = match model.section() {
        LibrarySection::Set(id) if !on_search => model.sets().get(id),
        _ => None,
    };
    let set_cards = browse::set_cards(model.sets(), crate::ui::unix_now());
    let subtitle = if let Some(set) = open_set {
        let subtitle = browse::set_subtitle(set);
        Subtitle::Set {
            date: subtitle.date,
            pieces: count(subtitle.pieces),
            notes: count(subtitle.notes),
        }
    } else if model.section() == &LibrarySection::Sets {
        Subtitle::Sets {
            count: count(set_cards.len()),
        }
    } else {
        Subtitle::Scores {
            count: count(visible.len()),
        }
    };
    let set_entries = open_set.map_or_else(Vec::new, |set| {
        browse::set_entries(model.search_rows(), set)
            .into_iter()
            .map(|entry| match entry {
                browse::SetEntry::Piece(row) => SetEntry::Piece {
                    row: build_row(model, row),
                },
                browse::SetEntry::Note {
                    position,
                    item_id,
                    text,
                } => SetEntry::Note {
                    position: count(position),
                    item_id,
                    text,
                },
            })
            .collect()
    });
    let status = model.status();
    let prefs = model.ui_prefs();
    Snapshot {
        revision: extras.revision,
        screen: screen(model),
        section_key: model.section().key(),
        title: model.section_title().into(),
        subtitle,
        set_notes: open_set.and_then(|set| set.notes.clone()),
        sections: browse::sections(model.search_rows(), &model.section_context())
            .into_iter()
            .map(|section| SectionEntry {
                key: section.key,
                title: section.label.into(),
                count: count(section.count),
            })
            .collect(),
        composers: browse::composers(model.search_rows())
            .into_iter()
            .map(|composer| ComposerEntry {
                key: LibrarySection::Composer(composer.name.clone()).key(),
                name: composer.name,
                count: count(composer.count),
            })
            .collect(),
        rows: visible
            .into_iter()
            .map(|row| build_row(model, row))
            .collect(),
        set_entries,
        set_cards: set_cards
            .into_iter()
            .map(|card| SetCard {
                id: card.id,
                name: card.name,
                date: card.date,
                count: count(card.count),
                edited: card.edited.into(),
            })
            .collect(),
        selection: model.selected_row().map(|row| {
            let neighbours = model.preview_neighbours();
            Selection {
                row: build_row(model, row),
                previous: neighbours.as_ref().and_then(|nav| nav.previous.clone()),
                next: neighbours.as_ref().and_then(|nav| nav.next.clone()),
                position: neighbours.map(|nav| {
                    let place = nav.position_of();
                    PositionOf {
                        position: count(place.position),
                        total: count(place.total),
                    }
                }),
            }
        }),
        search: SearchBox {
            query: state.search().query.clone(),
            scope: state.search().scope.into(),
        },
        banner: banner(&status.operation),
        rescan_enabled: model.rescan_enabled(),
        settings: SettingsInfo {
            library_name: match model.storage_kind() {
                StorageStateKindView::Ready { display_name } => Some(display_name),
                _ => None,
            },
            indexed_count: count(status.indexed_count),
            no_text_count: count(status.no_text_count),
            latest_scan: status.latest_rescan.map(Into::into),
            export: extras.export.cloned(),
        },
        layout: LayoutPrefs {
            library_view: prefs.library_view,
            sidebar_collapsed_portrait: prefs.sidebar_collapsed_portrait,
            sidebar_collapsed_landscape: prefs.sidebar_collapsed_landscape,
        },
        set_dialog: extras.set_dialog.cloned(),
        delete_set: model
            .pending_delete_set_name()
            .map(|name| DeleteSetConfirmation {
                name: name.to_owned(),
            }),
        metadata_editor: model.metadata_editor().map(|metadata| MetadataDraft {
            note_key: metadata.note_key.clone(),
            number: metadata.id.clone(),
            title: metadata.title.clone(),
            composer: metadata.composer_or_artist.clone(),
            tags: metadata.tags.clone(),
            error: model.metadata_error().cloned(),
        }),
        add_to_set: model.add_to_set_key().map(|key| AddToSetSheet {
            note_key: key.to_owned(),
            title: model.add_to_set_title(),
            choices: model
                .add_to_set_choices()
                .into_iter()
                .map(|choice| SetChoice {
                    id: choice.id,
                    name: choice.name,
                    date: choice.date,
                    count: count(choice.count),
                    checked: choice.checked,
                })
                .collect(),
        }),
        add_picker: model.add_picker_open().then(|| AddPicker {
            query: model.add_picker_query().to_owned(),
            rows: model.add_picker_rows(),
            added: count(model.add_picker_added_count()),
        }),
        pdf_open_error: model.pdf_launch_error().map(Into::into),
        import_available: model.favorites().is_empty() && model.sets().sets().is_empty(),
    }
}

/// The storage state decides the setup screens, as in the Slint app; the controller decides the
/// rest.
fn screen(model: &UiModel) -> Screen {
    if model.startup_presentation() != StartupPresentation::Ready {
        return Screen::Loading;
    }
    let setup = match model.storage_kind() {
        StorageStateKindView::Unconfigured => SetupScreen::Unconfigured,
        StorageStateKindView::PermissionLost(folder) => SetupScreen::PermissionLost { folder },
        StorageStateKindView::Error(error) => SetupScreen::Error {
            error: (&error).into(),
        },
        StorageStateKindView::Ready { .. } => {
            return match model.state().screen() {
                crate::state::Screen::Search => Screen::Search,
                crate::state::Screen::Settings => Screen::Settings,
                crate::state::Screen::Library | crate::state::Screen::Setup(_) => Screen::Library,
            };
        }
    };
    Screen::Setup { setup }
}

fn build_row(model: &UiModel, row: SearchRow) -> Row {
    let thumbnail = if row.missing {
        // A missing piece has no PDF, so a thumbnail left from its old content is stale.
        None
    } else {
        model
            .thumbnail_cache()
            .and_then(|cache| cache.existing(&row.note_key))
            .map(|path| path.to_string_lossy().into_owned())
    };
    Row {
        selection_id: browse::row_id(&row).to_owned(),
        favorite: model.is_favorite(&row.note_key),
        snippet: row.text_snippet.as_deref().map(|snippet| {
            let (before, hit, after) = split_snippet(snippet);
            Snippet { before, hit, after }
        }),
        thumbnail,
        note_key: row.note_key,
        number: row.id,
        title: row.title,
        composer: row.composer_or_artist,
        tags: row.tags,
        status: row.status,
        text_pending: row.text_pending,
        relative_path: row.relative_path,
        title_origin: row.title_origin.into(),
        composer_origin: row.composer_origin.into(),
        item_id: row.item_id,
        position: row.position.map(count),
        missing: row.missing,
    }
}

fn banner(operation: &OperationState) -> Banner {
    let banner = OperationBanner::project(operation);
    Banner {
        message: banner.message.map(|message| match message {
            text::StatusMessage::ScanningLibrary => StatusMessage::ScanningLibrary,
            text::StatusMessage::Indexing { total } => StatusMessage::Indexing {
                total: count(total),
            },
            text::StatusMessage::ReadingText { completed, total } => StatusMessage::ReadingText {
                completed: count(completed),
                total: count(total),
            },
            text::StatusMessage::Finished(summary) => StatusMessage::Finished {
                summary: summary.into(),
            },
            text::StatusMessage::Failed { failure } => StatusMessage::Failed { failure },
        }),
        busy: banner.busy,
        failed: banner.failed,
        fraction: banner.fraction,
        cancel_enabled: banner.cancel_enabled,
        acknowledge_enabled: banner.acknowledge_enabled,
    }
}
