//! UI state bridge and host-testable library/search view models.
//!
//! `library` and `search` project the shared index into rows the Android
//! surface can display, and compile on Linux for deterministic tests. The
//! `UiModel` keeps storage and application state synchronized; the UI
//! toolkit bindings and the Android event loop live in the app crates.
//!
//! [`state`]: crate::state

#![forbid(unsafe_code)]
pub mod browse;
pub mod library;
pub mod metadata_editor;
pub mod search;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use crate::favorites::{Favorites, PieceRef};
use crate::operations::events::{OperationEvent, RescanSummary};
use crate::pdf_intent::ViewerError;
use crate::recent::RecentScores;
use crate::sets::{ItemKind, SetFieldError, SetFields, SetStore};
use crate::state::{
    AppController, AppState, EventError, OperationState, RescanPhase, Transition, UiEvent,
};
use crate::storage::{LibraryRoot, StorageError, StorageState};
use crate::text::{Failure, SectionTitle, StatusMessage};
use crate::thumbnails::ThumbnailCache;
use crate::ui::browse::{
    LibrarySection, Neighbours, SectionContext, neighbours, row_id, section_rows,
};
use crate::ui::metadata_editor::EditableMetadata;
use crate::ui::search::SearchRow;
use crate::ui_prefs::{LibraryView, UiPrefs};

/// What the app may present while a previously selected library is restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPresentation {
    /// The saved library permission has not been checked yet.
    Restoring,
    /// Storage is ready, but the first worker row projection has not arrived.
    LoadingRows,
    /// Setup, recovery, the library, or an actionable error may be presented.
    Ready,
}

/// A UI-facing snapshot of operation and library status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStatus {
    /// The background operation's state.
    pub operation: crate::state::OperationState,
    /// The last finished rescan's counts.
    pub latest_rescan: Option<RescanSummary>,
    /// Scores in the index.
    pub indexed_count: usize,
    /// Indexed PDFs with no extractable text; shown in Settings as a plain count.
    pub no_text_count: usize,
}

/// One set in the "Add to set…" checklist; `checked` when it holds the piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetChoice {
    /// The set's id.
    pub id: String,
    /// The set's name.
    pub name: String,
    /// The set's ISO `YYYY-MM-DD` date, if it has one.
    pub date: Option<String>,
    /// How many items (pieces and notes) the set holds.
    pub count: usize,
    /// Whether the set holds the piece the checklist is open for.
    pub checked: bool,
}

/// Where [`UiModel::move_set_item`] moves an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetMove {
    /// By this many places; negative is up.
    By(i32),
    /// To this zero-based place.
    To(usize),
}

/// One score in the add picker.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PickerRow {
    /// The score's content key.
    pub note_key: String,
    /// The score's title.
    pub title: String,
    /// The note number, when one is assigned.
    pub number: Option<String>,
    /// The composer or artist, when known.
    pub composer: Option<String>,
    /// In the set before the picker opened.
    pub in_set: bool,
    /// Added during this picker session.
    pub added: bool,
}

/// Bridge between the UI and the [`AppState`].
///
/// The glue is intentionally thin: it owns an [`AppController`]
/// (which holds the in-memory screen / library / search state) and a
/// [`StorageState`] (which holds the SAF-side root and source). The
/// [`StorageState`] is what the picker manipulates; the
/// [`AppController`] is what the rest of the UI dispatches against.
/// They are kept distinct so the storage layer can be swapped (host
/// test vs. device) without touching the state machine the UI
/// consumes.
pub struct UiModel {
    controller: AppController,
    storage: Arc<Mutex<StorageState>>,
    /// The open library's root, copied when storage became ready. The worker holds the
    /// [`LibrarySource`](crate::storage::LibrarySource) lock for a whole platform call, so the
    /// UI reads the root from here instead.
    ready_root: Option<LibraryRoot>,
    search_rows: Vec<SearchRow>,
    indexed_count: usize,
    latest_rescan: Option<RescanSummary>,
    metadata_editor: Option<EditableMetadata>,
    metadata_error: Option<Failure>,
    pdf_launch_error: Option<ViewerError>,
    section: LibrarySection,
    recent: RecentScores,
    recent_path: Option<PathBuf>,
    favorites: Favorites,
    favorites_path: Option<PathBuf>,
    sets: SetStore,
    sets_path: Option<PathBuf>,
    /// The set whose deletion waits for the user's confirmation.
    pending_delete_set: Option<String>,
    /// The "Add to set…" checklist, while it is open.
    add_to_set: Option<AddToSet>,
    /// The piece that the set dialog, opened from the checklist's "New set", adds to the new set.
    new_set_piece: Option<String>,
    /// The add picker, while it is open over a set.
    add_picker: Option<AddPicker>,
    /// Reorder mode is on in the open set.
    reordering: bool,
    ui_prefs: UiPrefs,
    ui_prefs_path: Option<PathBuf>,
    /// The previewed row's selection id ([`row_id`]).
    selected: Option<String>,
    thumbnails: Option<ThumbnailCache>,
    startup_presentation: StartupPresentation,
    result_shown_at: Option<std::time::Instant>,
}

/// How long a successful operation result stays in the banner before it clears itself.
pub const RESULT_VISIBLE_FOR: std::time::Duration = std::time::Duration::from_secs(5);

impl UiModel {
    /// Builds a `UiModel` around the supplied controller and storage
    /// state. Used by the Android entry point to share the storage
    /// state with the picker callback closures.
    #[must_use]
    pub fn from_parts(controller: AppController, storage: Arc<Mutex<StorageState>>) -> Self {
        Self {
            controller,
            storage,
            ready_root: None,
            search_rows: Vec::new(),
            indexed_count: 0,
            latest_rescan: None,
            metadata_editor: None,
            metadata_error: None,
            pdf_launch_error: None,
            section: LibrarySection::All,
            recent: RecentScores::default(),
            recent_path: None,
            favorites: Favorites::default(),
            favorites_path: None,
            sets: SetStore::empty(),
            sets_path: None,
            pending_delete_set: None,
            add_to_set: None,
            new_set_piece: None,
            add_picker: None,
            reordering: false,
            ui_prefs: UiPrefs::default(),
            ui_prefs_path: None,
            selected: None,
            thumbnails: None,
            startup_presentation: StartupPresentation::Restoring,
            result_shown_at: None,
        }
    }

    /// Builds a `UiModel` around a fresh `AppController` and an
    /// unconfigured storage state. The resulting state matches the
    /// `Setup::Unconfigured` / `Idle` contract the smoke test asserts
    /// on Linux.
    #[must_use]
    pub fn new() -> Self {
        Self::from_parts(
            AppController::new(),
            Arc::new(Mutex::new(StorageState::Unconfigured)),
        )
    }

    /// Returns a reference to the underlying [`AppController`]. Used by
    /// future tasks (Task 5 in particular) to plug the operation worker into
    /// the same state machine the UI is mutating.
    #[allow(dead_code)] // Touched by Task 5's worker integration.
    pub fn controller(&self) -> &AppController {
        &self.controller
    }

    /// Returns a mutable reference to the underlying [`AppController`].
    /// Same future use as [`UiModel::controller`].
    #[allow(dead_code)] // Touched by Task 5's worker integration.
    pub fn controller_mut(&mut self) -> &mut AppController {
        &mut self.controller
    }

    /// The current startup presentation gate.
    #[must_use]
    pub fn startup_presentation(&self) -> StartupPresentation {
        self.startup_presentation
    }

    /// Shows setup after the platform confirms there is no saved library root.
    pub fn finish_startup_without_library(&mut self) {
        if self.startup_presentation == StartupPresentation::Restoring {
            self.startup_presentation = StartupPresentation::Ready;
        }
    }

    /// Updates the storage state in place.
    ///
    /// A ready source is locked once here to copy its root; call this before the worker
    /// starts a scan over the source.
    pub fn set_storage(&mut self, state: StorageState) {
        if self.startup_presentation == StartupPresentation::Restoring {
            self.startup_presentation = if matches!(&state, StorageState::Ready(_)) {
                StartupPresentation::LoadingRows
            } else {
                StartupPresentation::Ready
            };
        }
        if matches!(
            self.controller.state().screen(),
            crate::state::Screen::Setup(_)
        ) && matches!(&state, StorageState::Ready(_))
        {
            // SAF readiness must also enable controller navigation. Worker
            // scheduling remains explicit: choosing a folder does not scan it.
            self.controller.library_ready();
        }
        self.ready_root = match &state {
            StorageState::Ready(source) => Some(
                source
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .root()
                    .clone(),
            ),
            StorageState::Unconfigured
            | StorageState::PermissionLost(_)
            | StorageState::Error(_) => None,
        };
        *self.storage.lock().unwrap_or_else(PoisonError::into_inner) = state;
    }

    /// Returns the persisted SAF tree URI for the active library.
    #[must_use]
    pub fn library_root_handle(&self) -> Option<String> {
        self.ready_root
            .as_ref()
            .map(|root| root.handle().to_owned())
    }

    /// Returns the latest PDF launch error, when a viewer could not be opened.
    #[must_use]
    pub fn pdf_launch_error(&self) -> Option<&ViewerError> {
        self.pdf_launch_error.as_ref()
    }

    /// Replaces the PDF launch error.
    pub fn set_pdf_launch_error(&mut self, error: Option<ViewerError>) {
        self.pdf_launch_error = error;
    }

    /// Replaces the rows currently displayed by the library screen and re-links saved pieces,
    /// as the row-carrying arms of [`Self::apply_operation_event`] do.
    ///
    /// Production rows arrive through [`Self::apply_operation_event`]; tests use this method
    /// to set up a library directly.
    pub fn set_search_rows(&mut self, rows: Vec<SearchRow>) {
        self.search_rows = rows;
        self.relink_saved_pieces();
        self.forget_missing_selection();
        self.forget_missing_composer();
    }

    /// Returns the rows most recently projected by the operation worker.
    #[must_use]
    pub fn search_rows(&self) -> &[SearchRow] {
        &self.search_rows
    }

    /// Loads the recent list from, and later saves it to, `path`.
    pub fn set_recent_path(&mut self, path: PathBuf) {
        self.recent = RecentScores::load(&path);
        self.recent_path = Some(path);
    }

    /// Loads the favorites from, and later saves them to, `path`.
    pub fn set_favorites_path(&mut self, path: PathBuf) {
        self.favorites = Favorites::load(&path);
        self.favorites_path = Some(path);
    }

    /// Stars or unstars the score with `note_key`; returns whether it is now a favorite.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the file cannot be saved; the in-memory list is still updated.
    /// A key that is not in the library changes nothing and returns `Ok(false)`.
    pub fn toggle_favorite(&mut self, note_key: &str) -> std::io::Result<bool> {
        let Some(path) = self
            .search_rows
            .iter()
            .find(|row| row.note_key == note_key)
            .map(|row| row.relative_path.clone())
        else {
            return Ok(false);
        };
        let now = self.favorites.toggle(PieceRef {
            key: note_key.to_owned(),
            path,
        });
        if let Some(path) = &self.favorites_path {
            self.favorites.save(path)?;
        }
        Ok(now)
    }

    /// Whether the score with `key` is starred.
    #[must_use]
    pub fn is_favorite(&self, key: &str) -> bool {
        self.favorites.contains(key)
    }

    /// The starred scores, newest first.
    #[must_use]
    pub fn favorites(&self) -> &[PieceRef] {
        self.favorites.items()
    }

    /// Loads the singing sets from, and later saves them to, `path`. Call before
    /// [`UiModel::set_ui_prefs_path`], which checks a saved open set against them.
    pub fn set_sets_path(&mut self, path: PathBuf) {
        self.sets = SetStore::load(&path);
        self.sets_path = Some(path);
    }

    /// The singing sets.
    #[must_use]
    pub fn sets(&self) -> &SetStore {
        &self.sets
    }

    /// The suggested file name and the text of a favorites-and-sets export made at `now`.
    #[must_use]
    pub fn export_sets(&self, now: u64) -> (String, String) {
        (
            crate::backup::suggested_file_name(now),
            crate::backup::export_json(&self.favorites, &self.sets),
        )
    }

    /// Adds the backup's sets beside the current ones, replacing those with the same id, merges
    /// its favorites, and saves both files. Pieces are re-linked against the open library.
    pub fn import_backup(&mut self, backup: crate::backup::Backup) {
        crate::backup::apply_import(&mut self.favorites, &mut self.sets, backup);
        if let Some(path) = &self.favorites_path
            && let Err(error) = self.favorites.save(path)
        {
            eprintln!("notae: cannot save favorites: {error}");
        }
        self.save_sets();
        self.relink_saved_pieces();
    }

    /// Remembers `item_id` as the open set's item last opened ("Just opened") and saves the
    /// set; its edited time does not change. Outside a set nothing happens.
    pub fn mark_set_item_opened(&mut self, item_id: &str) {
        let LibrarySection::Set(set_id) = &self.section else {
            return;
        };
        if self.sets.mark_opened(set_id, item_id) {
            self.save_sets();
        }
    }

    /// The open set's item last opened; `None` outside a set.
    #[must_use]
    pub fn just_opened_item(&self) -> Option<&str> {
        let LibrarySection::Set(set_id) = &self.section else {
            return None;
        };
        self.sets.get(set_id)?.last_opened_item.as_deref()
    }

    /// Validates `fields`, adds the set and opens it. A set made from the "Add to set…"
    /// checklist's "New set" gets that piece instead, and the owner stays where they were.
    ///
    /// # Errors
    ///
    /// The validation error; nothing is created. A failed save is logged and the set stays in
    /// memory.
    pub fn create_set(&mut self, fields: SetFields) -> Result<String, SetFieldError> {
        let fields = fields.validate()?;
        let now = unix_now();
        let id = self.sets.create(fields, now);
        match self
            .new_set_piece
            .take()
            .and_then(|key| self.piece_ref(&key))
        {
            Some(piece) => {
                self.sets.add_piece(&id, piece, now);
                self.save_sets();
            }
            None => {
                self.save_sets();
                log_prefs_error(self.select_section(LibrarySection::Set(id.clone())));
            }
        }
        Ok(id)
    }

    /// The set dialog was closed without saving; a piece waiting for a new set is forgotten.
    pub fn set_dialog_cancelled(&mut self) {
        self.new_set_piece = None;
    }

    /// A library piece as a set or favorite reference, if `key` is in the library.
    fn piece_ref(&self, key: &str) -> Option<PieceRef> {
        let picker_rows = self
            .add_picker
            .iter()
            .flat_map(|picker| picker.rows.iter().flatten());
        picker_rows
            .chain(&self.search_rows)
            .find(|row| row.note_key == key)
            .map(|row| PieceRef {
                key: key.to_owned(),
                path: row.relative_path.clone(),
            })
    }

    /// Opens the "Add to set…" checklist for the library piece `key`.
    pub fn add_to_set_requested(&mut self, key: &str) {
        if self.piece_ref(key).is_none() {
            return;
        }
        self.add_to_set = Some(AddToSet {
            key: key.to_owned(),
            order: self
                .sets
                .sets_by_modified()
                .into_iter()
                .map(|set| set.id.clone())
                .collect(),
        });
    }

    /// The note key the checklist is open for.
    #[must_use]
    pub fn add_to_set_key(&self) -> Option<&str> {
        self.add_to_set.as_ref().map(|open| open.key.as_str())
    }

    /// The title of the piece the checklist is open for.
    #[must_use]
    pub fn add_to_set_title(&self) -> String {
        self.add_to_set_key()
            .and_then(|key| self.search_rows.iter().find(|row| row.note_key == key))
            .map_or_else(String::new, |row| row.title.clone())
    }

    /// The checklist's rows: every set, most recently edited first when the checklist opened,
    /// checked when it holds the piece.
    #[must_use]
    pub fn add_to_set_choices(&self) -> Vec<SetChoice> {
        let Some(open) = &self.add_to_set else {
            return Vec::new();
        };
        let containing = self.sets.sets_containing(&open.key);
        let cards = crate::ui::browse::set_cards(&self.sets, unix_now());
        open.order
            .iter()
            .filter_map(|id| cards.iter().find(|card| card.id == id.as_str()))
            .map(|card| SetChoice {
                id: card.id.clone(),
                name: card.name.clone(),
                date: card.date.clone(),
                count: card.count,
                checked: containing.iter().any(|id| card.id == id.as_str()),
            })
            .collect()
    }

    /// Checks or unchecks a set in the checklist: checking appends the piece; unchecking removes
    /// the last item with that key from the set.
    pub fn set_membership_toggled(&mut self, set_id: &str, key: &str) {
        let now = unix_now();
        let last_copy = self.sets.get(set_id).and_then(|set| {
            set.items
                .iter()
                .rev()
                .find(|item| matches!(&item.kind, ItemKind::Piece(piece) if piece.key == key))
                .map(|item| item.id.clone())
        });
        let changed = match last_copy {
            Some(item_id) => self.sets.remove_item(set_id, &item_id, now),
            None => self
                .piece_ref(key)
                .and_then(|piece| self.sets.add_piece(set_id, piece, now))
                .is_some(),
        };
        if changed {
            self.save_sets();
            self.forget_missing_selection();
        }
    }

    /// Closes the checklist.
    pub fn add_to_set_closed(&mut self) {
        self.add_to_set = None;
    }

    /// The checklist's "New set": closes it and keeps its piece for the set dialog's save.
    pub fn add_to_set_new_set_requested(&mut self) {
        self.new_set_piece = self.add_to_set.take().map(|open| open.key);
    }

    /// Removes one item from the open set at once; removal is not confirmed.
    pub fn remove_set_item_requested(&mut self, item_id: &str) {
        let LibrarySection::Set(set_id) = &self.section else {
            return;
        };
        if self.sets.remove_item(set_id, item_id, unix_now()) {
            self.save_sets();
            self.forget_missing_selection();
        }
    }

    /// Starts reorder mode in the open set and closes the details beside it. Outside a set,
    /// or on the search screen, nothing happens.
    pub fn reorder_started(&mut self) {
        if matches!(self.section, LibrarySection::Set(_))
            && matches!(self.state().screen(), crate::state::Screen::Library)
        {
            self.reordering = true;
            self.select_note(None);
        }
    }

    /// Ends reorder mode; the layout was never changed, so the owner's choice shows again.
    pub fn reorder_finished(&mut self) {
        self.reordering = false;
    }

    /// Reorder mode is on in the open set.
    #[must_use]
    pub fn reordering(&self) -> bool {
        self.reordering
    }

    /// Moves an item of the open set by `delta` places (up and down) or to `index` (a drag),
    /// clamped to the set, and saves the set.
    pub fn move_set_item(&mut self, item_id: &str, to: SetMove) {
        let LibrarySection::Set(set_id) = &self.section else {
            return;
        };
        let now = unix_now();
        let moved = match to {
            SetMove::By(delta) => self.sets.move_item(set_id, item_id, delta, now),
            SetMove::To(index) => self.sets.move_item_to(set_id, item_id, index, now),
        };
        if moved {
            self.save_sets();
        }
    }

    /// Appends a text note to the open set and closes the add picker. Blank text does nothing.
    pub fn add_note_requested(&mut self, text: &str) {
        let LibrarySection::Set(set_id) = &self.section else {
            return;
        };
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if self
            .sets
            .add_note(set_id, text.to_owned(), unix_now())
            .is_some()
        {
            self.save_sets();
            self.add_picker = None;
        }
    }

    /// Replaces a note's text in the open set. Blank text does nothing.
    pub fn edit_note_requested(&mut self, item_id: &str, text: &str) {
        let LibrarySection::Set(set_id) = &self.section else {
            return;
        };
        let text = text.trim();
        if !text.is_empty()
            && self
                .sets
                .edit_note(set_id, item_id, text.to_owned(), unix_now())
        {
            self.save_sets();
        }
    }

    /// Opens the add picker over the open set.
    pub fn add_picker_opened(&mut self) {
        let LibrarySection::Set(set_id) = &self.section else {
            return;
        };
        let Some(set) = self.sets.get(set_id) else {
            return;
        };
        let before = set
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                ItemKind::Piece(piece) => Some(piece.key.clone()),
                ItemKind::Note { .. } => None,
            })
            .collect();
        self.add_picker = Some(AddPicker {
            set_id: set.id.clone(),
            before,
            added: Vec::new(),
            query: String::new(),
            rows: None,
        });
    }

    /// Whether the add picker is open.
    #[must_use]
    pub fn add_picker_open(&self) -> bool {
        self.add_picker.is_some()
    }

    /// The picker's search text. A blank query lists the whole library at once; any other waits
    /// for the worker's [`OperationEvent::PickerRows`].
    pub fn add_picker_search_changed(&mut self, query: &str) {
        if let Some(picker) = &mut self.add_picker {
            picker.query = query.to_owned();
            if query.trim().is_empty() {
                picker.rows = None;
            }
        }
    }

    /// Taps a picker row: adds the piece at the end of the set, or, if this session added it,
    /// removes the copy this session added last.
    pub fn add_picker_toggled(&mut self, key: &str) {
        let now = unix_now();
        let piece = self.piece_ref(key);
        let Some(picker) = &mut self.add_picker else {
            return;
        };
        let changed = match picker.added.iter().rposition(|(added, _)| added == key) {
            Some(index) => {
                let (_, item_id) = picker.added.remove(index);
                self.sets.remove_item(&picker.set_id, &item_id, now)
            }
            None => match piece.and_then(|piece| self.sets.add_piece(&picker.set_id, piece, now)) {
                Some(item_id) => {
                    picker.added.push((key.to_owned(), item_id));
                    true
                }
                None => false,
            },
        };
        if changed {
            self.save_sets();
        }
    }

    /// Closes the add picker; what it added stays in the set.
    pub fn add_picker_closed(&mut self) {
        self.add_picker = None;
    }

    /// The picker's search text; empty when the picker is closed.
    #[must_use]
    pub fn add_picker_query(&self) -> &str {
        self.add_picker
            .as_ref()
            .map_or("", |picker| picker.query.as_str())
    }

    /// The picker's rows: the search hits, or the whole library for a blank query.
    #[must_use]
    pub fn add_picker_rows(&self) -> Vec<PickerRow> {
        let Some(picker) = &self.add_picker else {
            return Vec::new();
        };
        picker
            .rows
            .as_deref()
            .unwrap_or(&self.search_rows)
            .iter()
            .map(|row| PickerRow {
                note_key: row.note_key.clone(),
                title: row.title.clone(),
                number: row.id.clone(),
                composer: row.composer_or_artist.clone(),
                in_set: picker.before.contains(&row.note_key),
                added: picker.added.iter().any(|(key, _)| *key == row.note_key),
            })
            .collect()
    }

    /// How many pieces this picker session added, for the picker's footer.
    #[must_use]
    pub fn add_picker_added_count(&self) -> usize {
        self.add_picker
            .as_ref()
            .map_or(0, |picker| picker.added.len())
    }

    /// Validates `fields` and replaces the set's name, date and notes.
    ///
    /// # Errors
    ///
    /// The validation error; the set is unchanged. An unknown id changes nothing.
    pub fn update_set(&mut self, id: &str, fields: SetFields) -> Result<(), SetFieldError> {
        let fields = fields.validate()?;
        if self.sets.update(id, fields, unix_now()) {
            self.save_sets();
        }
        Ok(())
    }

    /// Deletes the set; when it was open, the list of sets is shown instead.
    pub fn delete_set(&mut self, id: &str) {
        if !self.sets.delete(id) {
            return;
        }
        self.save_sets();
        if self.section == LibrarySection::Set(id.to_owned()) {
            log_prefs_error(self.select_section(LibrarySection::Sets));
        }
    }

    /// Asks before deleting the set: [`UiModel::pending_delete_set_name`] names it until the
    /// user confirms or cancels.
    pub fn request_delete_set(&mut self, id: &str) {
        self.pending_delete_set = self.sets.get(id).map(|set| set.id.clone());
    }

    /// The name of the set waiting for delete confirmation.
    #[must_use]
    pub fn pending_delete_set_name(&self) -> Option<&str> {
        let id = self.pending_delete_set.as_deref()?;
        self.sets.get(id).map(|set| set.name.as_str())
    }

    /// Deletes the set the user was asked about.
    pub fn confirm_delete_set(&mut self) {
        if let Some(id) = self.pending_delete_set.take() {
            self.delete_set(&id);
        }
    }

    /// Keeps the set the user was asked about.
    pub fn cancel_delete_set(&mut self) {
        self.pending_delete_set = None;
    }

    /// Follows replaced PDFs: a piece whose key left the library takes the key of the entry at
    /// its path, and the changed file is saved.
    fn relink_saved_pieces(&mut self) {
        let library: Vec<(String, String)> = self
            .search_rows
            .iter()
            .map(|row| (row.note_key.clone(), row.relative_path.clone()))
            .collect();
        if self.favorites.relink(&library)
            && let Some(path) = &self.favorites_path
            && let Err(error) = self.favorites.save(path)
        {
            eprintln!("notae: cannot save favorites: {error}");
        }
        if self.sets.relink(&library) {
            self.save_sets();
        }
    }

    fn save_sets(&self) {
        if let Some(path) = &self.sets_path
            && let Err(error) = self.sets.save(path)
        {
            eprintln!("notae: cannot save sets: {error}");
        }
    }

    /// The lists the sections draw on.
    #[must_use]
    pub fn section_context(&self) -> SectionContext<'_> {
        SectionContext {
            recent: self.recent.keys(),
            favorites: self.favorites.items(),
            sets: &self.sets,
        }
    }

    /// Loads the UI preferences from, and later saves them to, `path`, and reopens the saved
    /// section and details. A saved set that no longer exists opens the list of sets; the saved
    /// details wait for the library's rows, which close them if their score is gone.
    pub fn set_ui_prefs_path(&mut self, path: PathBuf) {
        self.ui_prefs = UiPrefs::load(&path);
        self.ui_prefs_path = Some(path);
        self.section = match LibrarySection::from_key(&self.ui_prefs.section) {
            LibrarySection::Set(id) if self.sets.get(&id).is_none() => LibrarySection::Sets,
            section => section,
        };
        self.selected.clone_from(&self.ui_prefs.selection);
        self.forget_missing_composer();
    }

    /// The current UI preferences.
    #[must_use]
    pub fn ui_prefs(&self) -> &UiPrefs {
        &self.ui_prefs
    }

    /// Chooses the library layout for every section and persists a change.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the preferences cannot be saved; the choice stays in effect.
    pub fn set_library_view(&mut self, view: LibraryView) -> std::io::Result<()> {
        if self.ui_prefs.library_view == view {
            return Ok(());
        }
        self.ui_prefs.library_view = view;
        self.save_ui_prefs()
    }

    /// Collapses or expands the sidebar for one orientation and persists a change.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the preferences cannot be saved; the choice stays in effect.
    pub fn set_sidebar_collapsed(
        &mut self,
        portrait: bool,
        collapsed: bool,
    ) -> std::io::Result<()> {
        let slot = if portrait {
            &mut self.ui_prefs.sidebar_collapsed_portrait
        } else {
            &mut self.ui_prefs.sidebar_collapsed_landscape
        };
        if *slot == collapsed {
            return Ok(());
        }
        *slot = collapsed;
        self.save_ui_prefs()
    }

    fn save_ui_prefs(&self) -> std::io::Result<()> {
        match &self.ui_prefs_path {
            Some(path) => self.ui_prefs.save(path),
            None => Ok(()),
        }
    }

    /// Uses `cache` to look up row thumbnails.
    pub fn set_thumbnail_cache(&mut self, cache: ThumbnailCache) {
        self.thumbnails = Some(cache);
    }

    /// The thumbnail cache, once the Android entry point has created it.
    #[must_use]
    pub fn thumbnail_cache(&self) -> Option<&ThumbnailCache> {
        self.thumbnails.as_ref()
    }

    /// Records that the score with `note_key` was opened.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the list cannot be saved; the in-memory list is still updated.
    pub fn record_opened(&mut self, note_key: &str) -> std::io::Result<()> {
        self.recent.record(note_key);
        match &self.recent_path {
            Some(path) => self.recent.save(path),
            None => Ok(()),
        }
    }

    /// Content keys of recently opened scores, newest first.
    #[must_use]
    pub fn recent(&self) -> &[String] {
        self.recent.keys()
    }

    /// The side-panel section on display.
    #[must_use]
    pub fn section(&self) -> &LibrarySection {
        &self.section
    }

    /// The large title of the section on display; an open set's title is its name.
    #[must_use]
    pub fn section_title(&self) -> SectionTitle {
        self.section.title(&self.sets)
    }

    /// Switches section, closes the details pane and remembers the section.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the preferences cannot be saved; the section is still shown.
    pub fn select_section(&mut self, section: LibrarySection) -> std::io::Result<()> {
        self.section = section;
        self.reordering = false;
        self.selected = None;
        let key = self.section.key();
        if self.ui_prefs.section == key && self.ui_prefs.selection.is_none() {
            return Ok(());
        }
        self.ui_prefs.section = key;
        self.ui_prefs.selection = None;
        self.save_ui_prefs()
    }

    /// Shows All instead of a composer none of the loaded rows has. While no rows are loaded yet
    /// the composer is kept, since a saved choice is restored before the library arrives.
    fn forget_missing_composer(&mut self) {
        if matches!(self.section, LibrarySection::Composer(_))
            && !self.search_rows.is_empty()
            && section_rows(&self.search_rows, &self.section, &self.section_context()).is_empty()
        {
            self.section = LibrarySection::All;
        }
    }

    /// Rows for the current screen: search hits on the Search screen, the section otherwise.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<SearchRow> {
        if matches!(self.state().screen(), crate::state::Screen::Search) {
            self.search_rows.clone()
        } else {
            section_rows(&self.search_rows, &self.section, &self.section_context())
        }
    }

    /// Opens the details pane on the row with selection id `id` ([`row_id`]: a set row's item
    /// id, else the note key), or closes it with `None`. The choice is remembered, so the details
    /// reopen after Android ends the process.
    pub fn select_note(&mut self, id: Option<String>) {
        self.selected = id;
        self.remember_selection();
    }

    /// Saves the details' score with the UI preferences, when it changed.
    fn remember_selection(&mut self) {
        if self.ui_prefs.selection == self.selected {
            return;
        }
        self.ui_prefs.selection.clone_from(&self.selected);
        if let Err(error) = self.save_ui_prefs() {
            eprintln!("notae: cannot save UI preferences: {error}");
        }
    }

    /// The row shown in the details pane, if it is still in the library. Inside a set this is
    /// the selected copy of the piece, carrying its item id.
    #[must_use]
    pub fn selected_row(&self) -> Option<SearchRow> {
        let id = self.selected.as_deref()?;
        self.search_rows
            .iter()
            .find(|row| row_id(row) == id)
            .cloned()
            .or_else(|| {
                section_rows(&self.search_rows, &self.section, &self.section_context())
                    .into_iter()
                    .find(|row| !row.missing && row_id(row) == id)
            })
    }

    /// Closes the preview when its score left the library (or its set), so it cannot reopen if
    /// the same id later returns.
    fn forget_missing_selection(&mut self) {
        if self.selected.is_some() && self.selected_row().is_none() {
            self.selected = None;
            self.remember_selection();
        }
    }

    /// The previewed score's place among the current section's rows. `None` when no preview
    /// is open or the score is not in the section (for example a Search result).
    #[must_use]
    pub fn preview_neighbours(&self) -> Option<Neighbours> {
        let id = self.selected.as_deref()?;
        neighbours(
            &section_rows(&self.search_rows, &self.section, &self.section_context()),
            id,
        )
    }

    /// The metadata currently shown by the editor, if one is open.
    #[must_use]
    pub fn metadata_editor(&self) -> Option<&EditableMetadata> {
        self.metadata_editor.as_ref()
    }

    /// Error from the latest metadata load/save attempt, if any.
    #[must_use]
    pub fn metadata_error(&self) -> Option<&Failure> {
        self.metadata_error.as_ref()
    }

    /// Closes the editor and clears its error message.
    pub fn close_metadata_editor(&mut self) {
        self.metadata_editor = None;
        self.metadata_error = None;
    }

    /// Opens an editor while the worker loads the selected record.
    pub fn begin_metadata_edit(&mut self, note_key: String) {
        self.metadata_error = None;
        self.metadata_editor = Some(EditableMetadata {
            note_key,
            ..EditableMetadata::default()
        });
    }

    /// Applies a worker event to the UI-owned state and row projection.
    pub fn apply_operation_event(&mut self, event: OperationEvent) -> Result<(), EventError> {
        match event {
            OperationEvent::Started => {
                self.controller.set_rescan_phase(RescanPhase::Enumerating)?
            }
            OperationEvent::Indexing { total } => self
                .controller
                .set_rescan_phase(RescanPhase::Indexing { total })?,
            OperationEvent::RescanFinalizing { total } => {
                self.controller.begin_rescan_finalization(total)?;
            }
            OperationEvent::Progress { completed, total } => {
                self.controller.update_rescan_progress(completed, total)?;
            }
            OperationEvent::FileSucceeded { .. } | OperationEvent::FileFailed { .. } => {}
            OperationEvent::Canceled => self.controller.cancel_operation()?,
            OperationEvent::Completed { summary, rows } => {
                self.latest_rescan = Some(summary);
                self.search_rows = rows;
                self.relink_saved_pieces();
                self.startup_presentation = StartupPresentation::Ready;
                self.controller.complete_operation(summary);
            }
            OperationEvent::SearchRows(rows) => {
                self.startup_presentation = StartupPresentation::Ready;
                self.search_rows = rows;
                self.relink_saved_pieces();
            }
            OperationEvent::PickerRows { query, rows } => {
                // Rows for an older query, or a closed picker, are stale.
                if let Some(picker) = &mut self.add_picker
                    && picker.query == query
                {
                    picker.rows = Some(rows);
                }
            }
            OperationEvent::IndexedCount(count) => self.indexed_count = count,
            OperationEvent::MetadataLoaded(metadata) => {
                self.metadata_error = None;
                self.metadata_editor = Some(metadata);
            }
            OperationEvent::MetadataSaved { rows } => {
                self.search_rows = rows;
                self.relink_saved_pieces();
                self.close_metadata_editor();
            }
            OperationEvent::MetadataFailed { failure } => {
                self.metadata_error = Some(failure);
            }
            OperationEvent::Failed { failure } => {
                self.controller.fail_operation(failure);
                self.startup_presentation = StartupPresentation::Ready;
            }
        }
        // Until the first rows arrive, a restored selection cannot be checked against them.
        if self.startup_presentation == StartupPresentation::Ready {
            self.forget_missing_selection();
        }
        self.forget_missing_composer();
        Ok(())
    }

    /// Clears a successful result once it has been visible for [`RESULT_VISIBLE_FOR`].
    ///
    /// `now` is sampled by the caller; the first call that sees a result starts its clock.
    /// Failures are never cleared here. Returns whether the banner
    /// changed.
    pub fn expire_finished_result(&mut self, now: std::time::Instant) -> bool {
        if !matches!(
            self.controller.state().operation(),
            OperationState::Completed { .. }
        ) {
            self.result_shown_at = None;
            return false;
        }
        let shown_at = *self.result_shown_at.get_or_insert(now);
        if now.saturating_duration_since(shown_at) < RESULT_VISIBLE_FOR {
            return false;
        }
        self.result_shown_at = None;
        self.controller
            .dispatch(UiEvent::AcknowledgeOperation)
            .is_ok()
    }

    /// Whether a new library scan can be started from shared app chrome.
    #[must_use]
    pub fn rescan_enabled(&self) -> bool {
        self.controller.state().operation() == &crate::state::OperationState::Idle
    }

    /// Operation and library status for the banner and Settings.
    #[must_use]
    pub fn status(&self) -> UiStatus {
        UiStatus {
            operation: self.controller.state().operation().clone(),
            latest_rescan: self.latest_rescan,
            indexed_count: self.indexed_count,
            no_text_count: self
                .search_rows
                .iter()
                .filter(|row| row.text_pending)
                .count(),
        }
    }

    /// Returns the current [`AppState`].
    #[must_use]
    pub fn state(&self) -> &AppState {
        self.controller.state()
    }

    /// Apply a `UiEvent` to the underlying controller. Convenience for
    /// the UI's callback wiring.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::OperationAlreadyInProgress`] when an event
    /// would overlap an existing rescan. The UI is supposed to
    /// disable the relevant controls while the operation is in flight,
    /// so this is a guard rather than a control-flow signal.
    pub fn dispatch(&mut self, event: UiEvent) -> Result<Transition, EventError> {
        self.controller.dispatch(event)
    }

    /// The "Rescan" button handler. Forwards to
    /// [`AppController::dispatch`] with [`UiEvent::RequestRescan`].
    pub fn start_rescan(&mut self) -> Result<Transition, EventError> {
        self.dispatch(UiEvent::RequestRescan)
    }
}

/// The "Add to set…" checklist: the piece and the set order it opened with.
struct AddToSet {
    key: String,
    order: Vec<String>,
}

/// The add picker's session over one set.
struct AddPicker {
    set_id: String,
    /// Content keys of the pieces in the set when the picker opened.
    before: HashSet<String>,
    /// `(content key, item id)` of each piece this session added, oldest first.
    added: Vec<(String, String)>,
    query: String,
    /// The worker's rows for `query`; `None` lists the whole library.
    rows: Option<Vec<SearchRow>>,
}

fn log_prefs_error(result: std::io::Result<()>) {
    if let Err(error) = result {
        eprintln!("notae: cannot save UI preferences: {error}");
    }
}

/// The current time in Unix seconds; 0 if the clock is before 1970.
#[must_use]
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl Default for UiModel {
    fn default() -> Self {
        Self::new()
    }
}

/// What the shared operation banner shows, projected once from [`OperationState`] so
/// Library, Search and Settings always agree.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationBanner {
    /// The work in progress or its outcome; `None` when idle.
    pub message: Option<StatusMessage>,
    /// Whether work is in progress.
    pub busy: bool,
    /// Whether the banner reports a failure.
    pub failed: bool,
    /// Determinate progress in `0.0..=1.0` derived from worker counts; `None` while no real
    /// total exists (the bar is then indeterminate). `Some(0.0)` is a known, empty `0 of N`.
    pub fraction: Option<f32>,
    /// Whether the user can still cancel.
    pub cancel_enabled: bool,
    /// Whether the user can dismiss an outcome.
    pub acknowledge_enabled: bool,
}

impl OperationBanner {
    /// Projects `operation` into banner fields.
    #[must_use]
    pub fn project(operation: &OperationState) -> Self {
        let message = match operation {
            OperationState::Idle => None,
            OperationState::Rescan { phase } => Some(match *phase {
                RescanPhase::Enumerating => StatusMessage::ScanningLibrary,
                RescanPhase::Indexing { total } => StatusMessage::Indexing { total },
                RescanPhase::ReadingText { completed, total } => {
                    StatusMessage::ReadingText { completed, total }
                }
            }),
            OperationState::Completed { summary } => Some(StatusMessage::Finished(*summary)),
            OperationState::Failed { failure } => Some(StatusMessage::Failed {
                failure: failure.clone(),
            }),
        };
        let fraction = match operation {
            OperationState::Rescan {
                phase: RescanPhase::ReadingText { completed, total },
            } if *total > 0 => Some((*completed as f32 / *total as f32).clamp(0.0, 1.0)),
            _ => None,
        };
        Self {
            message,
            busy: matches!(operation, OperationState::Rescan { .. }),
            failed: matches!(operation, OperationState::Failed { .. }),
            fraction,
            cancel_enabled: matches!(
                operation,
                OperationState::Rescan { phase } if phase.is_cancellable()
            ),
            acknowledge_enabled: matches!(
                operation,
                OperationState::Completed { .. } | OperationState::Failed { .. }
            ),
        }
    }
}

/// A view over [`StorageState`] that drops the contained source or
/// root. The UI uses it to pick which screen to show without
/// having to clone the underlying `Box<dyn PdfCatalog>` (which is not
/// `Clone`).
#[derive(Debug, Clone)]
pub enum StorageStateKindView {
    /// No library root chosen yet.
    Unconfigured,
    /// A library root is open and the catalog is ready.
    Ready {
        /// The library's display name.
        display_name: String,
    },
    /// The previously-chosen root's persisted permission was revoked.
    /// The string is the root's display name, used as the body text
    /// of the recovery screen.
    PermissionLost(String),
    /// Storage failed in a way the user has to dismiss; the setup
    /// screen shows the error.
    Error(StorageError),
}

impl UiModel {
    /// Returns the [`StorageStateKindView`] of the current storage
    /// state. The view drops the contained source or root.
    ///
    /// It never waits for the worker: the ready root's name comes from the copy
    /// [`UiModel::set_storage`] made.
    #[must_use]
    pub fn storage_kind(&self) -> StorageStateKindView {
        let guard = self.storage.lock().unwrap_or_else(PoisonError::into_inner);
        match &*guard {
            StorageState::Unconfigured => StorageStateKindView::Unconfigured,
            StorageState::Ready(_) => StorageStateKindView::Ready {
                display_name: self
                    .ready_root
                    .as_ref()
                    .map_or_else(String::new, |root| root.display_name().to_owned()),
            },
            StorageState::PermissionLost(root) => {
                StorageStateKindView::PermissionLost(root.display_name().to_owned())
            }
            StorageState::Error(error) => StorageStateKindView::Error(error.clone()),
        }
    }
}
