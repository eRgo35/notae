//! UI state bridge and host-testable library/search view models.
//!
//! `library` and `search` project the shared index into rows the Android
//! surface can display, and compile on Linux for deterministic tests. The
//! `UiModel` keeps storage and application state synchronized; the Slint
//! bindings and Android event loop remain in their Android-only modules.
//!
//! [`state`]: crate::state

#![forbid(unsafe_code)]
pub mod browse;
pub mod library;
pub mod metadata_editor;
pub mod search;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

use crate::favorites::{Favorites, PieceRef};
use crate::language::pick;
use crate::operations::events::{
    DesktopImportCheck, ImportPreviewSummary, ImportSummary, OperationEvent, RescanSummary,
};
use crate::recent::RecentScores;
use crate::sets::{SetFieldError, SetFields, SetStore};
use crate::state::{
    AppController, AppState, EventError, OperationState, RescanPhase, Transition, UiEvent,
};
use crate::storage::StorageState;
use crate::thumbnails::ThumbnailCache;
use crate::ui::browse::{LibrarySection, Neighbours, SectionContext, neighbours, section_rows};
use crate::ui::metadata_editor::EditableMetadata;
use crate::ui::search::SearchRow;
use crate::ui_prefs::{LibraryView, UiPrefs};

/// Bridge between the Slint-generated UI types and the [`AppState`].
///
/// The glue is intentionally thin: it owns an [`AppController`]
/// (which holds the in-memory screen / library / search state) and a
/// [`StorageState`] (which holds the SAF-side root and source). The
/// [`StorageState`] is what the picker manipulates; the
/// [`AppController`] is what the rest of the UI dispatches against.
/// They are kept distinct so the storage layer can be swapped (host
/// test vs. device) without touching the state machine the UI
/// consumes.
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
    pub operation: crate::state::OperationState,
    pub latest_rescan: Option<RescanSummary>,
    pub indexed_count: usize,
    /// Indexed PDFs with no extractable text; shown in Settings as a plain count.
    pub no_text_count: usize,
}

pub struct UiModel {
    controller: AppController,
    storage: Rc<Mutex<StorageState>>,
    search_rows: Vec<SearchRow>,
    indexed_count: usize,
    import_preview_summary: Option<ImportPreviewSummary>,
    latest_rescan: Option<RescanSummary>,
    metadata_editor: Option<EditableMetadata>,
    metadata_error: Option<String>,
    pdf_launch_error: Option<String>,
    section: LibrarySection,
    recent: RecentScores,
    recent_path: Option<PathBuf>,
    favorites: Favorites,
    favorites_path: Option<PathBuf>,
    sets: SetStore,
    sets_path: Option<PathBuf>,
    /// The set whose deletion waits for the user's confirmation.
    pending_delete_set: Option<String>,
    ui_prefs: UiPrefs,
    ui_prefs_path: Option<PathBuf>,
    selected_note: Option<String>,
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
    pub fn from_parts(controller: AppController, storage: Rc<Mutex<StorageState>>) -> Self {
        Self {
            controller,
            storage,
            search_rows: Vec::new(),
            indexed_count: 0,
            import_preview_summary: None,
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
            ui_prefs: UiPrefs::default(),
            ui_prefs_path: None,
            selected_note: None,
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
            Rc::new(Mutex::new(StorageState::Unconfigured)),
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
    // Storage state is UI-thread-owned; this lock protects brief, callback-free access.
    #[allow(clippy::expect_used)]
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
        *self.storage.lock().expect("storage mutex poisoned") = state;
    }

    /// Returns the persisted SAF tree URI for the active library.
    #[must_use]
    pub fn library_root_handle(&self) -> Option<String> {
        let guard = self.storage.lock().ok()?;
        match &*guard {
            StorageState::Ready(source) => source
                .lock()
                .ok()
                .map(|source| source.root().handle().to_owned()),
            StorageState::Unconfigured
            | StorageState::PermissionLost(_)
            | StorageState::Error(_) => None,
        }
    }

    /// Returns the latest PDF launch error, when a viewer could not be opened.
    #[must_use]
    pub fn pdf_launch_error(&self) -> Option<&str> {
        self.pdf_launch_error.as_deref()
    }

    /// Replaces the user-facing PDF launch error.
    pub fn set_pdf_launch_error(&mut self, error: Option<String>) {
        self.pdf_launch_error = error;
    }

    /// Replaces the rows currently displayed by the library screen.
    ///
    /// The operation worker owns the index and sends its search projection
    /// back to the UI thread through this method.
    pub fn set_search_rows(&mut self, rows: Vec<SearchRow>) {
        self.search_rows = rows;
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

    /// Validates `fields`, adds the set and opens it.
    ///
    /// # Errors
    ///
    /// The validation error; nothing is created. A failed save is logged and the set stays in
    /// memory.
    pub fn create_set(&mut self, fields: SetFields) -> Result<String, SetFieldError> {
        let fields = fields.validate()?;
        let id = self.sets.create(fields, unix_now());
        self.save_sets();
        log_prefs_error(self.select_section(LibrarySection::Set(id.clone())));
        Ok(id)
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
    /// section. A saved set that no longer exists opens the list of sets.
    pub fn set_ui_prefs_path(&mut self, path: PathBuf) {
        self.ui_prefs = UiPrefs::load(&path);
        self.ui_prefs_path = Some(path);
        self.section = match LibrarySection::from_key(&self.ui_prefs.section) {
            LibrarySection::Set(id) if self.sets.get(&id).is_none() => LibrarySection::Sets,
            section => section,
        };
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

    /// Remembers the Settings language choice where Android cannot store it.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the preferences cannot be saved; the choice stays in effect.
    pub fn set_language(
        &mut self,
        language: crate::language::LanguageChoice,
    ) -> std::io::Result<()> {
        if self.ui_prefs.language == language {
            return Ok(());
        }
        self.ui_prefs.language = language;
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
    pub fn section_title(&self) -> String {
        self.section.title(&self.sets)
    }

    /// Switches section, closes the details pane and remembers the section.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the preferences cannot be saved; the section is still shown.
    pub fn select_section(&mut self, section: LibrarySection) -> std::io::Result<()> {
        self.section = section;
        self.selected_note = None;
        let key = self.section.key();
        if self.ui_prefs.section == key {
            return Ok(());
        }
        self.ui_prefs.section = key;
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

    /// Opens (`Some`) or closes (`None`) the details pane.
    pub fn select_note(&mut self, note_key: Option<String>) {
        self.selected_note = note_key;
    }

    /// The row shown in the details pane, if it is still in the library.
    #[must_use]
    pub fn selected_row(&self) -> Option<SearchRow> {
        let key = self.selected_note.as_deref()?;
        self.search_rows
            .iter()
            .find(|row| row.note_key == key)
            .cloned()
    }

    /// Closes the preview when its score left the library, so it cannot reopen if the
    /// same key later returns.
    fn forget_missing_selection(&mut self) {
        if let Some(key) = self.selected_note.as_deref()
            && !self.search_rows.iter().any(|row| row.note_key == key)
        {
            self.selected_note = None;
        }
    }

    /// The previewed score's place among the current section's rows. `None` when no preview
    /// is open or the score is not in the section (for example a Search result).
    #[must_use]
    pub fn preview_neighbours(&self) -> Option<Neighbours> {
        let key = self.selected_note.as_deref()?;
        neighbours(
            &section_rows(&self.search_rows, &self.section, &self.section_context()),
            key,
        )
    }

    /// The current read-only sidecar preview counts, when one is awaiting
    /// confirmation.
    #[must_use]
    pub fn import_preview_summary(&self) -> Option<&ImportPreviewSummary> {
        self.import_preview_summary.as_ref()
    }

    /// Hides a sidecar preview that the user declined to apply.
    pub fn clear_import_preview(&mut self) {
        self.import_preview_summary = None;
    }

    /// The metadata currently shown by the editor, if one is open.
    #[must_use]
    pub fn metadata_editor(&self) -> Option<&EditableMetadata> {
        self.metadata_editor.as_ref()
    }

    /// Error from the latest metadata load/save attempt, if any.
    #[must_use]
    pub fn metadata_error(&self) -> Option<&str> {
        self.metadata_error.as_deref()
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
            OperationEvent::CheckingDesktopImport => self
                .controller
                .set_rescan_phase(RescanPhase::CheckingDesktopImport)?,
            OperationEvent::Progress { completed, total } => {
                self.controller.update_rescan_progress(completed, total)?;
            }
            OperationEvent::FileSucceeded { .. } | OperationEvent::FileFailed { .. } => {}
            OperationEvent::Canceled => self.controller.cancel_operation()?,
            OperationEvent::Completed {
                summary,
                rows,
                desktop_import,
            } => {
                self.latest_rescan = Some(summary);
                self.search_rows = rows;
                self.startup_presentation = StartupPresentation::Ready;
                match desktop_import {
                    DesktopImportCheck::NoSidecar => {
                        self.controller.complete_operation(rescan_summary(&summary));
                    }
                    DesktopImportCheck::PreviewReady(preview) => {
                        let candidate_count = preview.new + preview.changed;
                        self.import_preview_summary = Some(preview);
                        self.controller.import_preview_ready(candidate_count);
                    }
                    DesktopImportCheck::Failed { reason } => {
                        let scan = rescan_summary(&summary);
                        self.controller.fail_operation(pick(
                            format!(
                                "Scan finished ({scan}), but the desktop sidecar check failed: {reason}"
                            ),
                            format!(
                                "Skanowanie zakończone ({scan}), ale sprawdzenie pliku sidecar nie powiodło się: {reason}"
                            ),
                        ));
                    }
                }
            }
            OperationEvent::ImportPreviewReady { summary } => {
                let candidate_count = summary.new + summary.changed;
                self.import_preview_summary = Some(summary);
                self.controller.import_preview_ready(candidate_count);
            }
            OperationEvent::ImportApplied { summary, rows } => {
                self.import_preview_summary = None;
                self.search_rows = rows;
                self.controller.complete_operation(import_summary(&summary));
            }
            OperationEvent::SearchRows(rows) => {
                self.startup_presentation = StartupPresentation::Ready;
                self.search_rows = rows;
            }
            OperationEvent::IndexedCount(count) => self.indexed_count = count,
            OperationEvent::MetadataLoaded(metadata) => {
                self.metadata_error = None;
                self.metadata_editor = Some(metadata);
            }
            OperationEvent::MetadataSaved { rows } => {
                self.search_rows = rows;
                self.close_metadata_editor();
            }
            OperationEvent::MetadataFailed { reason } => {
                self.metadata_error = Some(reason);
            }
            OperationEvent::Failed { reason } => {
                self.controller.fail_operation(reason);
                self.startup_presentation = StartupPresentation::Ready;
            }
        }
        self.forget_missing_selection();
        self.forget_missing_composer();
        Ok(())
    }

    /// Clears a successful result once it has been visible for [`RESULT_VISIBLE_FOR`].
    ///
    /// `now` is sampled by the caller; the first call that sees a result starts its clock.
    /// Failures and pending decisions are never cleared here. Returns whether the banner
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
    /// the Slint callback wiring in `app.slint`.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::OperationAlreadyInProgress`] when an event
    /// would overlap an existing rescan/import. The UI is supposed to
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
    /// One-line description of the work or outcome; empty when idle.
    pub text: String,
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
        let text = match operation {
            OperationState::Idle => String::new(),
            OperationState::Rescan { phase } => match phase {
                RescanPhase::Enumerating => {
                    pick("Scanning library…", "Skanowanie biblioteki…").to_owned()
                }
                RescanPhase::Indexing { total: 1 } => {
                    pick("Indexing 1 PDF…", "Indeksowanie 1 pliku PDF…").to_owned()
                }
                RescanPhase::Indexing { total } => pick(
                    format!("Indexing {total} PDFs…"),
                    format!("Indeksowanie {total} plików PDF…"),
                ),
                RescanPhase::ReadingText { completed, total } => pick(
                    format!("Reading PDF text and thumbnails: {completed} of {total}"),
                    format!("Odczyt tekstu i miniatur PDF: {completed} z {total}"),
                ),
                RescanPhase::CheckingDesktopImport => pick(
                    "Checking for a desktop sidecar…",
                    "Sprawdzanie pliku sidecar z komputera…",
                )
                .to_owned(),
            },
            OperationState::ImportPreviewPending { candidate_count } => pick(
                format!("Review {candidate_count} proposed sidecar changes"),
                format!("Przejrzyj proponowane zmiany z pliku sidecar: {candidate_count}"),
            ),
            OperationState::ImportApplying => pick(
                "Applying sidecar import…",
                "Stosowanie importu z pliku sidecar…",
            )
            .to_owned(),
            OperationState::Completed { summary } => pick(
                format!("Finished: {summary}"),
                format!("Zakończono: {summary}"),
            ),
            OperationState::Failed { reason } => reason.clone(),
        };
        let fraction = match operation {
            OperationState::Rescan {
                phase: RescanPhase::ReadingText { completed, total },
            } if *total > 0 => Some((*completed as f32 / *total as f32).clamp(0.0, 1.0)),
            _ => None,
        };
        Self {
            text,
            busy: matches!(
                operation,
                OperationState::Rescan { .. } | OperationState::ImportApplying
            ),
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

fn rescan_summary(summary: &RescanSummary) -> String {
    let mut parts = Vec::new();
    add_count(&mut parts, summary.added, "added", "dodano");
    add_count(&mut parts, summary.updated, "updated", "zaktualizowano");
    add_count(&mut parts, summary.moved, "moved", "przeniesiono");
    add_count(&mut parts, summary.deleted, "deleted", "usunięto");
    add_count(
        &mut parts,
        summary.conflicts,
        "ID conflicts",
        "konflikty numerów",
    );
    add_count(&mut parts, summary.failures, "failures", "błędy");
    if parts.is_empty() {
        pick("No changes", "Bez zmian").to_owned()
    } else {
        parts.join(", ")
    }
}

fn import_summary(summary: &ImportSummary) -> String {
    let mut parts = Vec::new();
    add_count(&mut parts, summary.imported, "imported", "zaimportowano");
    add_count(
        &mut parts,
        summary.mismatches,
        "hash mismatches",
        "niezgodne sumy kontrolne",
    );
    add_count(&mut parts, summary.missing, "missing", "brakujące");
    add_count(&mut parts, summary.unreadable, "unreadable", "nieczytelne");
    add_count(
        &mut parts,
        summary.conflicts,
        "ID conflicts",
        "konflikty numerów",
    );
    add_count(&mut parts, summary.failures, "failures", "błędy");
    if parts.is_empty() {
        pick("No import changes", "Import bez zmian").to_owned()
    } else {
        parts.join(", ")
    }
}

/// English reads "3 added"; Polish reads "dodano: 3", which needs no plural agreement.
fn add_count(parts: &mut Vec<String>, count: usize, english: &str, polish: &str) {
    if count != 0 {
        parts.push(pick(
            format!("{count} {english}"),
            format!("{polish}: {count}"),
        ));
    }
}

/// A view over [`StorageState`] that drops the contained source or
/// root. The Slint layer uses it to pick which screen to show without
/// having to clone the underlying `Box<dyn PdfCatalog>` (which is not
/// `Clone`).
#[derive(Debug, Clone)]
pub enum StorageStateKindView {
    /// No library root chosen yet.
    Unconfigured,
    /// A library root is open and the catalog is ready.
    Ready { display_name: String },
    /// The previously-chosen root's persisted permission was revoked.
    /// The string is the root's display name, used as the body text
    /// of the recovery screen.
    PermissionLost(String),
    /// Storage failed in a way the user has to dismiss. The string is
    /// the user-facing error message the setup screen surfaces.
    Error(String),
}

impl UiModel {
    /// Returns the [`StorageStateKindView`] of the current storage
    /// state. The view drops the contained source or root.
    #[must_use]
    // Storage state is UI-thread-owned; this read invokes no user callback.
    #[allow(clippy::expect_used)]
    pub fn storage_kind(&self) -> StorageStateKindView {
        let guard = self.storage.lock().expect("storage mutex poisoned");
        match &*guard {
            StorageState::Unconfigured => StorageStateKindView::Unconfigured,
            StorageState::Ready(source) => StorageStateKindView::Ready {
                display_name: source
                    .lock()
                    .expect("library source mutex poisoned")
                    .root()
                    .display_name()
                    .to_owned(),
            },
            StorageState::PermissionLost(root) => {
                StorageStateKindView::PermissionLost(root.display_name().to_owned())
            }
            StorageState::Error(error) => StorageStateKindView::Error(error.to_string()),
        }
    }
}
