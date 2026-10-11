use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notae_domain::IndexStatus;
use notae_index::{IndexStore, OcrText, PdfFile, Reconciler, SidecarImporter, SidecarManifest};

use crate::operations::events::{ImportSummary, OperationEvent, RescanSummary};
use crate::operations::import::ImportPreviewViewModel;
use crate::storage::LibraryCatalog;
use crate::ui::library::LibraryViewModel;
use crate::ui::metadata_editor::{EditableMetadata, MetadataEditor};
use crate::ui::search::{SearchRow, UiError};

/// Owns the worker that serializes all index access away from the UI thread.
pub struct OperationController {
    commands: Sender<WorkerCommand>,
    events: Receiver<OperationEvent>,
    busy: Arc<AtomicBool>,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

enum WorkerCommand {
    Rescan,
    Search(String),
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
    #[must_use]
    pub fn new(local: IndexStore, catalog: Box<dyn LibraryCatalog + Send>) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        let worker_busy = Arc::clone(&busy);
        let worker_canceled = Arc::clone(&canceled);
        let worker = thread::Builder::new()
            .name("notae-index-worker".to_owned())
            .spawn(move || {
                run_worker(
                    local,
                    catalog,
                    command_rx,
                    event_tx,
                    worker_busy,
                    worker_canceled,
                );
            })
            .expect("start Notae index worker");
        Self {
            commands: command_tx,
            events: event_rx,
            busy,
            canceled,
            worker: Some(worker),
        }
    }

    /// Requests a manual rescan. A second active operation is rejected.
    pub fn start_rescan(&self) -> Result<(), UiError> {
        self.begin_operation()?;
        self.canceled.store(false, Ordering::Release);
        self.commands.send(WorkerCommand::Rescan).map_err(|_| {
            self.busy.store(false, Ordering::Release);
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

    /// Stops a running scan before it writes its reconciliation plan.
    pub fn cancel(&self) -> Result<(), UiError> {
        if !self.busy.load(Ordering::Acquire) {
            return Err(UiError::NoOperationInProgress);
        }
        self.canceled.store(true, Ordering::Release);
        Ok(())
    }

    /// Re-runs the core-ranked search on the worker-owned index.
    pub fn set_query(&self, query: String) -> Result<(), UiError> {
        self.commands
            .send(WorkerCommand::Search(query))
            .map_err(|_| UiError::WorkerStopped)
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
    canceled: Arc<AtomicBool>,
) {
    let mut query = String::new();
    let mut preview = ImportPreviewViewModel::default();
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Rescan => run_rescan(
                &mut local,
                catalog.as_ref(),
                &events,
                &busy,
                &canceled,
                &query,
                &mut preview,
            ),
            WorkerCommand::Search(next_query) => {
                query = next_query;
                match project_rows(&local, &query) {
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
                    Ok(()) => match project_rows(&local, &query) {
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
                        match project_rows(&local, &query) {
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

fn run_rescan(
    local: &mut IndexStore,
    catalog: &dyn LibraryCatalog,
    events: &Sender<OperationEvent>,
    busy: &AtomicBool,
    canceled: &AtomicBool,
    query: &str,
    preview: &mut ImportPreviewViewModel,
) {
    send(events, OperationEvent::Started);
    let files = match catalog.enumerate() {
        Ok(files) => files,
        Err(error) => {
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
    if canceled.load(Ordering::Acquire) {
        send(events, OperationEvent::Canceled);
        busy.store(false, Ordering::Release);
        return;
    }

    let plan = match Reconciler::plan(local, &files) {
        Ok(plan) => plan,
        Err(error) => {
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
    if canceled.load(Ordering::Acquire) {
        send(events, OperationEvent::Canceled);
        busy.store(false, Ordering::Release);
        return;
    }

    let failures: std::collections::HashMap<_, _> = plan
        .failures()
        .iter()
        .map(|failure| (failure.path().as_str().to_owned(), failure.to_string()))
        .collect();
    match Reconciler::apply(plan, local) {
        Ok(report) => {
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
            match project_rows(local, query) {
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

fn project_rows(local: &IndexStore, query: &str) -> Result<Vec<SearchRow>, UiError> {
    let mut view_model = LibraryViewModel::new(local);
    view_model.set_query(query.to_owned());
    view_model.search()
}

fn send(events: &Sender<OperationEvent>, event: OperationEvent) {
    let _ = events.send(event);
}
