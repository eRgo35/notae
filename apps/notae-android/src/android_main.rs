//! Android entry point.
//!
//! `android_main` is the symbol the `android-activity` runtime looks up
//! when `libnotae-android.so` is loaded. It:
//!
//! 1. Initialises the Slint Android platform via `slint::android::init`.
//! 2. Builds an [`AppController`](crate::state::AppController) — a fresh
//!    one, so the device boots into `Setup::Unconfigured` / `Idle`.
//! 3. Constructs a [`UiModel`](crate::ui::UiModel) around it.
//! 4. Wires the Slint `AppWindow`'s properties (visible fields of the
//!    state) and callbacks (forwarded as `UiEvent`s).
//! 5. Runs the Slint event loop until the OS asks us to stop.
//!
//! Task 1 only does the *plumbing*: the wiring is real, but every
//! callback either records the user action or applies an `Idle`-to-Idle
//! transition. Real work (rescan, OCR, import) lands in later tasks and
//! plugs into the same `UiModel::dispatch` / `AppController::dispatch`
//! surface.
//!
//! `#![allow(unsafe_code)]` here is intentional and scoped to this
//! file. The Android runtime requires `android_main` to be exported as
//! a C ABI symbol with a stable name; `#[unsafe(no_mangle)]` is the
//! mechanism Rust provides for that, and the runtime guarantees the
//! platform ABI rather than the language's. The rest of the crate
//! (`state.rs`, `ui.rs`) keeps a strict `#![forbid(unsafe_code)]`.
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

use crate::state::{
    AppController, EventError, OperationState, Screen, Setup, Transition, UiEvent,
};
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
    slint::android::init(app)
        .expect("failed to initialise the Slint Android backend");

    // 2 + 3. State + UI glue. A fresh `UiModel` is `Setup::Unconfigured`
    // and `Idle` — the same property the host smoke test asserts.
    //
    // The `Rc<RefCell<_>>` lets each Slint callback hold a shared
    // handle: the state model is `Send + Sync` already (its fields are
    // all `String`/`PathBuf`/numeric), but Slint's `move ||` closures
    // are single-threaded, so we use the standard interior-mutability
    // pattern instead of mutating through captures.
    let model = Rc::new(RefCell::new(UiModel::from_controller(
        AppController::new(),
    )));

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
    }

    // 4. Slint window. The `app.slint` UI is compiled into `$OUT_DIR`
    // and pulled in via `lib.rs`. Property setters and callback wiring
    // all share the same `model` so every state change goes through
    // `UiModel::dispatch`.
    let window = AppWindow::new().expect("failed to construct Slint AppWindow");

    sync_window(&window, &model.borrow());

    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_setup_chosen(move || {
            report_transition(model.borrow_mut().dispatch(
                UiEvent::PickLibraryRoot {
                    root: std::path::PathBuf::from("/sdcard/Pictures/Notae"),
                },
            ));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
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

/// Pushes the currently visible fields of `AppState` into the Slint
/// window's properties.
fn sync_window(window: &AppWindow, model: &UiModel) {
    let state = model.state();
    window.set_show_setup(state.screen() == &Screen::Setup(Setup::Unconfigured));
    window.set_rescan_enabled(state.operation() == &OperationState::Idle);
    window.set_search_text(state.search().query.as_str().into());
}

/// Logs the outcome of a state transition at the level the platform
/// allows: Task 1 has no worker thread yet, so `BeginRescan` is observed
/// but not actioned; `Err(_)` is logged because the UI should disable
/// the controls that produce it.
fn report_transition(outcome: Result<Transition, EventError>) {
    match outcome {
        Ok(Transition::NoOp) => {}
        Ok(Transition::BeginRescan) => {
            // The worker is wired up in Task 4. For Task 1 there is no
            // worker yet; we just observe the state transition.
        }
        Err(EventError::OperationAlreadyInProgress) => {
            // The UI is expected to disable the controls that produce
            // this branch, so seeing the error means the wiring has
            // drifted. Surface it on `adb logcat` (stderr on the host).
            eprintln!("notae: rejected overlapping operation");
        }
    }
}
