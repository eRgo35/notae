//! UI state bridge and host-testable library/search view models.
//!
//! `library` and `search` project the shared index into rows the Android
//! surface can display, and compile on Linux for deterministic tests. The
//! `UiModel` keeps storage and application state synchronized; the Slint
//! bindings and Android event loop remain in their Android-only modules.
//!
//! [`state`]: crate::state

#![forbid(unsafe_code)]
pub mod library;
pub mod search;

use std::rc::Rc;
use std::sync::Mutex;

use crate::state::{AppController, AppState, EventError, Transition, UiEvent};
use crate::storage::StorageState;
use crate::ui::search::SearchRow;

/// Bridge between the Slint-generated UI types and the [`AppState`].
///
/// The glue is intentionally thin: it owns an [`AppController`]
/// (which holds the in-memory screen / library / search state) and a
/// [`StorageState`] (which holds the SAF-side root and source). The
/// [`StorageState`] is what the picker manipulates; the
/// [`AppController`] is what the rest of the UI dispatches against.
/// They are kept distinct so the storage layer can be swapped (host
/// test vs. device) without touching the state machine the UI
/// consumes.
pub struct UiModel {
    controller: AppController,
    storage: Rc<Mutex<StorageState>>,
    search_rows: Vec<SearchRow>,
}

impl UiModel {
    /// Builds a `UiModel` around the supplied controller and storage
    /// state. Used by the Android entry point to share the storage
    /// state with the picker callback closures.
    #[must_use]
    pub fn from_parts(controller: AppController, storage: Rc<Mutex<StorageState>>) -> Self {
        Self {
            controller,
            storage,
            search_rows: Vec::new(),
        }
    }

    /// Builds a `UiModel` around a fresh `AppController` and an
    /// unconfigured storage state. The resulting state matches the
    /// `Setup::Unconfigured` / `Idle` contract the smoke test asserts
    /// on Linux.
    #[must_use]
    pub fn new() -> Self {
        Self::from_parts(
            AppController::new(),
            Rc::new(Mutex::new(StorageState::Unconfigured)),
        )
    }

    /// Returns a reference to the underlying [`AppController`]. Used by
    /// future tasks (Task 5 in particular) to plug the operation worker into
    /// the same state machine the UI is mutating.
    #[allow(dead_code)] // Touched by Task 5's worker integration.
    pub fn controller(&self) -> &AppController {
        &self.controller
    }

    /// Returns a mutable reference to the underlying [`AppController`].
    /// Same future use as [`UiModel::controller`].
    #[allow(dead_code)] // Touched by Task 5's worker integration.
    pub fn controller_mut(&mut self) -> &mut AppController {
        &mut self.controller
    }

    /// Updates the storage state in place.
    pub fn set_storage(&mut self, state: StorageState) {
        *self.storage.lock().expect("storage mutex poisoned") = state;
    }

    /// Replaces the rows currently displayed by the library screen.
    ///
    /// The operation worker owns the index and sends its search projection
    /// back to the UI thread through this method.
    pub fn set_search_rows(&mut self, rows: Vec<SearchRow>) {
        self.search_rows = rows;
    }

    /// Returns the rows most recently projected by the operation worker.
    #[must_use]
    pub fn search_rows(&self) -> &[SearchRow] {
        &self.search_rows
    }

    /// Returns the current [`AppState`].
    #[must_use]
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

/// A view over [`StorageState`] that drops the contained source or
/// root. The Slint layer uses it to pick which screen to show without
/// having to clone the underlying `Box<dyn PdfCatalog>` (which is not
/// `Clone`).
#[derive(Debug, Clone)]
pub enum StorageStateKindView {
    /// No library root chosen yet.
    Unconfigured,
    /// A library root is open and the catalog is ready.
    Ready,
    /// The previously-chosen root's persisted permission was revoked.
    /// The string is the root's display name, used as the body text
    /// of the recovery screen.
    PermissionLost(String),
    /// Storage failed in a way the user has to dismiss. The string is
    /// the user-facing error message the setup screen surfaces.
    Error(String),
}

impl UiModel {
    /// Returns the [`StorageStateKindView`] of the current storage
    /// state. The view drops the contained source or root.
    #[must_use]
    pub fn storage_kind(&self) -> StorageStateKindView {
        let guard = self.storage.lock().expect("storage mutex poisoned");
        match &*guard {
            StorageState::Unconfigured => StorageStateKindView::Unconfigured,
            StorageState::Ready(_) => StorageStateKindView::Ready,
            StorageState::PermissionLost(root) => {
                StorageStateKindView::PermissionLost(root.display_name().to_owned())
            }
            StorageState::Error(error) => StorageStateKindView::Error(error.to_string()),
        }
    }
}