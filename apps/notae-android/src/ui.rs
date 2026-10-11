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
pub mod metadata_editor;
pub mod search;

use std::rc::Rc;
use std::sync::Mutex;

use crate::operations::events::{
    ImportPreviewSummary, ImportSummary, OperationEvent, RescanSummary,
};
use crate::state::{AppController, AppState, EventError, Transition, UiEvent};
use crate::storage::StorageState;
use crate::ui::metadata_editor::EditableMetadata;
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
    import_preview_summary: Option<ImportPreviewSummary>,
    metadata_editor: Option<EditableMetadata>,
    metadata_error: Option<String>,
    pdf_launch_error: Option<String>,
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
            import_preview_summary: None,
            metadata_editor: None,
            metadata_error: None,
            pdf_launch_error: None,
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

    /// Returns the persisted SAF tree URI for the active library.
    #[must_use]
    pub fn library_root_handle(&self) -> Option<String> {
        let guard = self.storage.lock().ok()?;
        match &*guard {
            StorageState::Ready(source) => source
                .lock()
                .ok()
                .map(|source| source.root().handle().to_owned()),
            StorageState::Unconfigured
            | StorageState::PermissionLost(_)
            | StorageState::Error(_) => None,
        }
    }

    /// Returns the latest PDF launch error, when a viewer could not be opened.
    #[must_use]
    pub fn pdf_launch_error(&self) -> Option<&str> {
        self.pdf_launch_error.as_deref()
    }

    /// Replaces the user-facing PDF launch error.
    pub fn set_pdf_launch_error(&mut self, error: Option<String>) {
        self.pdf_launch_error = error;
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

    /// The current read-only sidecar preview counts, when one is awaiting
    /// confirmation.
    #[must_use]
    pub fn import_preview_summary(&self) -> Option<&ImportPreviewSummary> {
        self.import_preview_summary.as_ref()
    }

    /// Hides a sidecar preview that the user declined to apply.
    pub fn clear_import_preview(&mut self) {
        self.import_preview_summary = None;
    }

    /// The metadata currently shown by the editor, if one is open.
    #[must_use]
    pub fn metadata_editor(&self) -> Option<&EditableMetadata> {
        self.metadata_editor.as_ref()
    }

    /// Error from the latest metadata load/save attempt, if any.
    #[must_use]
    pub fn metadata_error(&self) -> Option<&str> {
        self.metadata_error.as_deref()
    }

    /// Closes the editor and clears its error message.
    pub fn close_metadata_editor(&mut self) {
        self.metadata_editor = None;
        self.metadata_error = None;
    }

    /// Opens an editor while the worker loads the selected record.
    pub fn begin_metadata_edit(&mut self, note_key: String) {
        self.metadata_error = None;
        self.metadata_editor = Some(EditableMetadata {
            note_key,
            ..EditableMetadata::default()
        });
    }

    /// Applies a worker event to the UI-owned state and row projection.
    pub fn apply_operation_event(&mut self, event: OperationEvent) -> Result<(), EventError> {
        match event {
            OperationEvent::Started => self.controller.update_rescan_progress(0, 0)?,
            OperationEvent::RescanFinalizing { total } => {
                self.controller.begin_rescan_finalization(total)?;
            }
            OperationEvent::Progress { completed, total } => {
                self.controller.update_rescan_progress(completed, total)?;
            }
            OperationEvent::FileSucceeded { .. } | OperationEvent::FileFailed { .. } => {}
            OperationEvent::Completed { summary, rows } => {
                self.search_rows = rows;
                self.controller.complete_operation(rescan_summary(&summary));
            }
            OperationEvent::Canceled => self.controller.cancel_operation()?,
            OperationEvent::ImportPreviewReady { summary } => {
                let candidate_count = summary.new + summary.changed;
                self.import_preview_summary = Some(summary);
                self.controller.import_preview_ready(candidate_count);
            }
            OperationEvent::ImportApplied { summary, rows } => {
                self.import_preview_summary = None;
                self.search_rows = rows;
                self.controller.complete_operation(import_summary(&summary));
            }
            OperationEvent::SearchRows(rows) => self.search_rows = rows,
            OperationEvent::MetadataLoaded(metadata) => {
                self.metadata_error = None;
                self.metadata_editor = Some(metadata);
            }
            OperationEvent::MetadataSaved { rows } => {
                self.search_rows = rows;
                self.close_metadata_editor();
            }
            OperationEvent::MetadataFailed { reason } => {
                self.metadata_error = Some(reason);
            }
            OperationEvent::Failed { reason } => self.controller.fail_operation(reason),
        }
        Ok(())
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

fn rescan_summary(summary: &RescanSummary) -> String {
    let mut parts = Vec::new();
    add_count(&mut parts, summary.added, "added");
    add_count(&mut parts, summary.updated, "updated");
    add_count(&mut parts, summary.moved, "moved");
    add_count(&mut parts, summary.deleted, "deleted");
    add_count(&mut parts, summary.conflicts, "ID conflicts");
    add_count(&mut parts, summary.failures, "failures");
    if parts.is_empty() {
        "No changes".to_owned()
    } else {
        parts.join(", ")
    }
}

fn import_summary(summary: &ImportSummary) -> String {
    let mut parts = Vec::new();
    add_count(&mut parts, summary.imported, "imported");
    add_count(&mut parts, summary.mismatches, "hash mismatches");
    add_count(&mut parts, summary.missing, "missing");
    add_count(&mut parts, summary.unreadable, "unreadable");
    add_count(&mut parts, summary.conflicts, "ID conflicts");
    add_count(&mut parts, summary.failures, "failures");
    if parts.is_empty() {
        "No import changes".to_owned()
    } else {
        parts.join(", ")
    }
}

fn add_count(parts: &mut Vec<String>, count: usize, label: &str) {
    if count != 0 {
        parts.push(format!("{count} {label}"));
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
