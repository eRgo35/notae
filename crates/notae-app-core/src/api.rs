//! The UniFFI API the Kotlin app talks to: one [`NotaeCore`] object.
//!
//! Kotlin builds it with the app's data directory, its [`PlatformLibrary`] and a
//! [`CoreListener`]. It reports what the user did with [`NotaeCore::dispatch`] and
//! [`NotaeCore::back`], and the outcome of platform work it was asked for with the other
//! methods. The core answers with a [`Snapshot`] after every change and asks for platform work
//! with an [`Effect`].
//!
//! # Threads
//!
//! The state lives behind one mutex. [`NotaeCore::dispatch`] applies an action under it,
//! releases it, then calls the listener on the caller's thread; it never calls the
//! [`PlatformLibrary`] and never waits for a platform call: the worker holds the library
//! source's lock while it calls the platform, and nothing on this path takes that lock. A core
//! thread (`notae-core`) opens the library root (the only
//! [`PlatformLibrary::check_root`] call), drains the operation worker's events, applies each
//! batch under the lock and pushes a snapshot after releasing it. The worker thread makes every
//! other platform call. Because the listener is never called with the lock held, it may call
//! back into the core. Snapshots from different threads can arrive out of order: keep the one
//! with the highest [`Snapshot::revision`]. Dropping the last reference stops the core thread
//! and the worker.
//!
//! # Data directory
//!
//! `recent.txt`, `ui-prefs.txt`, `favorites.json`, `sets.json`, `index.sqlite`, `thumbnails/`,
//! and `library-root.json`, the open folder, so a new core over the same directory reopens it.

mod action;
mod effect;
mod snapshot;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notae_index::IndexStore;

pub use action::Action;
pub use effect::Effect;
pub use snapshot::{
    AddPicker, AddToSetSheet, Banner, ComposerEntry, DeleteSetConfirmation, Edited, ExportStatus,
    LayoutPrefs, MetadataDraft, Origin, PdfOpenError, PositionOf, Row, ScanSummary, Screen,
    SearchBox, SearchScope, SectionEntry, SectionTitle, Selection, SetCard, SetChoice, SetDialog,
    SetEntry, SettingsInfo, SetupError, SetupScreen, Snapshot, Snippet, StatusMessage, Subtitle,
};

use crate::operations::events::OperationEvent;
use crate::operations::rescan::OperationController;
use crate::platform::{PlatformCatalog, PlatformLibrary};
use crate::sets::SetFields;
use crate::state::{EventError, Transition, UiEvent};
use crate::storage::{LibraryRoot, LibraryRootId, LibrarySource, SharedLibraryCatalog};
use crate::storage::{StorageError, StorageState};
use crate::text::Failure;
use crate::thumbnails::ThumbnailCache;
use crate::ui::browse::LibrarySection;
use crate::ui::metadata_editor::EditableMetadata;
use crate::ui::search::UiError;
use crate::ui::{StartupPresentation, StorageStateKindView, UiModel};

/// How long the core thread waits for a worker event before it checks for other work.
const POLL: Duration = Duration::from_millis(50);

/// The file that remembers the open library folder.
const ROOT_FILE: &str = "library-root.json";

/// What the core tells Kotlin. Both methods may be called on any thread.
#[uniffi::export(with_foreign)]
pub trait CoreListener: Send + Sync {
    /// The state changed; draw `snapshot` unless a newer revision is already shown.
    fn on_snapshot(&self, snapshot: Snapshot);

    /// Do this platform work.
    fn on_effect(&self, effect: Effect);
}

/// The app's state and logic, owned by the Kotlin view model.
#[derive(uniffi::Object)]
pub struct NotaeCore {
    shared: Arc<Shared>,
    core_thread: Option<JoinHandle<()>>,
}

#[uniffi::export]
impl NotaeCore {
    /// Loads the stores in `data_dir` and reopens the saved library folder, if any, on the core
    /// thread. Until that check finishes the snapshot shows [`Screen::Loading`].
    #[uniffi::constructor]
    pub fn new(
        data_dir: String,
        platform: Arc<dyn PlatformLibrary>,
        listener: Arc<dyn CoreListener>,
    ) -> Arc<Self> {
        let data_dir = PathBuf::from(data_dir);
        if let Err(error) = fs::create_dir_all(&data_dir) {
            eprintln!("notae: cannot create {}: {error}", data_dir.display());
        }
        let mut model = UiModel::new();
        model.set_recent_path(data_dir.join("recent.txt"));
        model.set_favorites_path(data_dir.join("favorites.json"));
        // Before the UI preferences, which reopen a saved set only if it still exists.
        model.set_sets_path(data_dir.join("sets.json"));
        model.set_ui_prefs_path(data_dir.join("ui-prefs.txt"));
        match ThumbnailCache::new(data_dir.join("thumbnails")) {
            Ok(cache) => model.set_thumbnail_cache(cache),
            Err(error) => eprintln!("notae: no thumbnail directory: {error}"),
        }
        let saved_root = load_root(&data_dir.join(ROOT_FILE));
        if saved_root.is_none() {
            model.finish_startup_without_library();
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(CoreState {
                model,
                operations: None,
                root: None,
                open_request: saved_root,
                revision: 0,
                set_dialog: None,
                export: None,
                opening_pdf: None,
            }),
            platform,
            listener,
            data_dir,
            stop: AtomicBool::new(false),
        });
        let core_thread = thread::Builder::new().name("notae-core".to_owned()).spawn({
            let shared = Arc::clone(&shared);
            move || shared.run()
        });
        let core_thread = match core_thread {
            Ok(handle) => Some(handle),
            Err(error) => {
                // Without the core thread no library can open; say so on the setup screen.
                let mut state = shared.lock();
                state.open_request = None;
                state
                    .model
                    .set_storage(StorageState::Error(StorageError::Other(error.to_string())));
                None
            }
        };
        Arc::new(Self {
            shared,
            core_thread,
        })
    }

    /// Applies a user action, then sends the snapshot and any effects it asks for.
    pub fn dispatch(&self, action: Action) {
        let (snapshot, effects) = {
            let mut state = self.shared.lock();
            let effects = state.apply(action);
            (state.next_snapshot(), effects)
        };
        self.shared.listener.on_snapshot(snapshot);
        for effect in effects {
            self.shared.listener.on_effect(effect);
        }
    }

    /// Closes the innermost thing open: a sheet, a dialog, the details, search or settings,
    /// then an open set (back to the list of sets). Returns `false` when nothing is left, so
    /// Kotlin lets the system finish the activity.
    pub fn back(&self) -> bool {
        let snapshot = {
            let mut state = self.shared.lock();
            if !state.back() {
                return false;
            }
            state.next_snapshot()
        };
        self.shared.listener.on_snapshot(snapshot);
        true
    }

    /// The current state, for the first frame.
    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().snapshot()
    }

    /// The folder picker returned a folder, with its permission already taken. The core thread
    /// checks and opens it, and saves it for the next launch.
    pub fn library_root_picked(&self, display_name: String, handle: String) {
        self.shared.lock().open_request = Some(LibraryRoot::new(display_name, handle));
        if let Some(core_thread) = &self.core_thread {
            core_thread.thread().unpark();
        }
    }

    /// The folder picker could not run, or its folder could not be kept. The setup screen shows
    /// `detail` as a storage error, whether or not a library was open, as the Slint app does.
    pub fn library_root_pick_failed(&self, detail: String) {
        let snapshot = {
            let mut state = self.shared.lock();
            state
                .model
                .set_storage(StorageState::Error(StorageError::Other(detail)));
            state.next_snapshot()
        };
        self.shared.listener.on_snapshot(snapshot);
    }

    /// The export from [`Effect::WriteExport`] was saved (`None`), or failed with the
    /// platform's message. A cancelled save-file picker reports nothing.
    pub fn file_written(&self, error: Option<String>) {
        let snapshot = {
            let mut state = self.shared.lock();
            state.export = Some(match error {
                None => ExportStatus::Exported,
                Some(detail) => ExportStatus::Failed { detail },
            });
            state.next_snapshot()
        };
        self.shared.listener.on_snapshot(snapshot);
    }

    /// The file chosen after [`Effect::PickImportFile`] was read. The import flow that asks
    /// for it, and uses the text, arrives with the import actions; until then nothing asks
    /// for a file and the text is ignored.
    pub fn import_file_read(&self, text: String) {
        drop(text);
    }

    /// The PDF from [`Effect::OpenPdf`] opened (`None`), or why it did not. An opened score
    /// joins Recent; a failure is shown until the next successful open.
    pub fn pdf_opened(&self, error: Option<PdfOpenError>) {
        let snapshot = {
            let mut state = self.shared.lock();
            let opened = state.opening_pdf.take();
            match error {
                None => {
                    state.model.set_pdf_launch_error(None);
                    if let Some(note_key) = opened
                        && let Err(error) = state.model.record_opened(&note_key)
                    {
                        eprintln!("notae: cannot save recent scores: {error}");
                    }
                }
                Some(error) => state.model.set_pdf_launch_error(Some(error.into())),
            }
            state.next_snapshot()
        };
        self.shared.listener.on_snapshot(snapshot);
    }
}

impl Drop for NotaeCore {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(core_thread) = self.core_thread.take() {
            core_thread.thread().unpark();
            // A listener that drops the last reference from a callback runs on the core thread,
            // which stops by itself once it returns.
            if core_thread.thread().id() != thread::current().id() {
                let _ = core_thread.join();
            }
        }
    }
}

/// What the core thread and the calls from Kotlin share.
struct Shared {
    state: Mutex<CoreState>,
    platform: Arc<dyn PlatformLibrary>,
    listener: Arc<dyn CoreListener>,
    data_dir: PathBuf,
    stop: AtomicBool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, CoreState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The core thread: opens requested roots, applies worker events and clears an old
    /// result from the banner, until the core is dropped.
    fn run(&self) {
        while !self.stop.load(Ordering::Acquire) {
            let (open_request, controller) = {
                let mut state = self.lock();
                (state.open_request.take(), state.operations.clone())
            };
            if let Some(root) = open_request {
                self.open_root(root);
                continue;
            }
            let first = match &controller {
                Some(controller) => controller.wait_event(POLL),
                None => {
                    thread::park_timeout(POLL);
                    Ok(None)
                }
            };
            let (snapshot, retired) = {
                let mut state = self.lock();
                let mut changed = false;
                let mut retired = None;
                match (&controller, first) {
                    (Some(controller), Ok(Some(first))) if state.is_current(controller) => {
                        changed = true;
                        retired = state.apply_event(first);
                        while retired.is_none()
                            && let Some(event) = controller.try_event()
                        {
                            retired = state.apply_event(event);
                        }
                    }
                    // The worker died (a panic in a platform call): retire it, so this loop
                    // parks instead of spinning on the closed channel, and say so.
                    (Some(controller), Err(error)) if state.is_current(controller) => {
                        changed = true;
                        retired = state.operations.take();
                        state.apply_event(OperationEvent::Failed {
                            failure: (&error).into(),
                        });
                    }
                    _ => {}
                }
                changed |= state.model.expire_finished_result(Instant::now());
                (changed.then(|| state.next_snapshot()), retired)
            };
            // A retired worker is joined here, outside the lock.
            drop(retired);
            drop(controller);
            if let Some(snapshot) = snapshot {
                self.listener.on_snapshot(snapshot);
            }
        }
    }

    /// Checks `root` with the platform and opens it with a new worker, or shows why it
    /// cannot be opened.
    fn open_root(&self, root: LibraryRoot) {
        // One worker at a time owns the index: stop the old one first.
        let old = self.lock().operations.take();
        if let Some(old) = old {
            let _ = old.cancel();
            drop(old);
        }
        let opened = self
            .platform
            .check_root(root.handle().to_owned())
            .map_err(StorageError::from)
            .and_then(|()| self.start_worker(&root));
        let snapshot = {
            let mut state = self.lock();
            match opened {
                Ok((controller, source)) => {
                    // Before the worker gets its first command, so copying the root out of
                    // the source never waits for it.
                    state.model.set_storage(StorageState::Ready(source));
                    let search = state.model.state().search().clone();
                    if let Err(error) = controller.set_search(search.query, search.scope) {
                        state.apply_event(OperationEvent::Failed {
                            failure: (&error).into(),
                        });
                    }
                    state.operations = Some(Arc::new(controller));
                    if let Err(error) = save_root(&self.data_dir.join(ROOT_FILE), &root) {
                        eprintln!("notae: cannot save the library folder: {error}");
                    }
                    state.root = Some(root);
                }
                Err(StorageError::PermissionLost) => {
                    state
                        .model
                        .set_storage(StorageState::PermissionLost(root.clone()));
                    state.root = Some(root);
                }
                Err(error) => {
                    state.model.set_storage(StorageState::Error(error));
                    state.root = Some(root);
                }
            }
            state.next_snapshot()
        };
        self.listener.on_snapshot(snapshot);
    }

    /// Opens the index and starts the worker over `root`.
    fn start_worker(
        &self,
        root: &LibraryRoot,
    ) -> Result<(OperationController, Arc<Mutex<LibrarySource>>), StorageError> {
        let local = IndexStore::open(self.data_dir.join("index.sqlite"))
            .map_err(|error| StorageError::Other(error.to_string()))?;
        let catalog = PlatformCatalog::new(Arc::clone(&self.platform), root.handle().to_owned());
        let source = Arc::new(Mutex::new(LibrarySource::new(
            root.clone(),
            Box::new(catalog),
        )));
        let worker_catalog = Box::new(SharedLibraryCatalog(Arc::clone(&source)));
        let thumbnails = self.lock().model.thumbnail_cache().cloned();
        let controller = match thumbnails {
            Some(cache) => OperationController::with_thumbnails(local, worker_catalog, cache),
            None => OperationController::new(local, worker_catalog),
        }
        .map_err(|error| StorageError::Other(error.to_string()))?;
        Ok((controller, source))
    }
}

/// Everything behind the core's lock.
struct CoreState {
    model: UiModel,
    /// The worker over the open library.
    operations: Option<Arc<OperationController>>,
    /// The library folder last opened, whether it is open, lost or failed to open.
    root: Option<LibraryRoot>,
    /// A folder the core thread should open next.
    open_request: Option<LibraryRoot>,
    revision: u64,
    set_dialog: Option<SetDialog>,
    export: Option<ExportStatus>,
    /// The content key of the PDF Kotlin was asked to open.
    opening_pdf: Option<String>,
}

impl CoreState {
    fn snapshot(&self) -> Snapshot {
        snapshot::build(
            &self.model,
            &snapshot::Extras {
                revision: self.revision,
                set_dialog: self.set_dialog.as_ref(),
                export: self.export.as_ref(),
            },
        )
    }

    /// Counts a change and builds its snapshot.
    fn next_snapshot(&mut self) -> Snapshot {
        self.revision += 1;
        self.snapshot()
    }

    fn is_current(&self, controller: &Arc<OperationController>) -> bool {
        self.operations
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, controller))
    }

    /// Applies one worker event. A scan that lost the folder's permission shows the
    /// permission-lost screen and retires the worker, which is returned to be dropped outside
    /// the lock.
    fn apply_event(&mut self, event: OperationEvent) -> Option<Arc<OperationController>> {
        let lost = matches!(
            &event,
            OperationEvent::Failed {
                failure: Failure::LibraryPermissionLost
            }
        );
        if let Err(error) = self.model.apply_operation_event(event) {
            eprintln!("notae: rejected worker event: {error}");
        }
        if !lost {
            return None;
        }
        // The setup screen explains the loss; no failure banner waits behind it.
        report(self.model.dispatch(UiEvent::AcknowledgeOperation));
        let root = self.root.clone().unwrap_or_else(LibraryRoot::placeholder);
        self.model.set_storage(StorageState::PermissionLost(root));
        self.operations.take()
    }

    /// Runs `command` on the worker; the worker has stopped when no library is open.
    fn on_worker(
        &self,
        command: impl FnOnce(&OperationController) -> Result<(), UiError>,
    ) -> Result<(), UiError> {
        self.operations
            .as_deref()
            .ok_or(UiError::WorkerStopped)
            .and_then(command)
    }

    fn apply(&mut self, action: Action) -> Vec<Effect> {
        let model = &mut self.model;
        match action {
            Action::PickLibraryRoot => {
                return vec![Effect::PickLibraryRoot {
                    initial_uri: self.root.as_ref().map(|root| root.handle().to_owned()),
                }];
            }
            Action::Rescan => match model.start_rescan() {
                Ok(Transition::BeginRescan) => {
                    if let Err(error) = self.on_worker(OperationController::start_rescan) {
                        self.apply_event(OperationEvent::Failed {
                            failure: (&error).into(),
                        });
                    }
                }
                outcome => report(outcome),
            },
            Action::CancelRescan => {
                if let Err(error) = self.on_worker(OperationController::cancel) {
                    eprintln!("notae: rescan cancellation rejected: {error}");
                }
            }
            Action::AcknowledgeOperation => report(model.dispatch(UiEvent::AcknowledgeOperation)),
            Action::SelectSection { key } => {
                log_prefs(model.select_section(LibrarySection::from_key(&key)));
            }
            Action::OpenSet { set_id } => {
                log_prefs(model.select_section(LibrarySection::Set(set_id)));
            }
            Action::OpenSearch => report(model.dispatch(UiEvent::OpenSearch)),
            Action::SearchChanged { query } => {
                report(model.dispatch(UiEvent::UpdateSearchQuery {
                    query: query.clone(),
                }));
                let scope = model.state().search().scope;
                self.search(|controller| controller.set_search(query, scope));
            }
            Action::SearchScopeChanged { scope } => {
                let scope = scope.into();
                report(model.dispatch(UiEvent::UpdateSearchScope { scope }));
                let query = model.state().search().query.clone();
                self.search(|controller| controller.set_search(query, scope));
            }
            Action::VoiceSearch => return vec![Effect::StartVoiceSearch],
            Action::CloseSearch => self.close_search(None),
            Action::SearchResultChosen { note_key } => self.close_search(Some(note_key)),
            Action::EditMetadata { note_key } => {
                model.begin_metadata_edit(note_key.clone());
                if let Err(error) = self.on_worker(|worker| worker.load_metadata(note_key)) {
                    self.apply_event(OperationEvent::MetadataFailed {
                        failure: (&error).into(),
                    });
                }
            }
            Action::SaveMetadata {
                note_key,
                number,
                title,
                composer,
                tags,
            } => {
                let metadata = EditableMetadata {
                    note_key,
                    id: number,
                    title,
                    composer_or_artist: composer,
                    tags: tags
                        .split(',')
                        .map(str::trim)
                        .filter(|tag| !tag.is_empty())
                        .map(str::to_owned)
                        .collect(),
                };
                if let Err(error) = self.on_worker(|worker| worker.save_metadata(metadata)) {
                    self.apply_event(OperationEvent::MetadataFailed {
                        failure: (&error).into(),
                    });
                }
            }
            Action::CancelMetadata => model.close_metadata_editor(),
            Action::SetLibraryView { view } => log_prefs(model.set_library_view(view)),
            Action::SetSidebarCollapsed {
                portrait,
                collapsed,
            } => log_prefs(model.set_sidebar_collapsed(portrait, collapsed)),
            Action::SelectScore { selection_id } => model.select_note(Some(selection_id)),
            Action::ClearSelection => model.select_note(None),
            Action::ToggleFavorite { note_key } => {
                if let Err(error) = model.toggle_favorite(&note_key) {
                    eprintln!("notae: cannot save favorites: {error}");
                }
            }
            Action::OpenPdf {
                relative_path,
                note_key,
                item_id,
            } => {
                // A score row exists only while a library is open.
                let Some(uri) = model.library_root_handle() else {
                    eprintln!("notae: no library is open to open a PDF from");
                    return Vec::new();
                };
                self.opening_pdf = Some(note_key.clone());
                return vec![Effect::OpenPdf {
                    uri,
                    relative_path,
                    note_key,
                    item_id,
                }];
            }
            Action::OpenSettings => {
                report(model.dispatch(UiEvent::OpenSettings));
                self.export = None;
            }
            Action::CloseSettings => report(model.dispatch(UiEvent::CloseSettings)),
            Action::ExportSets => {
                self.export = None;
                let (suggested_name, json) = model.export_sets(crate::ui::unix_now());
                return vec![Effect::WriteExport {
                    suggested_name,
                    json,
                }];
            }
            Action::NewSet => {
                // A plain New set never carries a piece left from the checklist.
                model.set_dialog_cancelled();
                self.set_dialog = Some(empty_set_dialog());
            }
            Action::EditSet { set_id } => {
                if let Some(set) = model.sets().get(&set_id) {
                    self.set_dialog = Some(SetDialog {
                        set_id: Some(set.id.clone()),
                        name: set.name.clone(),
                        date: set.date.clone(),
                        notes: set.notes.clone(),
                        error: None,
                    });
                }
            }
            Action::SaveSetDialog { name, date, notes } => {
                let Some(dialog) = &mut self.set_dialog else {
                    return Vec::new();
                };
                let fields = SetFields {
                    name: name.clone(),
                    date: date.clone(),
                    notes: notes.clone(),
                };
                let saved = match &dialog.set_id {
                    None => model.create_set(fields).map(drop),
                    Some(id) => model.update_set(id, fields),
                };
                match saved {
                    Ok(()) => self.set_dialog = None,
                    Err(error) => {
                        dialog.name = name;
                        dialog.date = date;
                        dialog.notes = notes;
                        dialog.error = Some(error);
                    }
                }
            }
            Action::CancelSetDialog => {
                model.set_dialog_cancelled();
                self.set_dialog = None;
            }
            Action::DeleteSet { set_id } => model.request_delete_set(&set_id),
            Action::ConfirmDeleteSet => model.confirm_delete_set(),
            Action::CancelDeleteSet => model.cancel_delete_set(),
            Action::AddToSet { note_key } => model.add_to_set_requested(&note_key),
            Action::ToggleSetMembership { set_id, note_key } => {
                model.set_membership_toggled(&set_id, &note_key);
            }
            Action::AddToSetNewSet => {
                model.add_to_set_new_set_requested();
                self.set_dialog = Some(empty_set_dialog());
            }
            Action::CloseAddToSet => model.add_to_set_closed(),
            Action::RemoveSetItem { item_id } => model.remove_set_item_requested(&item_id),
            Action::OpenAddPicker => model.add_picker_opened(),
            Action::AddPickerSearch { query } => {
                model.add_picker_search_changed(&query);
                // A blank query lists the whole library without the worker.
                if !query.trim().is_empty() {
                    self.search(|controller| controller.search_picker(query));
                }
            }
            Action::ToggleAddPickerPiece { note_key } => model.add_picker_toggled(&note_key),
            Action::CloseAddPicker => model.add_picker_closed(),
        }
        Vec::new()
    }

    /// Sends a search to the worker; a failure is logged, as the Slint app does.
    fn search(&self, command: impl FnOnce(&OperationController) -> Result<(), UiError>) {
        if let Err(error) = self.on_worker(command) {
            eprintln!("notae: search failed: {error}");
        }
    }

    /// Leaves search, opening `selected`'s details, and refreshes the library rows.
    fn close_search(&mut self, selected: Option<String>) {
        let scope = self.model.state().search().scope;
        report(self.model.dispatch(UiEvent::CloseSearch));
        self.model.select_note(selected);
        self.search(|controller| controller.set_search(String::new(), scope));
    }

    /// Closes the innermost thing open; `false` when nothing is.
    fn back(&mut self) -> bool {
        let model = &mut self.model;
        let library_open = model.startup_presentation() == StartupPresentation::Ready
            && matches!(model.storage_kind(), StorageStateKindView::Ready { .. });
        if !library_open {
            return false;
        }
        if model.add_to_set_key().is_some() {
            model.add_to_set_closed();
        } else if self.set_dialog.take().is_some() {
            model.set_dialog_cancelled();
        } else if model.pending_delete_set_name().is_some() {
            model.cancel_delete_set();
        } else if model.metadata_editor().is_some() {
            model.close_metadata_editor();
        } else if model.add_picker_open() {
            model.add_picker_closed();
        } else if model.selected_row().is_some() {
            model.select_note(None);
        } else if matches!(model.state().screen(), crate::state::Screen::Search) {
            self.close_search(None);
        } else if matches!(model.state().screen(), crate::state::Screen::Settings) {
            report(model.dispatch(UiEvent::CloseSettings));
        } else if matches!(model.section(), LibrarySection::Set(_)) {
            log_prefs(model.select_section(LibrarySection::Sets));
        } else {
            return false;
        }
        true
    }
}

fn empty_set_dialog() -> SetDialog {
    SetDialog {
        set_id: None,
        name: String::new(),
        date: None,
        notes: None,
        error: None,
    }
}

/// Logs a transition the state machine refused; the UI disables the controls that cause one.
fn report(outcome: Result<Transition, EventError>) {
    if let Err(error) = outcome {
        eprintln!("notae: rejected action: {error}");
    }
}

fn log_prefs(result: io::Result<()>) {
    if let Err(error) = result {
        eprintln!("notae: cannot save UI preferences: {error}");
    }
}

/// The saved library folder; `None` when there is none or the file is unreadable.
fn load_root(path: &Path) -> Option<LibraryRoot> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let field = |name: &str| value.get(name)?.as_str().map(str::to_owned);
    Some(LibraryRoot::from_parts(
        LibraryRootId::from_string(field("id")?),
        field("display_name")?,
        field("handle")?,
    ))
}

/// Saves the library folder through a temporary file.
fn save_root(path: &Path, root: &LibraryRoot) -> io::Result<()> {
    let text = serde_json::json!({
        "id": root.id().as_str(),
        "display_name": root.display_name(),
        "handle": root.handle(),
    })
    .to_string();
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text)?;
    fs::rename(temporary, path)
}
