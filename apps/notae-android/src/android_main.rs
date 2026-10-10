//! Android entry point.
//!
//! `android_main` is the symbol the `android-activity` runtime looks up
//! when `libnotae-android.so` is loaded. It:
//!
//! 1. Initialises the Slint Android platform via `slint::android::init`.
//! 2. Builds an [`AppController`](crate::state::AppController) — a fresh
//!    one, so the device boots into `Setup::Unconfigured` / `Idle`.
//! 3. Constructs a [`UiModel`](crate::ui::UiModel) around it and an
//!    empty [`StorageState`](crate::storage::StorageState).
//! 4. Wires the Slint `AppWindow`'s properties (visible fields of the
//!    state) and callbacks (forwarded as `UiEvent`s).
//! 5. Drives the Storage Access Framework picker through the
//!    [`SafLibraryRootProvider`](crate::android::saf::SafLibraryRootProvider),
//!    and the resulting [`LibraryRoot`](crate::storage::LibraryRoot)
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
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint::ComponentHandle;

use crate::android::saf::{SafHandles, SafLibraryRootProvider};
use crate::operations::rescan::OperationController;
use crate::state::{EventError, OperationState, Screen, Setup, Transition, UiEvent};
use crate::storage::{LibraryRootProvider, SharedLibraryCatalog, StorageError, StorageState};
use crate::ui::UiModel;
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
    // 1. Slint Android backend.
    slint::android::init(app.clone()).expect("failed to initialise the Slint Android backend");

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
        .expect("Android provides an internal data directory")
        .join("notae");
    std::fs::create_dir_all(&index_dir).expect("create private Notae index directory");
    let index_path = index_dir.join("index.sqlite");
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
                crate::ui::StorageStateKindView::Unconfigured
            ),
            "fresh UiModel must boot with StorageState::Unconfigured",
        );
    }

    // 4. Slint window. The `app.slint` UI is compiled into `$OUT_DIR`
    // and pulled in via `lib.rs`. Property setters and callback wiring
    // all share the same `model` so every state change goes through
    // `UiModel::dispatch`.
    let window = AppWindow::new().expect("failed to construct Slint AppWindow");

    sync_window(&window, &model.borrow());

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
        window.on_setup_chosen(move || {
            handle_picker_response(&model, &window_weak, &provider, &operations, &index_path);
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
                        .ok_or(crate::ui::search::UiError::WorkerStopped)
                        .and_then(OperationController::start_rescan);
                    if let Err(error) = result {
                        let _ = model.borrow_mut().apply_operation_event(
                            crate::operations::events::OperationEvent::Failed {
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
            if let Some(controller) = operations.borrow().as_ref() {
                if let Err(error) = controller.cancel() {
                    eprintln!("notae: rescan cancellation rejected: {error}");
                }
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
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_import_confirmed(move || {
            let transition = model.borrow_mut().dispatch(UiEvent::ConfirmImport);
            match transition {
                Ok(_) => {
                    let result = operations
                        .borrow()
                        .as_ref()
                        .ok_or(crate::ui::search::UiError::WorkerStopped)
                        .and_then(OperationController::confirm_import);
                    if let Err(error) = result {
                        let _ = model.borrow_mut().apply_operation_event(
                            crate::operations::events::OperationEvent::Failed {
                                reason: error.to_string(),
                            },
                        );
                    }
                }
                Err(error) => report_transition(Err(error)),
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
        window.on_import_cancelled(move || {
            report_transition(model.borrow_mut().dispatch(UiEvent::CancelImport));
            model.borrow_mut().clear_import_preview();
            if let Some(controller) = operations.borrow().as_ref() {
                if let Err(error) = controller.discard_import_preview() {
                    eprintln!("notae: cannot discard sidecar preview: {error}");
                }
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
            if let Some(controller) = operations.borrow().as_ref() {
                if let Err(error) = controller.set_query(query) {
                    eprintln!("notae: search failed: {error}");
                }
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
        window.on_edit_metadata_requested(move |note_key| {
            let note_key = note_key.to_string();
            model.borrow_mut().begin_metadata_edit(note_key.clone());
            let result = operations
                .borrow()
                .as_ref()
                .ok_or(crate::ui::search::UiError::WorkerStopped)
                .and_then(|controller| controller.load_metadata(note_key));
            if let Err(error) = result {
                let _ = model.borrow_mut().apply_operation_event(
                    crate::operations::events::OperationEvent::MetadataFailed {
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
            let metadata = crate::ui::metadata_editor::EditableMetadata {
                note_key: note_key.to_string(),
                id: id.to_string(),
                title: title.to_string(),
                composer_or_artist: composer.to_string(),
                tags: tags
                    .to_string()
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_owned)
                    .collect(),
            };
            let result = operations
                .borrow()
                .as_ref()
                .ok_or(crate::ui::search::UiError::WorkerStopped)
                .and_then(|controller| controller.save_metadata(metadata));
            if let Err(error) = result {
                let _ = model.borrow_mut().apply_operation_event(
                    crate::operations::events::OperationEvent::MetadataFailed {
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
        let window_weak = window.as_weak();
        let app = app.clone();
        window.on_open_pdf_requested(move |relative_path| {
            let relative_path = relative_path.to_string();
            let result = model
                .borrow()
                .library_root_handle()
                .ok_or_else(|| {
                    crate::pdf_intent::ViewerError::Platform(
                        "No active library folder is available.".to_owned(),
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
            model
                .borrow_mut()
                .set_pdf_launch_error(result.err().map(|error| error.to_string()));
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
                sync_window(&window, &model.borrow());
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
                if changed {
                    if let Some(window) = window_weak.upgrade() {
                        sync_window(&window, &model.borrow());
                    }
                }
            },
        );
    }

    // 5. UI loop. Returns `Ok(())` when the activity is destroyed by the
    // OS — not an error path. `.expect` matches the Slint template.
    window.run().expect("Slint UI loop exited with an error");
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
    let (show_setup, show_recovery, recovery_folder_name, setup_error) = match model.storage_kind()
    {
        crate::ui::StorageStateKindView::Unconfigured => {
            (true, false, String::new(), String::new())
        }
        crate::ui::StorageStateKindView::Ready => (false, false, String::new(), String::new()),
        crate::ui::StorageStateKindView::PermissionLost(name) => (false, true, name, String::new()),
        crate::ui::StorageStateKindView::Error(message) => {
            // The setup screen stays visible so the user can
            // re-tap "Select folder"; the error label is the body
            // text the user sees.
            (true, false, String::new(), message)
        }
    };
    window.set_show_setup(show_setup);
    window.set_show_recovery(show_recovery);
    window.set_recovery_folder_name(recovery_folder_name.into());
    window.set_setup_error_text(setup_error.into());
    let operation = state.operation();
    window.set_rescan_enabled(operation == &OperationState::Idle);
    window.set_cancel_enabled(matches!(operation, OperationState::RescanInProgress { .. }));
    window.set_acknowledge_enabled(matches!(
        operation,
        OperationState::Completed { .. } | OperationState::Failed { .. }
    ));
    window.set_operation_progress_text(operation_progress_text(operation).into());
    window.set_search_text(state.search().query.as_str().into());
    window.set_show_settings(matches!(state.screen(), Screen::Settings));
    let rows: Vec<crate::LibraryRow> = model
        .search_rows()
        .iter()
        .cloned()
        .map(crate::LibraryRow::from)
        .collect();
    window.set_library_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
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
    let preview = model.import_preview_summary();
    window.set_show_import_preview(preview.is_some());
    window.set_import_preview_text(preview.map_or_else(String::new, import_preview_text).into());
    let import_rows: Vec<crate::ImportRow> = preview
        .into_iter()
        .flat_map(|preview| preview.rows.iter())
        .map(|row| crate::ImportRow {
            path: row.path.as_str().into(),
            change: row.change.as_str().into(),
            local_metadata: row.local_metadata.as_deref().unwrap_or("—").into(),
            proposed_metadata: row.proposed_metadata.as_str().into(),
            fields: format!(
                "Imported: {}; preserved: {}; ",
                row.imported_fields.join(", "),
                row.preserved_fields.join(", ")
            )
            .into(),
            ocr: if row.ocr_available {
                "OCR available. ".into()
            } else {
                "No sidecar OCR. ".into()
            },
            conflict: row
                .conflict
                .as_deref()
                .map_or_else(String::new, |value| format!("{value}. "))
                .into(),
        })
        .collect();
    window.set_import_rows(slint::ModelRc::new(slint::VecModel::from(import_rows)));
}

fn operation_progress_text(operation: &OperationState) -> String {
    match operation {
        OperationState::Idle => String::new(),
        OperationState::RescanInProgress { completed, total } if *total == 0 => {
            "Scanning library…".to_owned()
        }
        OperationState::RescanInProgress { completed, total } => {
            format!("Scanning PDFs: {completed} of {total}")
        }
        OperationState::ImportPreviewPending { candidate_count } => {
            format!("Review {candidate_count} proposed sidecar changes")
        }
        OperationState::ImportApplying => "Applying sidecar import…".to_owned(),
        OperationState::Completed { summary } => summary.clone(),
        OperationState::Failed { reason } => reason.clone(),
    }
}

fn import_preview_text(summary: &crate::operations::events::ImportPreviewSummary) -> String {
    format!(
        "{} new; {} changed; {} unchanged; {} missing; {} hash mismatches; {} unreadable; {} ID conflicts; {} entries with OCR. Review proposed and preserved metadata below.",
        summary.new,
        summary.changed,
        summary.unchanged,
        summary.missing,
        summary.mismatches,
        summary.unreadable,
        summary.conflicts,
        summary.ocr_available,
    )
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
    index_path: &PathBuf,
) {
    let provider = provider.clone();
    let model = Rc::clone(model);
    let operations = Rc::clone(operations);
    let index_path = index_path.clone();
    let window_weak = window_weak.clone();
    slint::spawn_local(async move {
        let result = provider.select_root().await;
        apply_picker_result(
            &model,
            &window_weak,
            &provider,
            &operations,
            &index_path,
            result,
        );
    })
    .expect("failed to schedule SAF picker future on Slint event loop");
}

fn apply_picker_result(
    model: &Rc<RefCell<UiModel>>,
    window_weak: &slint::Weak<AppWindow>,
    provider: &SafLibraryRootProvider,
    operations: &Rc<RefCell<Option<OperationController>>>,
    index_path: &PathBuf,
    result: Result<crate::storage::LibraryRoot, StorageError>,
) {
    let next_state = match result {
        Ok(root) => match provider.reopen(&root) {
            Ok(source) => match IndexStore::open(index_path) {
                Ok(local) => {
                    let source = Arc::new(Mutex::new(source));
                    let controller = OperationController::new(
                        local,
                        Box::new(SharedLibraryCatalog(Arc::clone(&source))),
                    );
                    *operations.borrow_mut() = Some(controller);
                    let query = model.borrow().state().search().query.clone();
                    if let Some(controller) = operations.borrow().as_ref() {
                        if let Err(error) = controller.set_query(query) {
                            eprintln!("notae: initial library query failed: {error}");
                        }
                    }
                    StorageState::Ready(source)
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
    if let Some(window) = window_weak.upgrade() {
        sync_window(&window, &model.borrow());
    }
}
