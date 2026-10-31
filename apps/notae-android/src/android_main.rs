//! Android entry point.
//!
//! `android_main` is the symbol the `android-activity` runtime looks up
//! when `libnotae-android.so` is loaded. It:
//!
//! 1. Initialises the Slint Android platform via `slint::android::init`.
//! 2. Builds an [`AppController`](notae_app_core::state::AppController) — a fresh
//!    one, so the device boots into `Setup::Unconfigured` / `Idle`.
//! 3. Constructs a [`UiModel`](notae_app_core::ui::UiModel) around it and an
//!    empty [`StorageState`](notae_app_core::storage::StorageState).
//! 4. Wires the Slint `AppWindow`'s properties (visible fields of the
//!    state) and callbacks (forwarded as `UiEvent`s).
//! 5. Drives the Storage Access Framework picker through the
//!    [`SafLibraryRootProvider`](crate::android::saf::SafLibraryRootProvider),
//!    and the resulting [`LibraryRoot`](notae_app_core::storage::LibraryRoot)
//!    transitions the storage state from `Unconfigured` to `Ready`.
//! 6. Runs the Slint event loop until the OS asks us to stop.
//!
//! Task 2 adds the SAF half: the picker launches when the user taps
//! "Select folder" (or "Reauthorize folder" on the recovery screen),
//! and the resulting root transitions the storage state so the rest of
//! the app — including the eventual rescan worker — sees the library
//! the same way it would on any other platform.
//!
//! `#![allow(unsafe_code)]` here is intentional and scoped to this
//! file. The Android runtime requires `android_main` to be exported as
//! a C ABI symbol with a stable name; `#[unsafe(no_mangle)]` is the
//! mechanism Rust provides for that, and the runtime guarantees the
//! platform ABI rather than the language's. The rest of the crate
//! (`state.rs`, `ui.rs`) keeps a strict `#![forbid(unsafe_code)]`; the
//! JNI helpers in [`crate::android::saf`] opt back into `unsafe` for
//! themselves only.
//!
//! `#![allow(improper_ctypes_definitions)]` is also targeted at slint's
//! own FFI surface: `slint::android::AndroidApp` is an `Arc<…>`-shaped
//! wrapper that's not `#[repr(C)]`, but slint's own examples declare
//! `extern "C" fn android_main(app: AndroidApp)` with the same shape.
//! The warning is a false positive from rustc; silencing it here keeps
//! the build log clean without affecting the actual ABI.

#![allow(unsafe_code)]
#![allow(improper_ctypes_definitions)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint::ComponentHandle;

use crate::android::saf::{SafHandles, SafLibraryRootProvider};
use notae_app_core::favorites::PieceRef;
use notae_app_core::operations::rescan::OperationController;
use notae_app_core::state::{EventError, OperationState, Screen, Setup, Transition, UiEvent};
use notae_app_core::storage::{
    LibraryRootProvider, SharedLibraryCatalog, StorageError, StorageState,
};
use notae_app_core::thumbnails::ThumbnailCache;
use notae_app_core::ui::UiModel;
use notae_app_core::ui::search::SearchRow;
use notae_index::IndexStore;

// `AppWindow` and `set_*` callbacks are emitted by
// `slint_build::compile("ui/app.slint")` and pulled into the crate root
// at `lib.rs`. They're not in scope here by default — bring them in.
use crate::AppWindow;

/// The symbol `android-activity` exports into our `cdylib`.
///
/// The runtime finds this entry point by name from
/// `android.app.NativeActivity.onCreate`. The `extern "C"` ABI matches
/// what `android-activity` expects; the name is fixed by the contract,
/// not by us — that's exactly what `#[unsafe(no_mangle)]` enforces.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(app: slint::android::AndroidApp) {
    if let Err(error) = run(app) {
        eprintln!("notae: fatal startup error: {error}");
    }
}

#[cfg(target_os = "android")]
fn run(app: slint::android::AndroidApp) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Slint Android backend.
    slint::android::init(app.clone()).map_err(|error| format!("Slint Android backend: {error}"))?;

    // 2 + 3. State + UI glue. A fresh `UiModel` is `Setup::Unconfigured`,
    // `Idle`, and `StorageState::Unconfigured` — the same properties
    // the host smoke test asserts.
    let model = Rc::new(RefCell::new(UiModel::new()));

    // Storage handles: the SAF provider is what launches the picker.
    // The handles are `Clone` (the inner `AndroidApp` is an `Arc`), so
    // sharing them with the Slint callback closures is cheap.
    let handles = SafHandles::new(app.clone());
    let provider = handles.provider();

    let index_dir = app
        .internal_data_path()
        .ok_or("Android provides no internal data directory")?
        .join("notae");
    std::fs::create_dir_all(&index_dir)?;
    let index_path = index_dir.join("index.sqlite");
    let thumbnails = ThumbnailCache::new(index_dir.join("thumbnails"))
        .map_err(|error| format!("thumbnail directory: {error}"))?;
    model.borrow_mut().set_thumbnail_cache(thumbnails.clone());
    model
        .borrow_mut()
        .set_recent_path(index_dir.join("recent.txt"));
    model
        .borrow_mut()
        .set_favorites_path(index_dir.join("favorites.json"));
    // Before the UI preferences, which reopen a saved set only if it still exists.
    model
        .borrow_mut()
        .set_sets_path(index_dir.join("sets.json"));
    model
        .borrow_mut()
        .set_ui_prefs_path(index_dir.join("ui-prefs.txt"));
    let operations = Rc::new(RefCell::new(None::<OperationController>));

    // Sanity-check the contract on a fresh install. If the smoke test
    // and the device ever diverge, this is the place that catches it
    // before the user sees a broken screen.
    {
        let snapshot = model.borrow();
        debug_assert_eq!(
            snapshot.state().screen(),
            &Screen::Setup(Setup::Unconfigured),
            "fresh UiModel must boot into Setup::Unconfigured",
        );
        debug_assert_eq!(
            snapshot.state().operation(),
            &OperationState::Idle,
            "fresh UiModel must boot with no background work in flight",
        );
        debug_assert!(
            matches!(
                snapshot.storage_kind(),
                notae_app_core::ui::StorageStateKindView::Unconfigured
            ),
            "fresh UiModel must boot with StorageState::Unconfigured",
        );
    }

    // 4. Slint window. The `app.slint` UI is compiled into `$OUT_DIR`
    // and pulled in via `lib.rs`. Property setters and callback wiring
    // all share the same `model` so every state change goes through
    // `UiModel::dispatch`.
    let hardware_keyboard_present = crate::android::search_input::read_hardware_keyboard(&app);
    let voice_available = crate::android::search_input::voice_recognizer_available(&app);
    let animations_enabled = crate::android::motion::read_animations_enabled(&app);
    let window = AppWindow::new().map_err(|error| format!("Slint window: {error}"))?;
    crate::android::search_input::bind_window(window.as_weak());
    crate::android::motion::bind_window(window.as_weak());
    crate::android::accessibility::bind(app.clone(), window.as_weak());
    window.set_hardware_keyboard_present(hardware_keyboard_present);
    window.set_animations_enabled(animations_enabled);
    window.set_voice_search_available(voice_available);
    sync_window(&window, &model.borrow());

    // Language: Android's per-app setting when it has one, Notae's preferences otherwise.
    // Translations can only be selected once the window exists.
    crate::android::language::declare_languages(&app);
    {
        let app = app.clone();
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        let apply = move || {
            let choice = crate::android::language::read_choice(&app)
                .unwrap_or(model.borrow().ui_prefs().language);
            crate::android::language::select(&app, choice);
            if let Some(window) = window_weak.upgrade() {
                window.set_language_choice(choice.index());
                sync_window(&window, &model.borrow());
            }
        };
        apply();
        crate::android::language::on_change(apply);
    }
    {
        let app = app.clone();
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_language_chosen(move |index| {
            let choice = notae_app_core::ui_prefs::LanguageChoice::from_index(index);
            if !crate::android::language::write_choice(&app, choice)
                && let Err(error) = model.borrow_mut().set_language(choice)
            {
                eprintln!("notae: cannot save UI preferences: {error}");
            }
            // Android also reports the change; selecting now keeps the switch instant.
            crate::android::language::select(&app, choice);
            if let Some(window) = window_weak.upgrade() {
                window.set_language_choice(choice.index());
                sync_window(&window, &model.borrow());
            }
        });
    }

    // The "Select folder" / "Reauthorize folder" button on the setup
    // and recovery screens fires the same callback. The handler reads
    // the storage state to decide which action to take — on setup it
    // launches the picker for the first time, on recovery it
    // re-launches the picker with the same handle so the user can
    // re-grant read permission without re-picking the folder.
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        let provider = provider.clone();
        let operations = Rc::clone(&operations);
        let index_path = index_path.clone();
        let thumbnails = thumbnails.clone();
        window.on_setup_chosen(move || {
            handle_picker_response(
                &model,
                &window_weak,
                &provider,
                &operations,
                &index_path,
                &thumbnails,
            );
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_rescan_clicked(move || {
            let transition = model.borrow_mut().start_rescan();
            match transition {
                Ok(Transition::BeginRescan) => {
                    let result = operations
                        .borrow()
                        .as_ref()
                        .ok_or(notae_app_core::ui::search::UiError::WorkerStopped)
                        .and_then(OperationController::start_rescan);
                    if let Err(error) = result {
                        let _ = model.borrow_mut().apply_operation_event(
                            notae_app_core::operations::events::OperationEvent::Failed {
                                reason: error.to_string(),
                            },
                        );
                    }
                }
                outcome => report_transition(outcome),
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_cancel_rescan_clicked(move || {
            if let Some(controller) = operations.borrow().as_ref()
                && let Err(error) = controller.cancel()
            {
                eprintln!("notae: rescan cancellation rejected: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_acknowledge_clicked(move || {
            report_transition(model.borrow_mut().dispatch(UiEvent::AcknowledgeOperation));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_section_selected(move |key| {
            if let Err(error) = model.borrow_mut().select_section(
                notae_app_core::ui::browse::LibrarySection::from_key(key.as_str()),
            ) {
                eprintln!("notae: cannot save UI preferences: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_search_changed(move |text| {
            let query = text.to_string();
            report_transition(model.borrow_mut().dispatch(UiEvent::UpdateSearchQuery {
                query: query.clone(),
            }));
            let scope = model.borrow().state().search().scope;
            if let Some(controller) = operations.borrow().as_ref()
                && let Err(error) = controller.set_search(query, scope)
            {
                eprintln!("notae: search failed: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        let app = app.clone();
        window.on_search_opened(move || {
            if let Err(error) = model.borrow_mut().dispatch(UiEvent::OpenSearch) {
                eprintln!("notae: cannot open search: {error}");
            }
            let keyboard_present = crate::android::search_input::read_hardware_keyboard(&app);
            if let Some(window) = window_weak.upgrade() {
                window.set_hardware_keyboard_present(keyboard_present);
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let app = app.clone();
        let window_weak = window.as_weak();
        window.on_voice_search_requested(move || {
            match crate::android::search_input::launch_voice_recognition(&app) {
                Ok(true) => {}
                Ok(false) => {
                    if let Some(window) = window_weak.upgrade() {
                        window.set_voice_search_available(false);
                    }
                }
                Err(error) => eprintln!("notae: cannot launch voice recognizer: {error}"),
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_search_scope_changed(move |key| {
            let scope = notae_index::SearchScope::from_key(key.as_str());
            let _ = model
                .borrow_mut()
                .dispatch(UiEvent::UpdateSearchScope { scope });
            let query = model.borrow().state().search().query.clone();
            if let Some(controller) = operations.borrow().as_ref()
                && let Err(error) = controller.set_search(query, scope)
            {
                eprintln!("notae: search failed: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_search_closed(move || {
            close_search(&model, &operations, None);
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_search_result_chosen(move |key| {
            close_search(&model, &operations, Some(key.to_string()));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_edit_metadata_requested(move |note_key| {
            let note_key = note_key.to_string();
            model.borrow_mut().begin_metadata_edit(note_key.clone());
            let result = operations
                .borrow()
                .as_ref()
                .ok_or(notae_app_core::ui::search::UiError::WorkerStopped)
                .and_then(|controller| controller.load_metadata(note_key));
            if let Err(error) = result {
                let _ = model.borrow_mut().apply_operation_event(
                    notae_app_core::operations::events::OperationEvent::MetadataFailed {
                        reason: error.to_string(),
                    },
                );
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_metadata_save_requested(move |note_key, id, title, composer, tags| {
            let metadata = notae_app_core::ui::metadata_editor::EditableMetadata {
                note_key: note_key.to_string(),
                id: id.to_string(),
                title: title.to_string(),
                composer_or_artist: composer.to_string(),
                tags: tags
                    .as_str()
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_owned)
                    .collect(),
            };
            let result = operations
                .borrow()
                .as_ref()
                .ok_or(notae_app_core::ui::search::UiError::WorkerStopped)
                .and_then(|controller| controller.save_metadata(metadata));
            if let Err(error) = result {
                let _ = model.borrow_mut().apply_operation_event(
                    notae_app_core::operations::events::OperationEvent::MetadataFailed {
                        reason: error.to_string(),
                    },
                );
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_metadata_cancelled(move || {
            model.borrow_mut().close_metadata_editor();
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        window.on_library_view_changed(move |view| {
            let view = if view.as_str() == "list" {
                notae_app_core::ui_prefs::LibraryView::List
            } else {
                notae_app_core::ui_prefs::LibraryView::Grid
            };
            if let Err(error) = model.borrow_mut().set_library_view(view) {
                eprintln!("notae: cannot save UI preferences: {error}");
            }
        });
    }
    {
        let model = Rc::clone(&model);
        window.on_sidebar_collapsed_changed(move |portrait, collapsed| {
            if let Err(error) = model
                .borrow_mut()
                .set_sidebar_collapsed(portrait, collapsed)
            {
                eprintln!("notae: cannot save UI preferences: {error}");
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_score_selected(move |key| {
            model.borrow_mut().select_note(Some(key.to_string()));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    wire_sets(&window, &model, &operations);
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_favorite_toggled(move |key| {
            if let Err(error) = model.borrow_mut().toggle_favorite(&key) {
                eprintln!("notae: cannot save favorites: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_selection_cleared(move || {
            model.borrow_mut().select_note(None);
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        let app = app.clone();
        // The set item id (empty outside a set) is not used yet.
        window.on_open_pdf_requested(move |relative_path, note_key, _item_id| {
            let relative_path = relative_path.to_string();
            let result = model
                .borrow()
                .library_root_handle()
                .ok_or_else(|| {
                    notae_app_core::pdf_intent::ViewerError::Platform(
                        crate::language::pick(
                            "No active library folder is available.",
                            "Brak aktywnego folderu biblioteki.",
                        )
                        .to_owned(),
                    )
                })
                .and_then(|tree_uri| {
                    crate::android::saf::document_content_uri(
                        app.clone(),
                        &tree_uri,
                        &relative_path,
                    )
                })
                .and_then(|uri| {
                    crate::android::pdf_intent::PdfLauncher::new(app.clone()).open(uri)
                });
            let opened = result.is_ok();
            model
                .borrow_mut()
                .set_pdf_launch_error(result.err().map(|error| error.to_string()));
            if opened && let Err(error) = model.borrow_mut().record_opened(note_key.as_str()) {
                eprintln!("notae: cannot save recent scores: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_settings_clicked(move || {
            report_transition(model.borrow_mut().dispatch(UiEvent::OpenSettings));
            if let Some(window) = window_weak.upgrade() {
                window.set_settings_backup_status(slint::SharedString::new());
                sync_window(&window, &model.borrow());
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        let app = app.clone();
        window.on_export_sets_requested(move || {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            window.set_settings_backup_status(slint::SharedString::new());
            let (file_name, text) = model.borrow().export_sets(notae_app_core::ui::unix_now());
            if let Err(error) =
                crate::android::backup::start_export(&app, window_weak.clone(), &file_name, text)
            {
                window.set_settings_backup_status(
                    crate::android::backup::export_failed_text(&error.to_string()).into(),
                );
            }
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_settings_closed(move || {
            report_transition(model.borrow_mut().dispatch(UiEvent::CloseSettings));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }

    let operation_timer = slint::Timer::default();
    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        operation_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let mut changed = false;
                if let Some(controller) = operations.borrow().as_ref() {
                    while let Some(event) = controller.try_event() {
                        if let Err(error) = model.borrow_mut().apply_operation_event(event) {
                            eprintln!("notae: rejected worker event: {error}");
                        }
                        changed = true;
                    }
                }
                changed |= model
                    .borrow_mut()
                    .expire_finished_result(std::time::Instant::now());
                if changed && let Some(window) = window_weak.upgrade() {
                    sync_window(&window, &model.borrow());
                }
            },
        );
    }

    // Keep the initial frame neutral while a persisted folder and its first rows are restored.
    if let Some(root) = provider.persisted_root() {
        apply_picker_result(
            &model,
            &window.as_weak(),
            &provider,
            &operations,
            &index_path,
            &thumbnails,
            Ok(root),
        );
    } else {
        model.borrow_mut().finish_startup_without_library();
        sync_window(&window, &model.borrow());
    }

    // 5. UI loop. Returns `Ok(())` when the activity is destroyed by the
    // OS — not an error path.
    window
        .run()
        .map_err(|error| format!("Slint UI loop: {error}"))?;
    Ok(())
}

thread_local! {
    /// Decoded thumbnails and the last rows pushed to the window. `sync_window` runs only on the
    /// UI thread, so a thread-local avoids threading this through every caller.
    static LIBRARY_ROWS: RefCell<LibraryRowsCache> = RefCell::default();
}

type PushedRows = (Vec<SearchRow>, Vec<bool>, bool, Vec<PieceRef>);

/// Keeps `sync_window` from re-decoding thumbnail PNGs and from replacing an unchanged
/// `library_rows` model (which would rebuild every card and drop keyboard focus).
#[derive(Default)]
struct LibraryRowsCache {
    /// Decoded thumbnails by content key. A key's thumbnail is written once and never
    /// rewritten, so entries only need dropping when the key leaves the library.
    images: HashMap<String, slint::Image>,
    /// The rows, per-row thumbnail availability, language and favorites last pushed to the
    /// window.
    pushed: Option<PushedRows>,
}

impl LibraryRowsCache {
    fn push(&mut self, window: &AppWindow, model: &UiModel, visible: Vec<SearchRow>) {
        let keys: HashSet<&str> = model
            .search_rows()
            .iter()
            .map(|row| row.note_key.as_str())
            .collect();
        self.images.retain(|key, _| keys.contains(key.as_str()));
        let thumbnails: Vec<Option<slint::Image>> = visible
            .iter()
            // A missing piece has no PDF, so a thumbnail left from its old content is stale.
            .map(|row| {
                if row.missing {
                    None
                } else {
                    self.thumbnail(model.thumbnail_cache(), &row.note_key)
                }
            })
            .collect();
        let available: Vec<bool> = thumbnails.iter().map(Option::is_some).collect();
        // Row labels are translated in Rust, so a language switch must re-push them.
        let polish = crate::language::pick(false, true);
        let favorites = model.favorites();
        if self
            .pushed
            .as_ref()
            .is_some_and(|(rows, flags, language, starred)| {
                *rows == visible
                    && *flags == available
                    && *language == polish
                    && starred.as_slice() == favorites
            })
        {
            return;
        }
        let rows: Vec<crate::LibraryRow> = visible
            .iter()
            .cloned()
            .zip(thumbnails)
            .map(|(row, thumbnail)| {
                let favorite = model.is_favorite(&row.note_key);
                crate::rows::library_row(row, thumbnail, favorite)
            })
            .collect();
        window.set_library_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        self.pushed = Some((visible, available, polish, favorites.to_vec()));
    }

    /// The decoded thumbnail for `note_key`, loading it once the file exists. A file that fails
    /// to decode is deleted so the next rescan renders it again; meanwhile the row shows the
    /// placeholder.
    fn thumbnail(
        &mut self,
        files: Option<&ThumbnailCache>,
        note_key: &str,
    ) -> Option<slint::Image> {
        if let Some(image) = self.images.get(note_key) {
            return Some(image.clone());
        }
        let files = files?;
        let Ok(image) = slint::Image::load_from_path(&files.existing(note_key)?) else {
            files.discard(note_key);
            return None;
        };
        self.images.insert(note_key.to_owned(), image.clone());
        Some(image)
    }
}

fn close_search(
    model: &Rc<RefCell<UiModel>>,
    operations: &Rc<RefCell<Option<OperationController>>>,
    selected: Option<String>,
) {
    let scope = model.borrow().state().search().scope;
    if let Err(error) = model.borrow_mut().dispatch(UiEvent::CloseSearch) {
        eprintln!("notae: cannot close search: {error}");
    }
    model.borrow_mut().select_note(selected);
    if let Some(controller) = operations.borrow().as_ref()
        && let Err(error) = controller.set_search(String::new(), scope)
    {
        eprintln!("notae: library refresh failed: {error}");
    }
}

/// The Sets section's callbacks: open a set, the new/edit set dialog, the delete confirmation,
/// the "Add to set…" checklist, removing an item and the add picker.
fn wire_sets(
    window: &AppWindow,
    model: &Rc<RefCell<UiModel>>,
    operations: &Rc<RefCell<Option<OperationController>>>,
) {
    use notae_app_core::sets::SetFields;
    use notae_app_core::ui::browse::LibrarySection;

    /// A callback that runs `change` on the model, then redraws the window.
    fn redraw_after(
        window: &AppWindow,
        model: &Rc<RefCell<UiModel>>,
        change: impl Fn(&AppWindow, &mut UiModel) + 'static,
    ) -> impl FnMut() + 'static {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        move || {
            if let Some(window) = window_weak.upgrade() {
                change(&window, &mut model.borrow_mut());
                sync_window(&window, &model.borrow());
            }
        }
    }
    fn open_dialog(window: &AppWindow, id: &str, fields: SetFields) {
        window.set_set_dialog_id(id.into());
        window.set_set_dialog_name(fields.name.into());
        window.set_set_dialog_date(fields.date.unwrap_or_default().into());
        window.set_set_dialog_notes(fields.notes.unwrap_or_default().into());
        window.set_set_dialog_error("".into());
        window.set_show_set_dialog(true);
    }

    fn empty_fields() -> SetFields {
        SetFields {
            name: String::new(),
            date: None,
            notes: None,
        }
    }

    window.on_new_set_requested(redraw_after(window, model, |window, model| {
        // A plain New set never carries a piece left from the checklist.
        model.set_dialog_cancelled();
        open_dialog(window, "", empty_fields());
    }));
    window.on_set_dialog_cancelled(redraw_after(window, model, |window, model| {
        model.set_dialog_cancelled();
        window.set_show_set_dialog(false);
    }));
    window.on_add_to_set_new_set_requested(redraw_after(window, model, |window, model| {
        model.add_to_set_new_set_requested();
        open_dialog(window, "", empty_fields());
    }));
    window.on_add_to_set_closed(redraw_after(window, model, |_window, model| {
        model.add_to_set_closed();
    }));
    window.on_add_picker_opened(redraw_after(window, model, |window, model| {
        model.add_picker_opened();
        window.set_add_picker_query("".into());
    }));
    window.on_add_picker_closed(redraw_after(window, model, |_window, model| {
        model.add_picker_closed();
    }));
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_add_to_set_requested(move |key| {
            model.borrow_mut().add_to_set_requested(key.as_str());
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_set_membership_toggled(move |set_id, key| {
            model
                .borrow_mut()
                .set_membership_toggled(set_id.as_str(), key.as_str());
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_remove_set_item_requested(move |item_id| {
            model
                .borrow_mut()
                .remove_set_item_requested(item_id.as_str());
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(model);
        let operations = Rc::clone(operations);
        let window_weak = window.as_weak();
        // The global search's worker answers the picker too, with rows of its own.
        window.on_add_picker_search_changed(move |query| {
            model.borrow_mut().add_picker_search_changed(query.as_str());
            if !query.trim().is_empty()
                && let Some(controller) = operations.borrow().as_ref()
                && let Err(error) = controller.search_picker(query.to_string())
            {
                eprintln!("notae: picker search failed: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_add_picker_toggled(move |key| {
            model.borrow_mut().add_picker_toggled(key.as_str());
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    window.on_delete_set_confirmed(redraw_after(window, model, |_window, model| {
        model.confirm_delete_set();
    }));
    window.on_delete_set_cancelled(redraw_after(window, model, |_window, model| {
        model.cancel_delete_set();
    }));
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_set_opened(move |id| {
            if let Err(error) = model
                .borrow_mut()
                .select_section(LibrarySection::Set(id.to_string()))
            {
                eprintln!("notae: cannot save UI preferences: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_edit_set_requested(move |id| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            if let Some(set) = model.borrow().sets().get(id.as_str()) {
                let fields = SetFields {
                    name: set.name.clone(),
                    date: set.date.clone(),
                    notes: set.notes.clone(),
                };
                open_dialog(&window, &set.id, fields);
            }
        });
    }
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_delete_set_requested(move |id| {
            model.borrow_mut().request_delete_set(id.as_str());
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(model);
        let window_weak = window.as_weak();
        window.on_set_dialog_saved(move |id, name, date, notes| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let fields = SetFields {
                name: name.to_string(),
                date: Some(date.to_string()),
                notes: Some(notes.to_string()),
            };
            let result = if id.is_empty() {
                model.borrow_mut().create_set(fields).map(|_| ())
            } else {
                model.borrow_mut().update_set(id.as_str(), fields)
            };
            match result {
                Ok(()) => window.set_show_set_dialog(false),
                Err(error) => {
                    window.set_set_dialog_error(crate::texts::set_field_error(error).into())
                }
            }
            sync_window(&window, &model.borrow());
        });
    }
}

/// Updates `current` in place when only some rows changed, so a tapped checkbox or add button keeps
/// focus and the list keeps its scroll position; otherwise hands a new model to `replace`.
fn refresh_rows<T: Clone + PartialEq + 'static>(
    current: &slint::ModelRc<T>,
    rows: Vec<T>,
    replace: impl FnOnce(slint::ModelRc<T>),
) {
    use slint::Model;
    if current.row_count() != rows.len() {
        replace(slint::ModelRc::new(slint::VecModel::from(rows)));
        return;
    }
    for (index, row) in rows.into_iter().enumerate() {
        if current.row_data(index).as_ref() != Some(&row) {
            current.set_row_data(index, row);
        }
    }
}

/// Pushes the currently visible fields of `UiModel` into the Slint
/// window's properties.
///
/// The visible screens are derived from the storage state:
/// * `Unconfigured` → setup screen.
/// * `Ready(_)` → library screen.
/// * `PermissionLost(root)` → recovery screen, with the root's display
///   name in the body text.
/// * `Error(_)` → setup screen; the picker button is still the recovery.
///
/// `Screen::Setup(_)` from the controller is not consulted here because
/// the storage state is authoritative for the picker-driven flows.
fn sync_window(window: &AppWindow, model: &UiModel) {
    let state = model.state();
    let (show_setup, show_recovery, recovery_folder_name, setup_error, library_display_name) =
        match model.storage_kind() {
            notae_app_core::ui::StorageStateKindView::Unconfigured => {
                (true, false, String::new(), String::new(), String::new())
            }
            notae_app_core::ui::StorageStateKindView::Ready { display_name } => {
                (false, false, String::new(), String::new(), display_name)
            }
            notae_app_core::ui::StorageStateKindView::PermissionLost(name) => {
                (false, true, name, String::new(), String::new())
            }
            notae_app_core::ui::StorageStateKindView::Error(message) => {
                // The setup screen stays visible so the user can
                // re-tap "Select folder"; the error label is the body
                // text the user sees.
                (true, false, String::new(), message, String::new())
            }
        };
    window.set_library_display_name(library_display_name.into());
    window.set_show_setup(show_setup);
    window.set_show_recovery(show_recovery);
    window.set_recovery_folder_name(recovery_folder_name.into());
    window.set_setup_error_text(setup_error.into());
    window.set_startup_ready(
        model.startup_presentation() == notae_app_core::ui::StartupPresentation::Ready,
    );
    let status = model.status();
    let operation = &status.operation;
    window.set_rescan_enabled(model.rescan_enabled());
    let banner = notae_app_core::ui::OperationBanner::project(operation);
    window.set_cancel_enabled(banner.cancel_enabled);
    window.set_acknowledge_enabled(banner.acknowledge_enabled);
    // The banner reports the running operation or its outcome on every screen. The latest scan
    // result and the no-text count are standing facts and live in Settings only.
    let (scan_summary, scan_conflicts, scan_failures) = status.latest_rescan.map_or_else(
        || {
            (
                crate::language::pick("No scan this session", "Brak skanowania w tej sesji")
                    .to_owned(),
                String::new(),
                String::new(),
            )
        },
        |summary| {
            (
                crate::language::pick(
                    format!(
                        "{} added, {} updated, {} moved, {} deleted",
                        summary.added, summary.updated, summary.moved, summary.deleted,
                    ),
                    format!(
                        "dodano: {}, zaktualizowano: {}, przeniesiono: {}, usunięto: {}",
                        summary.added, summary.updated, summary.moved, summary.deleted,
                    ),
                ),
                summary.conflicts.to_string(),
                summary.failures.to_string(),
            )
        },
    );
    window.set_settings_scan_summary(scan_summary.into());
    window.set_settings_scan_conflicts(scan_conflicts.into());
    window.set_settings_scan_failures(scan_failures.into());
    window.set_settings_indexed_count(status.indexed_count.to_string().into());
    window.set_settings_no_text_count(status.no_text_count.to_string().into());
    window.set_app_version(env!("CARGO_PKG_VERSION").into());
    window.set_app_build_identity(env!("NOTAE_BUILD_ID").into());
    window.set_repository_url("https://github.com/eRgo35/notae".into());
    window.set_app_license("MIT License".into());
    window.set_operation_progress_text(
        banner
            .message
            .as_ref()
            .map_or_else(String::new, crate::texts::status_message)
            .into(),
    );
    window.set_operation_failed(banner.failed);
    window.set_operation_busy(banner.busy);
    // Slint reads a negative fraction as "total unknown" (indeterminate).
    window.set_operation_fraction(banner.fraction.unwrap_or(-1.0));
    let prefs = model.ui_prefs();
    window.set_library_list_view(prefs.library_view == notae_app_core::ui_prefs::LibraryView::List);
    window.set_sidebar_collapsed_portrait(prefs.sidebar_collapsed_portrait);
    window.set_sidebar_collapsed_landscape(prefs.sidebar_collapsed_landscape);
    window.set_search_text(state.search().query.as_str().into());
    window.set_search_scope(state.search().scope.key().into());
    window.set_show_settings(matches!(state.screen(), Screen::Settings));
    let visible = model.visible_rows();
    let row_count = visible.len();
    // The open set, unless Search covers it. Its rows are its pieces in set order.
    let open_set = match model.section() {
        notae_app_core::ui::browse::LibrarySection::Set(id)
            if !matches!(state.screen(), Screen::Search) =>
        {
            model.sets().get(id)
        }
        _ => None,
    };
    let selected = model.selected_row();
    window.set_details_open(selected.is_some());
    let preview_neighbours = model.preview_neighbours();
    window.set_selected_previous_key(
        preview_neighbours
            .as_ref()
            .and_then(|nav| nav.previous.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_selected_next_key(
        preview_neighbours
            .as_ref()
            .and_then(|nav| nav.next.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_selected_position(
        preview_neighbours
            .map_or_else(String::new, |nav| {
                crate::texts::position_of(nav.position_of())
            })
            .into(),
    );
    if let Some(row) = selected {
        let thumbnail = LIBRARY_ROWS
            .with_borrow_mut(|cache| cache.thumbnail(model.thumbnail_cache(), &row.note_key));
        let favorite = model.is_favorite(&row.note_key);
        window.set_selected(crate::rows::library_row(row, thumbnail, favorite));
    }
    LIBRARY_ROWS.with_borrow_mut(|cache| cache.push(window, model, visible));

    let all_rows = model.search_rows();
    let sections: Vec<crate::SectionItem> =
        notae_app_core::ui::browse::sections(all_rows, &model.section_context())
            .into_iter()
            .map(|view| crate::SectionItem {
                key: view.key.into(),
                label: crate::texts::section_title(&view.label).into(),
                count: i32::try_from(view.count).unwrap_or(i32::MAX),
                kind: view.kind.into(),
            })
            .collect();
    window.set_sections(slint::ModelRc::new(slint::VecModel::from(sections)));
    let composers: Vec<crate::SectionItem> = notae_app_core::ui::browse::composers(all_rows)
        .into_iter()
        .map(|composer| crate::SectionItem {
            key: notae_app_core::ui::browse::LibrarySection::Composer(composer.name.clone())
                .key()
                .into(),
            label: composer.name.into(),
            count: i32::try_from(composer.count).unwrap_or(i32::MAX),
            kind: "".into(),
        })
        .collect();
    window.set_composers(slint::ModelRc::new(slint::VecModel::from(composers)));
    window.set_selected_section(model.section().key().into());
    window.set_library_title(crate::texts::section_title(&model.section_title()).into());
    let set_cards =
        notae_app_core::ui::browse::set_cards(model.sets(), notae_app_core::ui::unix_now());
    let subtitle = if model.section() == &notae_app_core::ui::browse::LibrarySection::Sets {
        crate::texts::sets_subtitle(set_cards.len())
    } else if let Some(set) = open_set {
        crate::texts::set_subtitle(&notae_app_core::ui::browse::set_subtitle(set))
    } else {
        crate::language::pick(
            format!(
                "{row_count} {}",
                if row_count == 1 { "score" } else { "scores" }
            ),
            format!(
                "{row_count} {}",
                crate::language::polish_plural(row_count, "utwór", "utwory", "utworów")
            ),
        )
    };
    window.set_library_subtitle(subtitle.into());
    window.set_library_notes(
        open_set
            .and_then(|set| set.notes.as_deref())
            .unwrap_or_default()
            .into(),
    );
    let set_cards: Vec<crate::SetCard> = set_cards.into_iter().map(Into::into).collect();
    window.set_set_cards(slint::ModelRc::new(slint::VecModel::from(set_cards)));
    window.set_confirm_delete_set_name(model.pending_delete_set_name().unwrap_or_default().into());
    window.set_show_add_to_set(model.add_to_set_key().is_some());
    window.set_add_to_set_key(model.add_to_set_key().unwrap_or_default().into());
    window.set_add_to_set_title(model.add_to_set_title().into());
    refresh_rows(
        &window.get_add_to_set_choices(),
        model
            .add_to_set_choices()
            .into_iter()
            .map(Into::into)
            .collect(),
        |rows| {
            window.set_add_to_set_choices(rows);
        },
    );
    window.set_show_add_picker(model.add_picker_open());
    window.set_add_picker_footer(
        crate::texts::add_picker_footer(model.add_picker_added_count()).into(),
    );
    refresh_rows(
        &window.get_add_picker_rows(),
        model
            .add_picker_rows()
            .into_iter()
            .map(Into::into)
            .collect(),
        |rows| {
            window.set_add_picker_rows(rows);
        },
    );
    window.set_show_search(matches!(state.screen(), Screen::Search));
    let metadata = model.metadata_editor();
    window.set_show_metadata_editor(metadata.is_some());
    window.set_metadata_note_key(
        metadata
            .map_or("", |metadata| metadata.note_key.as_str())
            .into(),
    );
    window.set_metadata_id(metadata.map_or("", |metadata| metadata.id.as_str()).into());
    window.set_metadata_title(
        metadata
            .map_or("", |metadata| metadata.title.as_str())
            .into(),
    );
    window.set_metadata_composer_or_artist(
        metadata
            .map_or("", |metadata| metadata.composer_or_artist.as_str())
            .into(),
    );
    window.set_metadata_tags(
        metadata
            .map_or_else(String::new, |metadata| metadata.tags.join(", "))
            .into(),
    );
    window.set_metadata_error_text(model.metadata_error().unwrap_or_default().into());
    window.set_pdf_launch_error_text(model.pdf_launch_error().unwrap_or_default().into());
}

/// Logs the outcome of a state transition at the level the platform
/// allows.
fn report_transition(outcome: Result<Transition, EventError>) {
    match outcome {
        Ok(Transition::NoOp) => {}
        Ok(Transition::BeginRescan) => {}
        Err(EventError::OperationAlreadyInProgress) => {
            // The UI is expected to disable the controls that produce
            // this branch, so seeing the error means the wiring has
            // drifted. Surface it on `adb logcat` (stderr on the host).
            eprintln!("notae: rejected overlapping operation");
        }
        Err(EventError::SettingsUnavailable) => {
            eprintln!("notae: settings are unavailable until a library is open");
        }
        Err(EventError::SearchUnavailable) => {
            eprintln!("notae: search is unavailable from the current screen");
        }
    }
}

/// Drives the picker and applies the result to the storage state.
///
/// The flow on both screens is the same:
/// 1. The user taps the primary button.
/// 2. We launch `ACTION_OPEN_DOCUMENT_TREE` via the SAF provider.
/// 3. The Activity-result callback dispatches the outcome and wakes the
///    pending selection future.
/// 4. Slint resumes the task, which reopens the persisted root and
///    updates the storage state.
///
/// This callback must return to the event loop while the picker is open:
/// Android delivers `onActivityResult` on the same main thread.
fn handle_picker_response(
    model: &Rc<RefCell<UiModel>>,
    window_weak: &slint::Weak<AppWindow>,
    provider: &SafLibraryRootProvider,
    operations: &Rc<RefCell<Option<OperationController>>>,
    index_path: &Path,
    thumbnails: &ThumbnailCache,
) {
    let provider_task = provider.clone();
    let model_task = Rc::clone(model);
    let operations_task = Rc::clone(operations);
    let index_path_task = index_path.to_path_buf();
    let thumbnails_task = thumbnails.clone();
    let window_task = window_weak.clone();
    if let Err(error) = slint::spawn_local(async move {
        let result = provider_task.select_root().await;
        apply_picker_result(
            &model_task,
            &window_task,
            &provider_task,
            &operations_task,
            &index_path_task,
            &thumbnails_task,
            result,
        );
    }) {
        model
            .borrow_mut()
            .set_storage(StorageState::Error(StorageError::Other(format!(
                "could not schedule folder picker: {error}"
            ))));
        if let Some(window) = window_weak.upgrade() {
            sync_window(&window, &model.borrow());
        }
    }
}

fn apply_picker_result(
    model: &Rc<RefCell<UiModel>>,
    window_weak: &slint::Weak<AppWindow>,
    provider: &SafLibraryRootProvider,
    operations: &Rc<RefCell<Option<OperationController>>>,
    index_path: &Path,
    thumbnails: &ThumbnailCache,
    result: Result<notae_app_core::storage::LibraryRoot, StorageError>,
) {
    let mut initial_query_error = None;
    let next_state = match result {
        Ok(root) => match provider.reopen(&root) {
            Ok(source) => match IndexStore::open(index_path) {
                Ok(local) => {
                    let source = Arc::new(Mutex::new(source));
                    match OperationController::with_thumbnails(
                        local,
                        Box::new(SharedLibraryCatalog(Arc::clone(&source))),
                        thumbnails.clone(),
                    ) {
                        Ok(controller) => {
                            *operations.borrow_mut() = Some(controller);
                            let query = model.borrow().state().search().query.clone();
                            if let Some(controller) = operations.borrow().as_ref()
                                && let Err(error) = controller.set_query(query)
                            {
                                initial_query_error = Some(error.to_string());
                            }
                            StorageState::Ready(source)
                        }
                        Err(error) => {
                            *operations.borrow_mut() = None;
                            StorageState::Error(StorageError::Other(format!(
                                "cannot start index worker: {error}"
                            )))
                        }
                    }
                }
                Err(error) => StorageState::Error(StorageError::Other(format!(
                    "cannot open local index: {error}"
                ))),
            },
            Err(StorageError::PermissionLost) => {
                *operations.borrow_mut() = None;
                StorageState::PermissionLost(root)
            }
            Err(error) => {
                *operations.borrow_mut() = None;
                StorageState::Error(error)
            }
        },
        Err(StorageError::Cancelled) => return,
        Err(error) => {
            eprintln!("notae: SAF picker failed: {error}");
            return;
        }
    };
    model.borrow_mut().set_storage(next_state);
    if let Some(reason) = initial_query_error {
        let _ = model.borrow_mut().apply_operation_event(
            notae_app_core::operations::events::OperationEvent::Failed { reason },
        );
    }
    if let Some(window) = window_weak.upgrade() {
        sync_window(&window, &model.borrow());
    }
}
