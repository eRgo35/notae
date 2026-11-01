//! Host-side smoke test for the Notae Android shell.
//!
//! These tests run on the developer's Linux box and stand in for the
//! device smoke in step 5 of the task brief: a Galaxy Tab S10 FE+ isn't
//! attached to this machine. If `cargo test -p notae-app-core --test smoke`
//! passes, the *state machine* the device will run is the same state
//! machine Linux just exercised.

use notae_app_core::state::{
    AppController, AppState, EventError, OperationState, RescanPhase, Screen, Setup, Transition,
    UiEvent,
};
use notae_app_core::storage::{StorageError, StorageState};

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
    assert_eq!(
        controller.state().screen(),
        &Screen::Setup(Setup::Unconfigured)
    );
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
    controller
        .dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/tmp/example"),
        })
        .expect("picking a root moves the app off the setup screen");
    assert_eq!(controller.state().screen(), &Screen::Library);

    let transition = controller
        .dispatch(UiEvent::RequestRescan)
        .expect("the first rescan request goes through");
    assert_eq!(transition, Transition::BeginRescan);
    assert_eq!(
        controller.state().operation(),
        &OperationState::Rescan {
            phase: RescanPhase::Enumerating,
        }
    );
}

#[test]
fn overlapping_rescan_requests_are_rejected() {
    let mut controller = AppController::new();
    controller
        .dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/tmp/example"),
        })
        .unwrap();
    controller.dispatch(UiEvent::RequestRescan).unwrap();
    // A second rescan request while one is in progress is a wiring bug
    // at the UI layer. The state machine refuses and reports the
    // reason explicitly so the caller can disable the button.
    assert_eq!(
        controller.dispatch(UiEvent::RequestRescan),
        Err(EventError::OperationAlreadyInProgress)
    );
}

#[test]
fn shared_scan_eligibility_tracks_idle_operation_state() {
    use notae_app_core::ui::UiModel;

    let mut model = UiModel::new();
    assert!(model.rescan_enabled());

    model
        .dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/tmp/example"),
        })
        .expect("choose library");
    assert!(model.rescan_enabled());

    model.start_rescan().expect("request scan");
    assert!(!model.rescan_enabled());

    model
        .controller_mut()
        .cancel_operation()
        .expect("cancel scan");
    assert!(model.rescan_enabled());
}

// P3 #7: `StorageState::Error(_)` carries a payload the UI can
// render. The display string is what `StorageStateKindView::Error(message)`
// hands to 0.3.0's `setup_error_label`.
#[test]
fn storage_state_error_carries_message_to_kind() {
    let state = StorageState::Error(StorageError::Other("Permission refused".to_owned()));
    match state.kind() {
        notae_app_core::storage::StorageStateKind::Error => {}
        other => panic!("expected Error kind, got {other:?}"),
    }
    // The display string the UI uses is the underlying `Display`
    // impl, which is what the user sees.
    let message = match state {
        StorageState::Error(error) => error.to_string(),
        _ => unreachable!(),
    };
    assert_eq!(message, "storage error: Permission refused");
}

// P3 #8: the `PermissionDenied` variant exists and is distinct from
// `PermissionLost` (the platform-refused shape vs. revocation-after-the-fact).
#[test]
fn storage_error_permission_denied_is_a_distinct_variant() {
    let denied = StorageError::PermissionDenied;
    assert!(matches!(denied, StorageError::PermissionDenied));
    // The two variants are distinct: the platform-refused shape is
    // never observed as a revocation.
    assert!(!matches!(
        StorageError::PermissionLost,
        StorageError::PermissionDenied
    ));
    assert!(!matches!(
        StorageError::PermissionDenied,
        StorageError::PermissionLost
    ));
}

// `LibraryRoot::placeholder()` returns a usable default for the
// `PermissionLost` payload the Android provider surfaces when the
// persisted entry is missing.
#[test]
fn library_root_placeholder_is_usable() {
    let placeholder = notae_app_core::storage::LibraryRoot::placeholder();
    assert_eq!(placeholder.display_name(), "");
    assert_eq!(placeholder.handle(), "");
}
