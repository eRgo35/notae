//! What the user did, as the Kotlin app reports it through [`NotaeCore::dispatch`].
//!
//! One variant per callback the Slint app wires today (`android_main.rs` and its `wire_sets`),
//! except the language choice, which Kotlin applies through AppCompat on its own. The deferred
//! sets features add their variants with their tasks.
//!
//! [`NotaeCore::dispatch`]: super::NotaeCore::dispatch

use crate::api::snapshot::SearchScope;
use crate::ui_prefs::LibraryView;

/// One user action.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Action {
    /// "Choose score folder", "Reconnect folder" or Settings' "Change": asks Kotlin for the
    /// folder picker ([`Effect::PickLibraryRoot`](super::Effect::PickLibraryRoot)), starting at the
    /// open or saved folder.
    PickLibraryRoot,
    /// "Scan library".
    Rescan,
    /// The banner's Cancel.
    CancelRescan,
    /// The banner's Dismiss.
    AcknowledgeOperation,
    /// A side-panel row or chip, by [`SectionEntry::key`](super::SectionEntry::key) or
    /// [`ComposerEntry::key`](super::ComposerEntry::key).
    SelectSection {
        /// The section's key.
        key: String,
    },
    /// The search icon or Ctrl K.
    OpenSearch,
    /// The search text changed.
    SearchChanged {
        /// The whole text after the edit.
        query: String,
    },
    /// A scope chip.
    SearchScopeChanged {
        /// The chosen scope.
        scope: SearchScope,
    },
    /// The microphone: asks Kotlin for the recognizer
    /// ([`Effect::StartVoiceSearch`](super::Effect::StartVoiceSearch)). The spoken phrase
    /// comes back as [`Action::SearchChanged`].
    VoiceSearch,
    /// The search screen's back arrow.
    CloseSearch,
    /// A search result: closes search and opens its details.
    SearchResultChosen {
        /// The result's content key.
        note_key: String,
    },
    /// "Edit details".
    EditMetadata {
        /// The score's content key.
        note_key: String,
    },
    /// The metadata editor's "Save details".
    SaveMetadata {
        /// The score's content key.
        note_key: String,
        /// The score number field; empty removes the number.
        number: String,
        /// The title field.
        title: String,
        /// The composer field; empty removes the composer.
        composer: String,
        /// The tags field as typed: split on commas, trimmed, empty tags dropped.
        tags: String,
    },
    /// The metadata editor's Cancel or close.
    CancelMetadata,
    /// The grid/list switch.
    SetLibraryView {
        /// The chosen layout.
        view: LibraryView,
    },
    /// "Hide sidebar" / "Show sidebar".
    SetSidebarCollapsed {
        /// Whether the window is in portrait.
        portrait: bool,
        /// Whether the sidebar is now collapsed.
        collapsed: bool,
    },
    /// Opens the details screen on a row, or moves it to a neighbour.
    SelectScore {
        /// The row's [`Row::selection_id`](super::Row::selection_id).
        selection_id: String,
    },
    /// The details screen's back arrow.
    ClearSelection,
    /// The star.
    ToggleFavorite {
        /// The score's content key.
        note_key: String,
    },
    /// A tap on a score, or "Open score": asks Kotlin to open the PDF
    /// ([`Effect::OpenPdf`](super::Effect::OpenPdf)).
    OpenPdf {
        /// The row's [`Row::relative_path`](super::Row::relative_path).
        relative_path: String,
        /// The score's content key.
        note_key: String,
        /// The set item, when the score was opened inside a set.
        item_id: Option<String>,
    },
    /// Settings.
    OpenSettings,
    /// Settings' back arrow.
    CloseSettings,
    /// "Export sets": hands Kotlin the file to save
    /// ([`Effect::WriteExport`](super::Effect::WriteExport)).
    ExportSets,
    /// "New set": opens the empty set dialog.
    NewSet,
    /// A set card or row: opens the set.
    OpenSet {
        /// The set's id.
        set_id: String,
    },
    /// "Edit set": opens the set dialog filled from the set.
    EditSet {
        /// The set's id.
        set_id: String,
    },
    /// The set dialog's "Save set".
    SaveSetDialog {
        /// The name field.
        name: String,
        /// The ISO `YYYY-MM-DD` date, or `None` for no date.
        date: Option<String>,
        /// The notes field, or `None` for no notes.
        notes: Option<String>,
    },
    /// The set dialog's Cancel.
    CancelSetDialog,
    /// "Delete set": asks for confirmation.
    DeleteSet {
        /// The set's id.
        set_id: String,
    },
    /// The confirmation's Delete.
    ConfirmDeleteSet,
    /// The confirmation's Cancel.
    CancelDeleteSet,
    /// "Add to set…": opens the checklist for a score.
    AddToSet {
        /// The score's content key.
        note_key: String,
    },
    /// A checklist row: adds the score to the set, or removes its last copy.
    ToggleSetMembership {
        /// The set's id.
        set_id: String,
        /// The score's content key.
        note_key: String,
    },
    /// The checklist's "New set": closes it and opens the set dialog for a set holding the
    /// score.
    AddToSetNewSet,
    /// Closes the checklist.
    CloseAddToSet,
    /// "Remove from set", in the open set.
    RemoveSetItem {
        /// The set item's id.
        item_id: String,
    },
    /// The set header's Add: opens the add picker.
    OpenAddPicker,
    /// The picker's search text changed.
    AddPickerSearch {
        /// The whole text after the edit.
        query: String,
    },
    /// A picker row's add button.
    ToggleAddPickerPiece {
        /// The score's content key.
        note_key: String,
    },
    /// The picker's Done.
    CloseAddPicker,
}
