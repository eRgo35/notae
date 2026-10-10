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
use std::rc::Rc;

use slint::ComponentHandle;

use crate::android::saf::{SafHandles, SafLibraryRootProvider};
use crate::state::{
    EventError, OperationState, Screen, Setup, Transition, UiEvent,
};
use crate::storage::{LibraryRootProvider, StorageError, StorageState};
use crate::ui::UiModel;

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
    slint::android::init(app.clone())
        .expect("failed to initialise the Slint Android backend");

    // 2 + 3. State + UI glue. A fresh `UiModel` is `Setup::Unconfigured`,
    // `Idle`, and `StorageState::Unconfigured` — the same properties
    // the host smoke test asserts.
    let model = Rc::new(RefCell::new(UiModel::new()));

    // Storage handles: the SAF provider is what launches the picker.
    // The handles are `Clone` (the inner `AndroidApp` is an `Arc`), so
    // sharing them with the Slint callback closures is cheap.
    let handles = SafHandles::new(app.clone());
    let provider = handles.provider();

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
            matches!(snapshot.storage_kind(), crate::ui::StorageStateKindView::Unconfigured),
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
        window.on_setup_chosen(move || {
            handle_picker_response(&model, &window_weak, &provider);
        });
    }

    {
        let model = Rc::clone(&model);
        window.on_rescan_clicked(move || {
            report_transition(model.borrow_mut().start_rescan());
        });
    }

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_search_changed(move |text| {
            report_transition(
                model
                    .borrow_mut()
                    .dispatch(UiEvent::UpdateSearchQuery { query: text.to_string() }),
            );
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
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
    let (show_setup, show_recovery, recovery_folder_name, setup_error) =
        match model.storage_kind() {
            crate::ui::StorageStateKindView::Unconfigured => {
                (true, false, String::new(), String::new())
            }
            crate::ui::StorageStateKindView::Ready => {
                (false, false, String::new(), String::new())
            }
            crate::ui::StorageStateKindView::PermissionLost(name) => {
                (false, true, name, String::new())
            }
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
    window.set_rescan_enabled(state.operation() == &OperationState::Idle);
    window.set_search_text(state.search().query.as_str().into());
}

/// Logs the outcome of a state transition at the level the platform
/// allows.
fn report_transition(outcome: Result<Transition, EventError>) {
    match outcome {
        Ok(Transition::NoOp) => {}
        Ok(Transition::BeginRescan) => {
            // The worker is wired up in Task 4. For now we just
            // observe the transition.
        }
        Err(EventError::OperationAlreadyInProgress) => {
            // The UI is expected to disable the controls that produce
            // this branch, so seeing the error means the wiring has
            // drifted. Surface it on `adb logcat` (stderr on the host).
            eprintln!("notae: rejected overlapping operation");
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
) {
    let provider = provider.clone();
    let model = Rc::clone(model);
    let window_weak = window_weak.clone();
    slint::spawn_local(async move {
        let result = provider.select_root().await;
        apply_picker_result(&model, &window_weak, &provider, result);
    })
    .expect("failed to schedule SAF picker future on Slint event loop");
}

fn apply_picker_result(
    model: &Rc<RefCell<UiModel>>,
    window_weak: &slint::Weak<AppWindow>,
    provider: &SafLibraryRootProvider,
    result: Result<crate::storage::LibraryRoot, StorageError>,
) {
    let next_state = match result {
        Ok(root) => match provider.reopen(&root) {
            Ok(source) => StorageState::Ready(source),
            Err(StorageError::PermissionLost) => StorageState::PermissionLost(root),
            Err(error) => StorageState::Error(error),
        },
        Err(StorageError::Cancelled) => return, // user dismissed, no change
        Err(error) => {
            // `select_root` only returns `Cancelled` (the user
            // dismissed) or a platform-level failure. `NotADirectory`
            // and `PermissionLost` come from `reopen` and never appear
            // here; the variants exist on `StorageError` for the host
            // tests' contract but the Android side never sees them.
            eprintln!("notae: SAF picker failed: {error}");
            return;
        }
    };
    model.borrow_mut().set_storage(next_state);
    if let Some(window) = window_weak.upgrade() {
        sync_window(&window, &model.borrow());
    }
}