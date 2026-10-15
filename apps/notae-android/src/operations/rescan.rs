use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notae_domain::IndexStatus;
use notae_index::{
    IndexStore, OcrText, PdfFile, Reconciler, SearchScope, SidecarImporter, SidecarManifest,
};

use crate::operations::events::{ImportSummary, OperationEvent, RescanSummary};
use crate::operations::import::ImportPreviewViewModel;
use crate::storage::LibraryCatalog;
use crate::thumbnails::ThumbnailCache;
use crate::ui::library::LibraryViewModel;
use crate::ui::metadata_editor::{EditableMetadata, MetadataEditor};
use crate::ui::search::{SearchRow, UiError};

const CANCEL_OPEN: u8 = 0;
const CANCEL_REQUESTED: u8 = 1;
const CANCEL_CLOSED: u8 = 2;

/// Owns the worker that serializes all index access away from the UI thread.
pub struct OperationController {
    commands: Sender<WorkerCommand>,
    events: Receiver<OperationEvent>,
    busy: Arc<AtomicBool>,
    cancellation: Arc<AtomicU8>,
    worker: Option<JoinHandle<()>>,
}

enum WorkerCommand {
    Rescan,
    Search(String, SearchScope),
    PreviewImport(SidecarManifest),
    ConfirmImport,
    DiscardImport,
    LoadMetadata(String),
    SaveMetadata(EditableMetadata),
    Shutdown,
}

impl OperationController {
    /// Starts an idle worker owning the index and library catalog.
    ///
    /// Construction opens no files and starts no scan; the caller must request
    /// each rescan explicitly.
    ///
    /// # Errors
    /// Returns [`UiError::WorkerStart`] if the background worker cannot be started.
    pub fn new(
        local: IndexStore,
        catalog: Box<dyn LibraryCatalog + Send>,
    ) -> Result<Self, UiError> {
        Self::start(local, catalog, None)
    }

    /// Like [`OperationController::new`], also rendering first-page thumbnails into `cache`.
    ///
    /// # Errors
    /// Returns [`UiError::WorkerStart`] if the background worker cannot be started.
    pub fn with_thumbnails(
        local: IndexStore,
        catalog: Box<dyn LibraryCatalog + Send>,
        cache: ThumbnailCache,
    ) -> Result<Self, UiError> {
        Self::start(local, catalog, Some(cache))
    }

    fn start(
        local: IndexStore,
        catalog: Box<dyn LibraryCatalog + Send>,
        thumbnails: Option<ThumbnailCache>,
    ) -> Result<Self, UiError> {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        let cancellation = Arc::new(AtomicU8::new(CANCEL_CLOSED));
        let worker_busy = Arc::clone(&busy);
        let worker_cancellation = Arc::clone(&cancellation);
        let worker = thread::Builder::new()
            .name("notae-index-worker".to_owned())
            .spawn(move || {
                run_worker(
                    local,
                    catalog,
                    command_rx,
                    event_tx,
                    worker_busy,
                    worker_cancellation,
                    thumbnails,
                );
            })
            .map_err(UiError::WorkerStart)?;
        Ok(Self {
            commands: command_tx,
            events: event_rx,
            busy,
            cancellation,
            worker: Some(worker),
        })
    }

    /// Requests a manual rescan. A second active operation is rejected.
    pub fn start_rescan(&self) -> Result<(), UiError> {
        self.begin_operation()?;
        self.cancellation.store(CANCEL_OPEN, Ordering::Release);
        self.commands.send(WorkerCommand::Rescan).map_err(|_| {
            self.busy.store(false, Ordering::Release);
            self.cancellation.store(CANCEL_CLOSED, Ordering::Release);
            UiError::WorkerStopped
        })
    }

    /// Requests a read-only preview of the supplied sidecar manifest.
    pub fn preview_import(&self, manifest: SidecarManifest) -> Result<(), UiError> {
        self.begin_operation()?;
        self.commands
            .send(WorkerCommand::PreviewImport(manifest))
            .map_err(|_| {
                self.busy.store(false, Ordering::Release);
                UiError::WorkerStopped
            })
    }

    /// Applies the currently displayed preview after an explicit user action.
    pub fn confirm_import(&self) -> Result<(), UiError> {
        self.begin_operation()?;
        self.commands
            .send(WorkerCommand::ConfirmImport)
            .map_err(|_| {
                self.busy.store(false, Ordering::Release);
                UiError::WorkerStopped
            })
    }

    /// Discards the pending read-only preview without changing the index.
    pub fn discard_import_preview(&self) -> Result<(), UiError> {
        self.commands
            .send(WorkerCommand::DiscardImport)
            .map_err(|_| UiError::WorkerStopped)
    }

    /// Stops a running scan before reconciliation commits its plan.
    pub fn cancel(&self) -> Result<(), UiError> {
        if !self.busy.load(Ordering::Acquire) {
            return Err(UiError::NoOperationInProgress);
        }
        match self.cancellation.compare_exchange(
            CANCEL_OPEN,
            CANCEL_REQUESTED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) | Err(CANCEL_REQUESTED) => Ok(()),
            Err(_) => Err(UiError::RescanNotCancellable),
        }
    }

    /// Re-runs the search with `scope` on the worker-owned index.
    pub fn set_search(&self, query: String, scope: SearchScope) -> Result<(), UiError> {
        self.commands
            .send(WorkerCommand::Search(query, scope))
            .map_err(|_| UiError::WorkerStopped)
    }

    /// Re-runs the search across every field.
    pub fn set_query(&self, query: String) -> Result<(), UiError> {
        self.set_search(query, SearchScope::Everything)
    }

    /// Loads an indexed note into the editor on the index-owning worker.
    pub fn load_metadata(&self, note_key: String) -> Result<(), UiError> {
        self.commands
            .send(WorkerCommand::LoadMetadata(note_key))
            .map_err(|_| UiError::WorkerStopped)
    }

    /// Saves edited metadata on the index-owning worker.
    pub fn save_metadata(&self, metadata: EditableMetadata) -> Result<(), UiError> {
        self.commands
            .send(WorkerCommand::SaveMetadata(metadata))
            .map_err(|_| UiError::WorkerStopped)
    }

    /// Receives one event without blocking the UI loop.
    pub fn try_event(&self) -> Option<OperationEvent> {
        self.events.try_recv().ok()
    }

    /// Receives one event with a bounded wait; useful to exercise the worker
    /// contract from host tests without polling or sleeping.
    pub fn recv_event_timeout(&self, timeout: Duration) -> Option<OperationEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    fn begin_operation(&self) -> Result<(), UiError> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| UiError::OperationAlreadyInProgress)
    }
}

impl Drop for OperationController {
    fn drop(&mut self) {
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(
    mut local: IndexStore,
    catalog: Box<dyn LibraryCatalog + Send>,
    commands: Receiver<WorkerCommand>,
    events: Sender<OperationEvent>,
    busy: Arc<AtomicBool>,
    cancellation: Arc<AtomicU8>,
    thumbnails: Option<ThumbnailCache>,
) {
    let mut query = String::new();
    let mut scope = SearchScope::default();
    let mut preview = ImportPreviewViewModel::default();
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Rescan => run_rescan(
                &mut local,
                catalog.as_ref(),
                &events,
                &busy,
                &cancellation,
                &query,
                scope,
                &mut preview,
                thumbnails.as_ref(),
            ),
            WorkerCommand::Search(next_query, next_scope) => {
                query = next_query;
                scope = next_scope;
                match project_rows(&local, &query, scope) {
                    Ok(rows) => send(&events, OperationEvent::SearchRows(rows)),
                    Err(error) => send(
                        &events,
                        OperationEvent::Failed {
                            reason: error.to_string(),
                        },
                    ),
                }
            }
            WorkerCommand::LoadMetadata(note_key) => {
                match MetadataEditor::new(&local).load(&note_key) {
                    Ok(metadata) => send(&events, OperationEvent::MetadataLoaded(metadata)),
                    Err(error) => send(
                        &events,
                        OperationEvent::MetadataFailed {
                            reason: error.to_string(),
                        },
                    ),
                }
            }
            WorkerCommand::SaveMetadata(metadata) => {
                match MetadataEditor::new(&local).save(metadata) {
                    Ok(()) => match project_rows(&local, &query, scope) {
                        Ok(rows) => send(&events, OperationEvent::MetadataSaved { rows }),
                        Err(error) => send(
                            &events,
                            OperationEvent::MetadataFailed {
                                reason: error.to_string(),
                            },
                        ),
                    },
                    Err(error) => send(
                        &events,
                        OperationEvent::MetadataFailed {
                            reason: error.to_string(),
                        },
                    ),
                }
            }
            WorkerCommand::PreviewImport(manifest) => {
                match SidecarImporter::preview(&local, &manifest, catalog.as_ref()) {
                    Ok(next_preview) => {
                        preview.load(next_preview);
                        if let Some(summary) = preview.summary() {
                            send(&events, OperationEvent::ImportPreviewReady { summary });
                        }
                    }
                    Err(error) => send(
                        &events,
                        OperationEvent::Failed {
                            reason: error.to_string(),
                        },
                    ),
                }
                busy.store(false, Ordering::Release);
            }
            WorkerCommand::ConfirmImport => {
                preview.confirm();
                match preview.apply(&local) {
                    Ok(report) => {
                        let summary = ImportSummary {
                            imported: report.imported().len(),
                            unchanged: report.unchanged().len(),
                            mismatches: report.mismatches().len(),
                            missing: report.absent_entries().len(),
                            unreadable: report.unreadable_entries().len(),
                            conflicts: report.conflicts().len(),
                            failures: report.failures().len(),
                        };
                        match project_rows(&local, &query, scope) {
                            Ok(rows) => {
                                send(&events, OperationEvent::ImportApplied { summary, rows })
                            }
                            Err(error) => send(
                                &events,
                                OperationEvent::Failed {
                                    reason: error.to_string(),
                                },
                            ),
                        }
                    }
                    Err(error) => send(
                        &events,
                        OperationEvent::Failed {
                            reason: error.to_string(),
                        },
                    ),
                }
                busy.store(false, Ordering::Release);
            }
            WorkerCommand::DiscardImport => preview = ImportPreviewViewModel::default(),
            WorkerCommand::Shutdown => break,
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "worker state is passed explicitly"
)]
fn run_rescan(
    local: &mut IndexStore,
    catalog: &dyn LibraryCatalog,
    events: &Sender<OperationEvent>,
    busy: &AtomicBool,
    cancellation: &AtomicU8,
    query: &str,
    scope: SearchScope,
    preview: &mut ImportPreviewViewModel,
    thumbnails: Option<&ThumbnailCache>,
) {
    send(events, OperationEvent::Started);
    if stop_if_cancelled(cancellation, events, busy) {
        return;
    }
    let files = match catalog.enumerate() {
        Ok(files) => files,
        Err(error) => {
            cancellation.store(CANCEL_CLOSED, Ordering::Release);
            send(
                events,
                OperationEvent::Failed {
                    reason: error.to_string(),
                },
            );
            busy.store(false, Ordering::Release);
            return;
        }
    };
    if stop_if_cancelled(cancellation, events, busy) {
        return;
    }

    let plan = match Reconciler::plan(local, &files) {
        Ok(plan) => plan,
        Err(error) => {
            cancellation.store(CANCEL_CLOSED, Ordering::Release);
            send(
                events,
                OperationEvent::Failed {
                    reason: error.to_string(),
                },
            );
            busy.store(false, Ordering::Release);
            return;
        }
    };
    if stop_if_cancelled(cancellation, events, busy) {
        return;
    }

    let failures: std::collections::HashMap<_, _> = plan
        .failures()
        .iter()
        .map(|failure| (failure.path().as_str().to_owned(), failure.to_string()))
        .collect();
    if cancellation
        .compare_exchange(
            CANCEL_OPEN,
            CANCEL_CLOSED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        if !stop_if_cancelled(cancellation, events, busy) {
            send(
                events,
                OperationEvent::Failed {
                    reason: "rescan cancellation state closed unexpectedly".to_owned(),
                },
            );
            busy.store(false, Ordering::Release);
        }
        return;
    }
    match Reconciler::apply(plan, local) {
        Ok(report) => {
            send(
                events,
                OperationEvent::RescanFinalizing { total: files.len() },
            );
            let mut summary = RescanSummary {
                added: report.additions().len(),
                updated: report.updates().len(),
                moved: report.moves().len(),
                deleted: report.deletions().len(),
                conflicts: report.conflicts().len(),
                failures: report.failures().len(),
            };
            for (index, file) in files.iter().enumerate() {
                let path = file.path().as_str().to_owned();
                let plan_failure = failures.get(&path).cloned();
                let failure = plan_failure.clone().or_else(|| {
                    match apply_pdf_text_layer(local, catalog, file) {
                        Ok(failure) => failure,
                        Err(error) => Some(error.to_string()),
                    }
                });
                if let (Some(cache), Some(fingerprint)) = (thumbnails, file.fingerprint()) {
                    let key = fingerprint.sha256().to_string();
                    if let Err(error) =
                        cache.ensure(&key, |out| catalog.render_thumbnail(file.path(), out))
                    {
                        eprintln!("notae: no thumbnail for {path}: {error}");
                    }
                }
                if let Some(reason) = failure {
                    if plan_failure.is_none() {
                        summary.failures += 1;
                    }
                    send(events, OperationEvent::FileFailed { path, reason });
                } else {
                    send(events, OperationEvent::FileSucceeded { path });
                }
                send(
                    events,
                    OperationEvent::Progress {
                        completed: index + 1,
                        total: files.len(),
                    },
                );
            }
            if let Some(cache) = thumbnails
                && let Ok(notes) = local.list_all()
            {
                let keep = notes
                    .iter()
                    .map(|note| note.fingerprint().sha256().to_string())
                    .collect();
                if let Err(error) = cache.prune(&keep) {
                    eprintln!("notae: cannot prune thumbnails: {error}");
                }
            }
            match project_rows(local, query, scope) {
                Ok(rows) => {
                    send(events, OperationEvent::Completed { summary, rows });
                    match catalog.sidecar_manifest() {
                        Ok(Some(manifest)) => {
                            match SidecarImporter::preview(local, &manifest, catalog) {
                                Ok(sidecar_preview) => {
                                    preview.load(sidecar_preview);
                                    if let Some(summary) = preview.summary() {
                                        send(
                                            events,
                                            OperationEvent::ImportPreviewReady { summary },
                                        );
                                    }
                                }
                                Err(error) => send(
                                    events,
                                    OperationEvent::Failed {
                                        reason: error.to_string(),
                                    },
                                ),
                            }
                        }
                        Ok(None) => {}
                        Err(error) => send(
                            events,
                            OperationEvent::Failed {
                                reason: error.to_string(),
                            },
                        ),
                    }
                }
                Err(error) => send(
                    events,
                    OperationEvent::Failed {
                        reason: error.to_string(),
                    },
                ),
            }
        }
        Err(error) => send(
            events,
            OperationEvent::Failed {
                reason: error.to_string(),
            },
        ),
    }
    busy.store(false, Ordering::Release);
}

fn stop_if_cancelled(
    cancellation: &AtomicU8,
    events: &Sender<OperationEvent>,
    busy: &AtomicBool,
) -> bool {
    if cancellation.load(Ordering::Acquire) != CANCEL_REQUESTED {
        return false;
    }
    cancellation.store(CANCEL_CLOSED, Ordering::Release);
    send(events, OperationEvent::Canceled);
    busy.store(false, Ordering::Release);
    true
}

fn apply_pdf_text_layer(
    local: &IndexStore,
    catalog: &dyn LibraryCatalog,
    file: &PdfFile,
) -> Result<Option<String>, UiError> {
    let Some(fingerprint) = file.fingerprint() else {
        return Ok(None);
    };
    let hash = fingerprint.sha256();
    let Some(note) = local.get_by_hash(&hash)? else {
        return Ok(None);
    };
    if note.status() == IndexStatus::DuplicateIdConflict {
        return Ok(None);
    }

    let existing_text = local.ocr_text(&hash)?;
    let has_existing_searchable_text = existing_text
        .as_ref()
        .is_some_and(|text| !text.text().is_empty());
    let outcome = crate::pdf_pages::classify_text_layer(
        catalog.extract_text_layer(file.path()),
        has_existing_searchable_text,
    );
    let updated = note.with_status(outcome.status);
    if let Some(text) = outcome.text {
        local.reindex_note_with_ocr(&updated, Some(&OcrText::new(text)))?;
    } else {
        local.reindex_note(&updated)?;
    }
    Ok(outcome.failure.map(|error| error.to_string()))
}

fn project_rows(
    local: &IndexStore,
    query: &str,
    scope: SearchScope,
) -> Result<Vec<SearchRow>, UiError> {
    let mut view_model = LibraryViewModel::new(local);
    view_model.set_query(query.to_owned());
    view_model.set_scope(scope);
    view_model.search()
}

fn send(events: &Sender<OperationEvent>, event: OperationEvent) {
    let _ = events.send(event);
}
