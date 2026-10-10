//! Slint glue between the `state` model and the `app.slint` UI.
//!
//! This module is Android-only: the `app.slint` build script emits
//! bindings into `$OUT_DIR/app.rs`, and those bindings reference
//! platform symbols that aren't on the build graph when the host smoke
//! test runs. Keeping the Slint glue behind `cfg(target_os = "android")`
//! lets the same crate expose the host-testable [`state`] module on
//! both targets.
//!
//! [`state`]: crate::state

#![forbid(unsafe_code)]

use crate::state::{AppController, AppState, EventError, Transition, UiEvent};

/// Bridge between the Slint-generated UI types and the [`AppState`].
///
/// The glue is intentionally thin: it owns an [`AppController`], projects
/// the visible fields of [`AppState`] into Slint properties, and
/// forwards Slint callbacks (Slint -> Rust) as [`UiEvent`]s.
///
/// `start_rescan` is exposed so the Slint layer has a one-call button
/// handler, but `UiModel::new()` never calls it — the fresh-app state
/// is `Idle` and only transitions out of `Idle` on a user-driven
/// `UiEvent`. The smoke test locks that invariant in.
pub struct UiModel {
    controller: AppController,
}

impl UiModel {
    /// Builds a `UiModel` around the supplied controller. Used by the
    /// Android entry point to share an `AppController` between the UI
    /// loop and a future worker thread (Task 4).
    pub fn from_controller(controller: AppController) -> Self {
        Self { controller }
    }

    /// Builds a `UiModel` around a fresh `AppController`. The resulting
    /// `AppState` matches the `Setup::Unconfigured` / `Idle` contract
    /// the smoke test asserts on Linux.
    pub fn new() -> Self {
        Self::from_controller(AppController::new())
    }

    /// Returns a reference to the underlying [`AppController`]. Used by
    /// future tasks (Task 4 in particular) to plug a worker thread into
    /// the same state machine the UI is mutating.
    #[allow(dead_code)] // Touched by Task 4's worker integration.
    pub fn controller(&self) -> &AppController {
        &self.controller
    }

    /// Returns a mutable reference to the underlying [`AppController`].
    /// Same future use as [`UiModel::controller`].
    #[allow(dead_code)] // Touched by Task 4's worker integration.
    pub fn controller_mut(&mut self) -> &mut AppController {
        &mut self.controller
    }

    pub fn state(&self) -> &AppState {
        self.controller.state()
    }

    /// Apply a `UiEvent` to the underlying controller. Convenience for
    /// the Slint callback wiring in `app.slint`.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::OperationAlreadyInProgress`] when an event
    /// would overlap an existing rescan/import. The UI is supposed to
    /// disable the relevant controls while the operation is in flight,
    /// so this is a guard rather than a control-flow signal.
    pub fn dispatch(&mut self, event: UiEvent) -> Result<Transition, EventError> {
        self.controller.dispatch(event)
    }

    /// The "Rescan" button handler. Forwards to
    /// [`AppController::dispatch`] with [`UiEvent::RequestRescan`].
    pub fn start_rescan(&mut self) -> Result<Transition, EventError> {
        self.dispatch(UiEvent::RequestRescan)
    }
}

impl Default for UiModel {
    fn default() -> Self {
        Self::new()
    }
}
