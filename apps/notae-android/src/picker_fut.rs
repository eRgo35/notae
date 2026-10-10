//! Host-testable core of the SAF picker handshake.
//!
//! The Storage Access Framework picker is launched by a JNI call and
//! delivers its result through the Android Activity-Result callback
//! running on the Java main looper. We bridge the two with this
//! module:
//!
//! * [`PickerState`] stores one entry per in-flight request: the
//!   waker the future registered, plus the outcome the callback
//!   wrote. The future's `poll` reads the outcome first, registers
//!   its waker only if the outcome is not yet present, and returns
//!   `Poll::Ready` once the outcome is in place.
//! * [`SelectRootHandshake`] is the future returned by
//!   [`select_root`](super::storage::LibraryRootProvider::select_root).
//!   On Android it is built with the `request_code` the JNI helper
//!   returned when it called `startActivityForResult`; the activity
//!   result callback looks up the same request code in
//!   [`PickerState::dispatch`]
//!   to deliver the answer.
//!
//! # Concurrency model
//!
//! The future runs on whichever thread called `poll` (the Slint main
//! thread on Android); the callback runs on the Java main looper.
//! Both threads contend on [`PickerState`]'s `Mutex<HashMap<…>>`. We
//! could use lock-free atomics here, but a single active picker at a
//! time is the contract: opening a second one cancels the first via
//! the OS, so contention is bounded by user behaviour, not by
//! anything a perf-sensitive path sees.
//!
//! # Testing
//!
//! `tests/picker_fut.rs` drives the handshake from the host test
//! thread: it never touches JNI, so it runs on Linux without the
//! `android-activity` runtime. The test asserts that the future
//! stays pending, that an outcome delivered before the first poll
//! returns immediately, and that a second dispatch for the same
//! request code is ignored so a sloppy double-callback does not
//! produce inconsistent results.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use atomic_waker::AtomicWaker;

use crate::storage::{LibraryRoot, LibraryRootId, StorageError};

const RESULT_OK: i32 = -1;
const RESULT_CANCELED: i32 = 0;
pub const FLAG_GRANT_READ_URI_PERMISSION: i32 = 1;

/// Decodes a Java Activity result before the Android bridge touches it.
///
/// A canceled picker is a normal no-selection outcome. A successful
/// result is accepted only when it contains a non-empty URI and the
/// read permission Notae needs to enumerate PDFs. Unknown result codes
/// and malformed success results become a storage error.
pub fn decode_activity_result(
    result_code: i32,
    tree_uri: Option<String>,
    grant_flags: i32,
) -> Result<Option<String>, StorageError> {
    match result_code {
        RESULT_CANCELED => Ok(None),
        RESULT_OK => {
            let Some(tree_uri) = tree_uri.filter(|uri| !uri.is_empty()) else {
                return Err(StorageError::Other(
                    "SAF picker returned no tree URI".to_owned(),
                ));
            };
            if grant_flags & FLAG_GRANT_READ_URI_PERMISSION == 0 {
                return Err(StorageError::PermissionDenied);
            }
            Ok(Some(tree_uri))
        }
        code => Err(StorageError::Other(format!(
            "SAF picker returned unexpected result code {code}",
        ))),
    }
}

/// Outcome of one picker request.
///
/// The handshake hands one of these to the future when the activity
/// result callback fires. The variants split cleanly into "user
/// dismissed", "user picked this tree", and "platform failure" so the
/// future can return the right shape without parsing strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerOutcome {
    /// The user dismissed the picker without choosing anything.
    Cancelled,
    /// The user picked a tree URI we successfully parsed.
    Ok {
        /// The `tree:` URI returned by the platform.
        tree_uri: String,
        /// The folder name the platform reports (the human-readable
        /// label, used in the recovery screen).
        display_name: String,
        /// The library-root id the future should yield. The bridge
        /// stores it here so the future builds a stable `LibraryRoot`
        /// even if `LibraryRootId::from_string` would otherwise pick
        /// a different counter value.
        root_id: LibraryRootId,
    },
    /// The picker failed for a platform reason (JNI error, security
    /// exception, …). The string is what the platform reported.
    Platform {
        /// Platform-reported error message.
        error: String,
    },
}

/// Builder for [`PickerOutcome::Platform`]. Exists so the JNI helper
/// does not need to spell the variant with its private field name.
#[derive(Debug, Clone, Copy)]
pub struct DispatchOutcome {
    _private: (),
}

impl DispatchOutcome {
    /// Wraps a platform error message as a `PickerOutcome::Platform`.
    #[must_use]
    pub fn platform(error: impl Into<String>) -> PickerOutcome {
        PickerOutcome::Platform {
            error: error.into(),
        }
    }
}

/// Shared state for the SAF picker handshake.
///
/// One global instance lives at `crate::android::saf::PICKER_STATE`
/// on Android (and the host tests construct one inline). It is
/// `Clone` (the inner `Mutex<…>` is wrapped in `Arc`) so any
/// function that touches the handshake can borrow it cheaply.
#[derive(Clone)]
pub struct PickerState {
    inner: Arc<PickerStateInner>,
}

struct PickerStateInner {
    /// In-flight requests keyed by their request code. Each entry is
    /// the `AtomicWaker` the future registered; the
    /// [`dispatch`](Self::dispatch) callback drops the entry as soon
    /// as it wakes the future.
    pending: Mutex<HashMap<i32, Arc<AtomicWaker>>>,
    /// Outcomes keyed by request code. Stays populated until
    /// consumed or until the request code is reused.
    outcomes: Mutex<HashMap<i32, PickerOutcome>>,
    /// Monotonic counter for the next request code.
    next_code: AtomicI32,
}

/// Builds a fresh [`PickerState`]. Convenience free function so the
/// handshake owns its preferred constructor spelling (`new_picker_state`)
/// rather than the JNI-side `PickerState::new` calling convention.
#[must_use]
pub fn new_picker_state() -> PickerState {
    PickerState::new()
}

impl PickerState {
    /// Builds a fresh, empty state. The hash maps start empty, the
    /// counter starts at `1` (so a request code of `0` — `RESULT_CANCELED`
    /// in Java — never accidentally collides).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(PickerStateInner {
                pending: Mutex::new(HashMap::new()),
                outcomes: Mutex::new(HashMap::new()),
                next_code: AtomicI32::new(1),
            }),
        }
    }

    /// Reserves a fresh request code, returning the value to use in
    /// the JNI `startActivityForResult` call. Starts at `1`.
    pub fn next_request_code(&self) -> i32 {
        self.inner.next_code.fetch_add(1, Ordering::Relaxed)
    }

    /// Records the waker the future registered under `request_code`.
    /// Replacing an existing entry is allowed; the waker contract
    /// guarantees at least one wake is delivered when
    /// [`dispatch`](Self::dispatch) fires.
    ///
    /// `pub` (not `fn`) so the Android-only
    /// [`SelectRootFuture`](crate::android::saf::SelectRootFuture) can
    /// register its waker into the *same* map `dispatch` walks —
    /// writing into a parallel map is exactly the bug round 4 fixed.
    /// The map is the single source of truth; both the
    /// host-testable [`SelectRootHandshake`] and the production
    /// Android future use this method.
    pub fn register_waker(&self, request_code: i32, waker: Arc<AtomicWaker>) {
        let mut guard = self
            .inner
            .pending
            .lock()
            .expect("pending requests mutex poisoned");
        guard.insert(request_code, waker);
    }

    /// Looks up the outcome for a request code, draining it so the
    /// next `select_root` for the same code observes no stale value.
    fn take_outcome(&self, request_code: i32) -> Option<PickerOutcome> {
        let mut guard = self
            .inner
            .outcomes
            .lock()
            .expect("outcomes mutex poisoned");
        guard.remove(&request_code)
    }

    /// Looks up the outcome for a request code without draining it.
    /// The Android `select_root` future uses this to know whether the
    /// bridge has already delivered an answer for the request code
    /// it allocated.
    pub fn peek_outcome(&self, request_code: i32) -> Option<PickerOutcome> {
        let guard = self
            .inner
            .outcomes
            .lock()
            .expect("outcomes mutex poisoned");
        guard.get(&request_code).cloned()
    }

    /// Drains the outcome for a request code. The future calls this
    /// once it has confirmed the bridge has written one, so the next
    /// `select_root` for the same request code observes a clean
    /// state.
    pub fn take_outcome_for(&self, request_code: i32) -> Option<PickerOutcome> {
        self.take_outcome(request_code)
    }

    /// Called by the activity-result callback (or any test stand-in
    /// for it) with the answer for `request_code`. The first
    /// dispatch wins: subsequent dispatches for the same code are
    /// ignored so a sloppy `onActivityResult` chain cannot rewrite a
    /// recorded outcome.
    pub fn dispatch(&self, request_code: i32, outcome: PickerOutcome) {
        let waker = {
            let mut guard = self
                .inner
                .pending
                .lock()
                .expect("pending requests mutex poisoned");
            guard.remove(&request_code)
        };
        {
            let mut guard = self
                .inner
                .outcomes
                .lock()
                .expect("outcomes mutex poisoned");
            guard.entry(request_code).or_insert(outcome);
        }
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Clones the underlying `Arc` so callers (typically tests) can
    /// observe the state from outside the future. Production code
    /// already holds a [`Clone`] of the [`PickerState`]; this method
    /// exists so the integration tests can reproduce that clone
    /// without going through `Clone` (which would obscure the
    /// `cargo test --test picker_fut` reading).
    #[must_use]
    pub fn clone_state(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Number of waker slots currently registered. `pub` so the
    /// integration tests in `tests/picker_fut.rs` can assert that
    /// `poll_outcome` / `dispatch` are operating on the same map
    /// (the round-4 regression net for the parallel-map bug).
    /// `inner` is private to keep the rest of the surface narrow;
    /// this accessor is the safe handle tests use to inspect the
    /// dispatcher's map.
    #[must_use]
    pub fn inner_pending_len(&self) -> usize {
        let guard = self
            .inner
            .pending
            .lock()
            .expect("pending requests mutex poisoned");
        guard.len()
    }
}

impl Default for PickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PickerState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pending = self
            .inner
            .pending
            .lock()
            .expect("pending requests mutex poisoned");
        let outcomes = self
            .inner
            .outcomes
            .lock()
            .expect("outcomes mutex poisoned");
        formatter
            .debug_struct("PickerState")
            .field("pending", &pending.len())
            .field("outcomes", &outcomes.len())
            .field("next_code", &self.inner.next_code.load(Ordering::Relaxed))
            .finish()
    }
}

/// The future returned by [`SafLibraryRootProvider::select_root`].
///
/// The future is one-shot: once it returns a value, polling it again
/// returns `Poll::Ready` with the same value (we only ever get one
/// outcome per request code, and the future owns that request code).
pub struct SelectRootHandshake {
    state: PickerState,
    request_code: i32,
    /// The waker we registered, kept here so `poll` can re-register
    /// across multiple pending polls.
    waker: Arc<AtomicWaker>,
}

impl SelectRootHandshake {
    /// Builds the future. The `request_code` is what the JNI helper
    /// passed to `startActivityForResult`; the activity-result
    /// callback dispatches the matching `request_code` to
    /// [`PickerState::dispatch`].
    #[must_use]
    pub fn new(state: PickerState, request_code: i32) -> Self {
        Self {
            state,
            request_code,
            waker: Arc::new(AtomicWaker::new()),
        }
    }

    /// Returns the request code the future was built for. Exposed
    /// for the JNI helper that hands the answer to
    /// [`PickerState::dispatch`].
    #[must_use]
    pub fn request_code(&self) -> i32 {
        self.request_code
    }
}

impl Future for SelectRootHandshake {
    type Output = Result<Option<LibraryRoot>, StorageError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        poll_outcome(&me.state, me.request_code, &me.waker, context)
    }
}

fn outcome_to_result(outcome: PickerOutcome) -> Result<Option<LibraryRoot>, StorageError> {
    match outcome {
        PickerOutcome::Cancelled => Ok(None),
        PickerOutcome::Ok {
            tree_uri,
            display_name,
            root_id,
        } => {
            // The root id carried in the outcome is the one the
            // bridge allocated when it launched the picker, so two
            // polls across the same request code return roots with
            // matching ids.
            let root = LibraryRoot::from_parts(root_id, display_name, tree_uri);
            Ok(Some(root))
        }
        PickerOutcome::Platform { error } => Err(StorageError::Other(error)),
    }
}

/// One-shot poll helper for any future that awaits an
/// `PickerOutcome` under `request_code`. Returns `Poll::Ready` once
/// the bridge has delivered an outcome (consuming it), or
/// `Poll::Pending` after registering `waker` so the next dispatch
/// wakes the caller.
///
/// This is the **single** waker-registration path used by every
/// picker future in the crate. Before round 4, the Android
/// `SelectRootFuture` registered into a parallel map
/// (`PENDING_ROOT_ENTRIES`) that the dispatcher never walked; the
/// wake never reached the future. Both the host-testable
/// [`SelectRootHandshake`] and the production Android future now
/// call this function, so the host tests exercise the same
/// code path the device ships.
///
/// The `waker` is `Arc`-shared with the [`PickerState`] so the
/// dispatcher's wake can fire even after `poll` returns
/// `Poll::Pending` and the future drops. The `AtomicWaker`
/// contract guarantees the wake still reaches the consumer.
pub fn poll_outcome(
    state: &PickerState,
    request_code: i32,
    waker: &Arc<AtomicWaker>,
    context: &mut Context<'_>,
) -> Poll<Result<Option<LibraryRoot>, StorageError>> {
    // Fast path: an answer is already in. Skip the waker
    // registration entirely — the callback has fired, and
    // registering a waker now would only wake ourselves.
    if let Some(outcome) = state.take_outcome(request_code) {
        return Poll::Ready(outcome_to_result(outcome));
    }
    // Slow path: register the waker, then re-check the outcome
    // so we do not race with a callback that fires between the
    // first read and the registration.
    waker.register(context.waker());
    state.register_waker(request_code, Arc::clone(waker));
    if let Some(outcome) = state.take_outcome(request_code) {
        return Poll::Ready(outcome_to_result(outcome));
    }
    Poll::Pending
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_outcome_helper_builds_platform() {
        let outcome = DispatchOutcome::platform("kaboom");
        assert_eq!(
            outcome,
            PickerOutcome::Platform {
                error: "kaboom".to_owned(),
            }
        );
    }

    #[test]
    fn request_codes_are_unique() {
        let state = PickerState::new();
        let first = state.next_request_code();
        let second = state.next_request_code();
        let third = state.next_request_code();
        assert!(first >= 1);
        assert!(second > first);
        assert!(third > second);
    }
}
