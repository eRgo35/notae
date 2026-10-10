//! Application state for the Notae Android UI.
//!
//! [`AppState`] is the single source of truth for what the user sees.
//! It owns four sub-states: the current [`Screen`], the [`LibraryState`]
//! (rows shown in the library list), the [`SearchState`] (the query
//! the user typed), and the [`OperationState`] (whether a rescan or
//! import preview is in flight).
//!
//! State transitions are driven by [`UiEvent`] values. The transitions
//! live on [`AppController`]: it wraps an `AppState` and enforces the
//! invariants the UI relies on — a fresh app exposes the setup screen,
//! the operation starts `Idle`, and nothing begins a rescan until the
//! user explicitly asks for one.
//!
//! This module is host-testable on Linux and intentionally has no
//! dependency on `slint`, on `android-activity`, or on any FFI symbol.
//! The Android FFI shim in `android_main` lives in a sibling module
//! that opts back into `unsafe_code` only for the `#[no_mangle]` entry
//! point.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// A user action received from the Slint layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// The user edited the search box.
    UpdateSearchQuery {
        /// The new text in the search field, after the edit.
        query: String,
    },
    /// The user tapped the "Rescan" button.
    RequestRescan,
    /// The user picked a directory as the new library root.
    PickLibraryRoot {
        /// The directory the user selected.
        root: PathBuf,
    },
    /// The user confirmed the import preview the sidecar importer produced.
    ConfirmImport,
    /// The user cancelled the import preview.
    CancelImport,
    /// The user dismissed a success or failure banner after a long-running
    /// operation completed.
    AcknowledgeOperation,
    /// The user opened the settings screen.
    OpenSettings,
    /// The user closed the settings screen, returning to whatever screen
    /// was below it (the library in normal flow).
    CloseSettings,
}

/// The instruction the controller hands back after a transition.
///
/// `BeginRescan` means the worker thread should kick off (or continue)
/// a rescan once the UI has had a moment to display the new
/// `OperationState`. The controller exposes this transition; Task 5
/// connects it to the background worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// No external work needs to happen. Used for events that only touch
    /// in-memory state (search query update, screen change, etc.).
    NoOp,
    /// The state machine has accepted a request that needs worker-thread
    /// follow-through: a rescan is now in flight and the worker should
    /// pick it up.
    BeginRescan,
}

/// Reasons a `UiEvent` could be rejected without applying it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
    /// A second long-running operation started while another was already
    /// in flight. The UI is supposed to disable the rescan button while
    /// `OperationState::RescanInProgress`, so reaching this branch means
    /// the wiring let a stale click through.
    #[error("a rescan or import is already in progress")]
    OperationAlreadyInProgress,
    /// Settings are only available while a configured library is open.
    #[error("settings are unavailable until a library is open")]
    SettingsUnavailable,
}

/// What top-level view the user is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// The library is unconfigured, lost, or unopenable. The variant
    /// carries the specific reason so the UI can render appropriate
    /// guidance.
    Setup(Setup),
    /// The configured library is open; show the indexed notes.
    Library,
    /// The settings screen.
    Settings,
}

/// Why the app is on the setup screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setup {
    /// No library has been chosen yet. This is the state a fresh install
    /// starts in.
    Unconfigured,
    /// A library was configured previously but the root is gone (SD
    /// card removed, app reinstalled, etc.).
    PermissionLost,
    /// The configured library root exists but cannot be opened. The
    /// payload is a short, user-facing sentence — not a raw `io::Error`.
    Error {
        /// Short, user-facing sentence describing the failure.
        message: String,
    },
}

/// Rows shown in the library list after indexing.
///
/// The state starts empty. Task 5 populates rows from completed index
/// operations; Task 4 owns the separate search projection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryState {
    /// The rows to show in the library list, in display order.
    pub rows: Vec<LibraryRow>,
}

/// A compact library row retained by the application state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRow {
    /// The portable [`note_id`](notae_domain::NoteId) for this record.
    pub note_id: String,
    /// The display title (parsed from the filename, or the OCR'd first
    /// page once OCR is wired up in Task 7).
    pub title: String,
    /// The PDF's path relative to the configured library root.
    pub relative_path: PathBuf,
    /// A short summary of the indexing status.
    pub status: String,
}

/// What the search box currently contains.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchState {
    /// The current text in the search box. Empty by default.
    pub query: String,
}

/// What long-running operation, if any, is in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationState {
    /// Nothing is happening. A fresh app is in this state.
    Idle,
    /// A rescan is running against the configured library.
    RescanInProgress {
        /// Files classified so far.
        completed: usize,
        /// Files found by the catalog.
        total: usize,
    },
    /// A sidecar import is ready for the user to confirm.
    ImportPreviewPending {
        /// How many records the importer wants the user to confirm.
        candidate_count: usize,
    },
    /// A confirmed sidecar import is being written.
    ImportApplying,
    /// A long-running operation just finished successfully. The
    /// controller moves this back to `Idle` once the user dismisses the
    /// banner.
    Completed {
        /// Short, human-readable summary (e.g. "47 indexed").
        summary: String,
    },
    /// A long-running operation failed.
    Failed {
        /// Short, user-facing reason — not a raw error chain.
        reason: String,
    },
}

/// The full application state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    screen: Screen,
    library: LibraryState,
    search: SearchState,
    operation: OperationState,
}

impl AppState {
    /// The state a fresh install starts in.
    ///
    /// `Screen::Setup(Setup::Unconfigured)` and `OperationState::Idle`
    /// are the contract the host smoke test asserts. No automatic rescan
    /// is started: `AppState::new()` is the only state a fresh process
    /// observes until a `UiEvent` drives a transition.
    pub fn new() -> Self {
        Self {
            screen: Screen::Setup(Setup::Unconfigured),
            library: LibraryState::default(),
            search: SearchState::default(),
            operation: OperationState::Idle,
        }
    }

    /// Returns the current top-level [`Screen`].
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Returns the current [`LibraryState`] (rows, ordering, etc.).
    pub fn library(&self) -> &LibraryState {
        &self.library
    }

    /// Returns the current [`SearchState`] (the search box contents).
    pub fn search(&self) -> &SearchState {
        &self.search
    }

    /// Returns the current [`OperationState`] (whether a rescan or
    /// import is in flight, or what just finished).
    pub fn operation(&self) -> &OperationState {
        &self.operation
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// The mutable side of the application — owns the `AppState` and applies
/// `UiEvent`s to it.
///
/// `AppController` is `Send + Sync` (the inner state only carries owned
/// `String` / `PathBuf` / numeric values) so future tasks can move it to
/// a worker thread for background indexing without reshuffling the state
/// type.
#[derive(Debug, Clone)]
pub struct AppController {
    state: AppState,
}

impl AppController {
    /// Constructs a controller around a fresh `AppState`.
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    /// Constructs a controller around an existing `AppState`. Used by
    /// tests and by the Android entry point once the state has been
    /// rebuilt from persisted preferences.
    pub fn from_state(state: AppState) -> Self {
        Self { state }
    }

    /// Returns the inner [`AppState`]. Read-only — transitions go
    /// through [`AppController::dispatch`].
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Applies a `UiEvent`, returning the side-effect instruction the
    /// worker should run.
    ///
    /// Returns `Err(EventError::OperationAlreadyInProgress)` when the
    /// event would overlap an existing rescan/import — the UI disables
    /// the relevant controls in that case, so this is a wiring error.
    pub fn dispatch(&mut self, event: UiEvent) -> Result<Transition, EventError> {
        match event {
            UiEvent::UpdateSearchQuery { query } => {
                self.state.search.query = query;
                Ok(Transition::NoOp)
            }
            UiEvent::RequestRescan => self.start_rescan(),
            UiEvent::PickLibraryRoot { root } => {
                self.state.screen = Screen::Library;
                // The chosen root takes effect immediately; the actual
                // sidecar import (Task 5) is what produces an import
                // preview. For Task 1 we just move off the setup screen.
                self.state.library = LibraryState::default();
                let _ = root;
                Ok(Transition::BeginRescan)
            }
            UiEvent::ConfirmImport => {
                if !matches!(
                    self.state.operation,
                    OperationState::ImportPreviewPending { .. }
                ) {
                    return Err(EventError::OperationAlreadyInProgress);
                }
                self.state.operation = OperationState::ImportApplying;
                Ok(Transition::NoOp)
            }
            UiEvent::CancelImport => {
                if !matches!(
                    self.state.operation,
                    OperationState::ImportPreviewPending { .. }
                ) {
                    return Err(EventError::OperationAlreadyInProgress);
                }
                self.state.operation = OperationState::Idle;
                Ok(Transition::NoOp)
            }
            UiEvent::AcknowledgeOperation => {
                let was_terminal = matches!(
                    self.state.operation,
                    OperationState::Completed { .. } | OperationState::Failed { .. }
                );
                if !was_terminal {
                    return Err(EventError::OperationAlreadyInProgress);
                }
                self.state.operation = OperationState::Idle;
                Ok(Transition::NoOp)
            }
            UiEvent::OpenSettings => {
                if !matches!(&self.state.screen, Screen::Library) {
                    return Err(EventError::SettingsUnavailable);
                }
                self.state.screen = Screen::Settings;
                Ok(Transition::NoOp)
            }
            UiEvent::CloseSettings => {
                if !matches!(&self.state.screen, Screen::Settings) {
                    return Err(EventError::SettingsUnavailable);
                }
                self.state.screen = Screen::Library;
                Ok(Transition::NoOp)
            }
        }
    }

    /// Updates visible rescan progress.
    pub fn update_rescan_progress(
        &mut self,
        completed: usize,
        total: usize,
    ) -> Result<(), EventError> {
        if !matches!(
            self.state.operation,
            OperationState::RescanInProgress { .. }
        ) {
            return Err(EventError::OperationAlreadyInProgress);
        }
        self.state.operation = OperationState::RescanInProgress { completed, total };
        Ok(())
    }

    /// Marks a worker operation as successfully completed.
    pub fn complete_operation(&mut self, summary: String) {
        self.state.operation = OperationState::Completed { summary };
    }

    /// Returns a canceled rescan to the idle state.
    pub fn cancel_operation(&mut self) -> Result<(), EventError> {
        if !matches!(
            self.state.operation,
            OperationState::RescanInProgress { .. }
        ) {
            return Err(EventError::OperationAlreadyInProgress);
        }
        self.state.operation = OperationState::Idle;
        Ok(())
    }

    /// Records a user-visible failure from the background worker.
    pub fn fail_operation(&mut self, reason: String) {
        self.state.operation = OperationState::Failed { reason };
    }

    /// Marks the read-only import preview as awaiting confirmation.
    pub fn import_preview_ready(&mut self, candidate_count: usize) {
        self.state.operation = OperationState::ImportPreviewPending { candidate_count };
    }

    /// Marks a user-confirmed import as canceled.
    pub fn cancel_import(&mut self) -> Result<(), EventError> {
        if !matches!(
            self.state.operation,
            OperationState::ImportPreviewPending { .. }
        ) {
            return Err(EventError::OperationAlreadyInProgress);
        }
        self.state.operation = OperationState::Idle;
        Ok(())
    }

    fn start_rescan(&mut self) -> Result<Transition, EventError> {
        match self.state.operation {
            OperationState::Idle => {
                self.state.operation = OperationState::RescanInProgress {
                    completed: 0,
                    total: 0,
                };
                Ok(Transition::BeginRescan)
            }
            _ => Err(EventError::OperationAlreadyInProgress),
        }
    }
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_is_setup_unconfigured_and_idle() {
        let state = AppState::new();
        assert_eq!(state.screen(), &Screen::Setup(Setup::Unconfigured));
        assert_eq!(state.operation(), &OperationState::Idle);
        assert!(state.search().query.is_empty());
        assert!(state.library().rows.is_empty());
    }

    #[test]
    fn fresh_controller_does_not_start_a_rescan_until_an_event_drives_it() {
        let controller = AppController::new();
        // `AppController::new` does nothing beyond constructing the
        // initial `AppState`: no background work, no timers, no thread.
        assert_eq!(controller.state().operation(), &OperationState::Idle);
        assert_eq!(
            controller.state().screen(),
            &Screen::Setup(Setup::Unconfigured)
        );
    }

    #[test]
    fn rescan_request_transitions_to_in_progress_and_rejects_repeat() {
        let mut controller = AppController::new();
        // The user can only request a rescan once they have configured a
        // library, so push the controller into the `Library` screen
        // first. In real flow, this comes from `PickLibraryRoot`.
        controller.state.screen = Screen::Library;
        assert_eq!(
            controller.dispatch(UiEvent::RequestRescan),
            Ok(Transition::BeginRescan)
        );
        assert_eq!(
            controller.state().operation(),
            &OperationState::RescanInProgress {
                completed: 0,
                total: 0,
            }
        );
        // A second rescan request must be rejected, not silently
        // overwritten.
        assert_eq!(
            controller.dispatch(UiEvent::RequestRescan),
            Err(EventError::OperationAlreadyInProgress)
        );
    }
}
