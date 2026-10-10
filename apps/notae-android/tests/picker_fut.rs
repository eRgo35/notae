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
    new_picker_state, poll_outcome, DispatchOutcome, PickerOutcome, SelectRootHandshake,
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

// 7. After `dispatch`, the waker the future registered is fired.
//    The handshake's "callback fires on the Java main looper,
//    poll runs on the Slint thread" property is what this test
//    guards — the future MUST be woken so the Slint executor gets
//    a chance to drive it on the next iteration.
#[test]
fn dispatch_wakes_the_future_waker() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::task::{Wake, Waker};

    /// A tiny `Arc<AtomicBool>`-backed `Waker` whose `wake` flips
    /// the flag. `Arc<AtomicWaker>` already does this internally;
    /// we use a separate counter so the test can assert the waker
    /// was fired without poking into `AtomicWaker`'s private state.
    struct FlagWaker(Arc<AtomicBool>);

    impl Wake for FlagWaker {
        fn wake(self: Arc<Self>) {
            self.0.store(true, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let flag = Arc::new(AtomicBool::new(false));
    let waker: Waker = Arc::new(FlagWaker(Arc::clone(&flag))).into();

    let state = new_picker_state();
    let mut future = SelectRootHandshake::new(state.clone_state(), 17);

    // First poll: pending, registers the waker.
    let mut context = Context::from_waker(&waker);
    assert!(matches!(
        Pin::new(&mut future).poll(&mut context),
        Poll::Pending
    ));

    // The handshake guarantees at least one wake when the
    // callback fires. The dispatch must wake the future.
    state.dispatch(17, PickerOutcome::Cancelled);
    assert!(
        flag.load(Ordering::SeqCst),
        "the registered waker MUST be fired after dispatch",
    );

    // The next poll completes the future.
    assert!(matches!(
        Pin::new(&mut future).poll(&mut context),
        Poll::Ready(Ok(None))
    ));
}

// 8. Round 4's host test: drive the same `poll_outcome` helper the
//    Android-only `SelectRootFuture::poll` uses, and prove that:
//
//    (a) A pending poll registers the waker into the dispatcher's
//        map (`PickerState.inner.pending`), not into a parallel map
//        the dispatcher never walks.
//    (b) The next dispatch fires that waker (no other thread
//        needed).
//    (c) The poll after the dispatch returns `Poll::Ready` with
//        the bridged outcome.
//
//    This is the regression net for the round-3 bug
//    (`SelectRootFuture::poll` registering into
//    `PENDING_ROOT_ENTRIES`, a different map the dispatcher never
//    walked). The host tests can't construct
//    `SelectRootFuture` (it requires an `AndroidApp`), but the
//    polling helper *is* the production path; covering it covers
//    the bug.
#[test]
fn poll_outcome_registers_waker_into_dispatchers_map_and_completes() {
    use std::sync::Arc;
    use atomic_waker::AtomicWaker;
    use std::task::{Wake, Waker};

    struct FlagWaker(std::sync::atomic::AtomicBool);
    impl Wake for FlagWaker {
        fn wake(self: Arc<Self>) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    let flag = Arc::new(FlagWaker(std::sync::atomic::AtomicBool::new(false)));
    let registered_waker: Waker = Arc::clone(&flag).into();
    let mut context = Context::from_waker(&registered_waker);

    let state = new_picker_state();
    let code: i32 = state.next_request_code();
    let shared = Arc::new(AtomicWaker::new());

    // (a) Pending poll: waker is registered into the dispatcher's map.
    let first = poll_outcome(&state, code, &shared, &mut context);
    assert!(matches!(first, Poll::Pending), "first poll must be Pending");
    {
        // The waker lives in `state.inner.pending` keyed by
        // `code`. If a future registered into a parallel map
        // (`PENDING_ROOT_ENTRIES` was the round-3 bug), this
        // assertion would fail.
        let pending_len = state.clone_state().inner_pending_len();
        assert_eq!(
            pending_len, 1,
            "the waker must be registered in `PickerState`'s map (the dispatcher walks it)",
        );
    }

    // (b) Dispatch fires the registered waker.
    state.dispatch(code, ok_outcome());
    assert!(
        flag.0.load(std::sync::atomic::Ordering::SeqCst),
        "dispatch MUST fire the waker the poll_outcome helper registered",
    );

    // (c) The next poll completes with the outcome — and the
    //     waker is consumed out of the map by `dispatch`/`take`.
    let second = poll_outcome(&state, code, &shared, &mut context);
    match second {
        Poll::Ready(Ok(Some(root))) => {
            assert_eq!(root.display_name(), "Documents");
        }
        other => panic!(
            "second poll must surface the dispatch outcome as `Ready(Ok(Some(root)))`, got {other:?}"
        ),
    }

    // After the future completes, the dispatcher's map no longer
    // holds the waker for this request code.
    assert_eq!(
        state.clone_state().inner_pending_len(),
        0,
        "the waker must be removed once the outcome has been delivered",
    );
}

// 9. Round 4's "callback before first poll" path through the same
//    helper: an outcome delivered before `poll_outcome` is ever
//    called must return `Poll::Ready` on the first invocation,
//    without ever registering a waker (the dispatcher is still
//    holding the waker slot's — there is no registered waker at
//    all until the future polls).
#[test]
fn poll_outcome_fast_paths_when_outcome_already_dispatched() {
    use std::sync::Arc;
    use atomic_waker::AtomicWaker;

    let state = new_picker_state();
    let code = state.next_request_code();
    state.dispatch(code, ok_outcome());

    let shared = Arc::new(AtomicWaker::new());
    let waker = Waker::noop().clone();
    let mut context = Context::from_waker(&waker);

    match poll_outcome(&state, code, &shared, &mut context) {
        Poll::Ready(Ok(Some(root))) => assert_eq!(root.display_name(), "Documents"),
        other => panic!("expected Ready(Ok(Some(root))) from fast path, got {other:?}"),
    }

    // Fast path never registered the waker — the pending map is
    // empty (the dispatcher's `or_insert` ran but the waker
    // stored there was the empty placeholder, then `wake` was
    // called on a noop waker). The pending count of `0` is the
    // observable side of the fast path.
    assert_eq!(
        state.clone_state().inner_pending_len(),
        0,
        "fast-path poll must not register a waker",
    );
}
