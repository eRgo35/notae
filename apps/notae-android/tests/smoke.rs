//! Host-side smoke test for the Notae Android shell.
//!
//! These tests run on the developer's Linux box and stand in for the
//! device smoke in step 5 of the task brief: a Galaxy Tab S10 FE+ isn't
//! attached to this machine. If `cargo test -p notae-android --test smoke`
//! passes, the *state machine* the device will run is the same state
//! machine Linux just exercised.

use notae_android::state::{
    AppController, AppState, EventError, OperationState, Screen, Setup, Transition, UiEvent,
};

#[test]
fn fresh_app_exposes_setup_unconfigured_and_idle() {
    // The brief: "Assert that a fresh app exposes the setup state and
    // does not start a rescan automatically."
    let state = AppState::new();
    assert_eq!(state.screen(), &Screen::Setup(Setup::Unconfigured));
    assert_eq!(state.operation(), &OperationState::Idle);
    // The library is empty and the search box starts blank.
    assert!(state.library().rows.is_empty());
    assert!(state.search().query.is_empty());
}

#[test]
fn fresh_controller_has_no_background_work_in_flight() {
    // `AppController::new` does nothing beyond constructing the initial
    // `AppState`. No threads, no timers, no spawn.
    let controller = AppController::new();
    assert_eq!(controller.state().screen(), &Screen::Setup(Setup::Unconfigured));
    assert_eq!(controller.state().operation(), &OperationState::Idle);
}

#[test]
fn state_machine_exposes_a_way_to_begin_a_rescan() {
    // The brief also asks for `OperationController::start_rescan` (or
    // equivalent) to be available even though nothing in Task 1 fires
    // it. The equivalent here is `UiModel::start_rescan`, exposed via
    // `AppController::dispatch(UiEvent::RequestRescan)`. Verify it's
    // reachable and that it produces the right transition.
    let mut controller = AppController::new();
    // The setup screen blocks a rescan (no library chosen), so this
    // just confirms the API is wired without asserting on the screen
    // transition — the production flow drives the screen transition
    // first via `PickLibraryRoot` and *then* requests the rescan.
    controller.dispatch(UiEvent::PickLibraryRoot {
        root: std::path::PathBuf::from("/tmp/example"),
    }).expect("picking a root moves the app off the setup screen");
    assert_eq!(controller.state().screen(), &Screen::Library);

    let transition = controller.dispatch(UiEvent::RequestRescan)
        .expect("the first rescan request goes through");
    assert_eq!(transition, Transition::BeginRescan);
    assert_eq!(
        controller.state().operation(),
        &OperationState::RescanInProgress
    );
}

#[test]
fn overlapping_rescan_requests_are_rejected() {
    let mut controller = AppController::new();
    controller.dispatch(UiEvent::PickLibraryRoot {
        root: std::path::PathBuf::from("/tmp/example"),
    }).unwrap();
    controller.dispatch(UiEvent::RequestRescan).unwrap();
    // A second rescan request while one is in progress is a wiring bug
    // at the UI layer. The state machine refuses and reports the
    // reason explicitly so the caller can disable the button.
    assert_eq!(
        controller.dispatch(UiEvent::RequestRescan),
        Err(EventError::OperationAlreadyInProgress)
    );
}
