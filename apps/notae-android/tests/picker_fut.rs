//! Host tests for the SAF picker handshake.
//!
//! The handshake itself is JNI-free: a future awaits an external
//! wake-up, the activity-result callback delivers the outcome, the
//! future returns it on the next poll. Only the *triggers* (the JNI
//! call that launches the picker, the JNI hook that delivers the
//! result) are Android-specific, so the host test drives the
//! handshake directly without needing the Slint/AFX runtime.
//!
//! These tests stand in for the device-side check the reviewer asked
//! for: the future *must* return `Poll::Ready` once the callback
//! fires, and must not deadlock if the callback fires before the
//! future is ever polled.

use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use notae_android::picker_fut::{
    new_picker_state, DispatchOutcome, PickerOutcome, SelectRootHandshake,
};
use notae_android::storage::{LibraryRootId, StorageError};

/// Polls `future` once and returns the `Poll`. If `Poll::Pending` is
/// returned the helper has consumed the waker — the test can drive
/// the callback to wake it again.
fn poll_once<F>(future: &mut F) -> Poll<F::Output>
where
    F: Future + Unpin,
{
    // `Waker::noop()` is enough — the tests drive the wake via
    // `state.dispatch`, never through the waker's own `wake`. The
    // atomic waker stored inside the future gets a copy of this and
    // is woken once the test calls `dispatch`.
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    Pin::new(future).poll(&mut context)
}

fn root_id_value() -> LibraryRootId {
    LibraryRootId::from_string("root-test")
}

fn ok_outcome() -> PickerOutcome {
    PickerOutcome::Ok {
        tree_uri: "content://com.android.externalstorage.documents/tree/primary%3ADocuments".to_owned(),
        display_name: "Documents".to_owned(),
        root_id: root_id_value(),
    }
}

// 1. The future stays pending until the callback delivers an outcome.
#[test]
fn future_is_pending_until_callback_delivers_an_outcome() {
    let state = new_picker_state();
    let mut future = SelectRootHandshake::new(state.clone_state(), 1);

    assert!(matches!(poll_once(&mut future), Poll::Pending));

    state.dispatch(1, PickerOutcome::Cancelled);
    // The callback writes the outcome and wakes the waker. Polling
    // again completes the future.
    assert!(matches!(
        poll_once(&mut future),
        Poll::Ready(Ok(None)) // Cancelled maps to Ok(None)
    ));
}

// 2. The future returns the picked URI as `Ok(Some(LibraryRoot))`.
#[test]
fn future_returns_picked_uri_as_library_root() {
    let state = new_picker_state();
    let mut future = SelectRootHandshake::new(state.clone_state(), 7);

    assert!(matches!(poll_once(&mut future), Poll::Pending));

    state.dispatch(7, ok_outcome());

    match poll_once(&mut future) {
        Poll::Ready(Ok(Some(root))) => {
            assert_eq!(root.display_name(), "Documents");
            assert_eq!(root.id(), &root_id_value());
        }
        other => panic!("expected Ready(Ok(Some(root))), got {other:?}"),
    }
}

// 3. If the callback fires BEFORE the future is polled, the first
//    poll returns the result without ever registering a waker.
#[test]
fn callback_before_first_poll_returns_immediately() {
    let state = new_picker_state();
    // Dispatch happens before any poll. The outcome is already in the
    // state map.
    state.dispatch(42, ok_outcome());

    let mut future = SelectRootHandshake::new(state.clone_state(), 42);
    match poll_once(&mut future) {
        Poll::Ready(Ok(Some(root))) => assert_eq!(root.display_name(), "Documents"),
        other => panic!("expected Ready(Ok(Some(root))) without ever pending, got {other:?}"),
    }
}

// 4. A second `dispatch` for the same request code is ignored — the
//    first answer is the one the future observes.
#[test]
fn second_dispatch_for_same_request_code_is_ignored() {
    let state = new_picker_state();
    state.dispatch(1, ok_outcome());

    // A second dispatch with a different outcome must not overwrite
    // the first one.
    state.dispatch(
        1,
        PickerOutcome::Ok {
            tree_uri: "content://example.com/tree/Other".to_owned(),
            display_name: "Other".to_owned(),
            root_id: LibraryRootId::from_string("root-other"),
        },
    );

    let mut future = SelectRootHandshake::new(state.clone_state(), 1);
    match poll_once(&mut future) {
        Poll::Ready(Ok(Some(root))) => {
            // First dispatch wins.
            assert_eq!(root.display_name(), "Documents");
        }
        other => panic!("expected the first dispatch's outcome to win, got {other:?}"),
    }
}

// 5. Dispatching a `Platform` outcome returns `Err(StorageError)` so
//    the UI can render an error message.
#[test]
fn platform_failure_outcome_returns_err() {
    let state = new_picker_state();
    state.dispatch(
        3,
        PickerOutcome::Platform {
            error: "test-only JNI failure".to_owned(),
        },
    );

    let mut future = SelectRootHandshake::new(state.clone_state(), 3);
    match poll_once(&mut future) {
        Poll::Ready(Err(StorageError::Other(message))) => {
            assert_eq!(message, "test-only JNI failure");
        }
        other => panic!("expected Ready(Err(StorageError::Other(_))), got {other:?}"),
    }
}

// 6. Sanity check: the `DispatchOutcome` helper exists and gives a
//    `Platform` outcome.
#[test]
fn dispatch_outcome_helper_constructs_platform_outcome() {
    let outcome = DispatchOutcome::platform("oops");
    match outcome {
        PickerOutcome::Platform { error } => assert_eq!(error, "oops"),
        _ => panic!("DispatchOutcome::platform produced a non-Platform outcome"),
    }
}
