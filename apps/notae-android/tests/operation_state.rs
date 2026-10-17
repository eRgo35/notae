use notae_android::operations::events::{DesktopImportCheck, OperationEvent};
use notae_android::operations::import::ImportPreviewViewModel;
use notae_android::operations::rescan::OperationController;
use notae_android::state::RescanPhase;
use notae_domain::{
    IndexedNote, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath,
    Sha256Digest,
};
use notae_index::{
    CatalogError, FileCondition, IndexStore, PdfCatalog, PdfFile, SidecarEntry, SidecarManifest,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;

use notae_android::storage::LibraryCatalog;

struct StaticCatalog {
    files: Vec<PdfFile>,
    entered: Option<mpsc::Sender<()>>,
    release: Option<Arc<(Mutex<bool>, Condvar)>>,
}

impl PdfCatalog for StaticCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        if let Some(entered) = &self.entered {
            entered.send(()).expect("test receiver is alive");
        }
        if let Some(release) = &self.release {
            let (lock, ready) = &**release;
            let mut released = lock.lock().expect("release lock is healthy");
            while !*released {
                released = ready.wait(released).expect("release lock is healthy");
            }
        }
        Ok(self.files.clone())
    }
}

impl LibraryCatalog for StaticCatalog {}

struct SidecarCatalog {
    files: Vec<PdfFile>,
    manifest: SidecarManifest,
}

impl PdfCatalog for SidecarCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(self.files.clone())
    }
}

impl LibraryCatalog for SidecarCatalog {
    fn sidecar_manifest(&self) -> Result<Option<SidecarManifest>, CatalogError> {
        Ok(Some(self.manifest.clone()))
    }
}

fn readable(path: &str, bytes: &[u8]) -> PdfFile {
    PdfFile::new(
        RelativePdfPath::new(path).expect("valid library-relative path"),
        PdfFingerprint::new(
            Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
            u64::try_from(bytes.len()).expect("fixture fits in u64"),
            None,
        ),
        path.rsplit('/')
            .next()
            .expect("file name exists")
            .to_owned(),
    )
}

fn unreadable(path: &str) -> PdfFile {
    PdfFile::with_condition(
        RelativePdfPath::new(path).expect("valid library-relative path"),
        path.to_owned(),
        FileCondition::Unreadable,
    )
}

fn next_event(controller: &OperationController) -> OperationEvent {
    controller
        .recv_event_timeout(Duration::from_secs(2))
        .expect("operation event arrives")
}

#[test]
fn rescan_starts_only_on_request_and_reports_success_and_per_file_failure() {
    let files = vec![
        readable("12 - Autumn Leaves.pdf", b"%PDF-1.4\nvalid"),
        unreadable("locked.pdf"),
    ];
    let controller = OperationController::new(
        IndexStore::open(":memory:").expect("open test index"),
        Box::new(StaticCatalog {
            files,
            entered: None,
            release: None,
        }),
    )
    .expect("start test worker");

    assert!(
        controller
            .recv_event_timeout(Duration::from_millis(25))
            .is_none(),
        "construction must not trigger a rescan"
    );
    controller
        .start_rescan()
        .expect("explicit request starts scan");

    assert!(matches!(next_event(&controller), OperationEvent::Started));
    let mut completed_files = Vec::new();
    let mut failures = Vec::new();
    let mut terminal = None;
    while terminal.is_none() {
        match next_event(&controller) {
            OperationEvent::FileSucceeded { path } => completed_files.push(path),
            OperationEvent::FileFailed { path, .. } => failures.push(path),
            event @ OperationEvent::Completed { .. } => terminal = Some(event),
            _ => {}
        }
    }

    assert_eq!(completed_files, ["12 - Autumn Leaves.pdf"]);
    assert_eq!(failures, ["locked.pdf"]);
    assert!(matches!(terminal, Some(OperationEvent::Completed { .. })));
}

#[test]
fn cancellation_during_catalog_enumeration_does_not_apply_a_partial_scan() {
    let dir = tempfile::tempdir().expect("temporary database directory");
    let database = dir.path().join("index.sqlite");
    drop(IndexStore::open(&database).expect("create test index"));
    let (entered_tx, entered_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let controller = OperationController::new(
        IndexStore::open(&database).expect("open test index"),
        Box::new(StaticCatalog {
            files: vec![readable("12 - Autumn Leaves.pdf", b"%PDF-1.4\nvalid")],
            entered: Some(entered_tx),
            release: Some(Arc::clone(&release)),
        }),
    )
    .expect("start test worker");

    controller
        .start_rescan()
        .expect("explicit request starts scan");
    assert!(matches!(next_event(&controller), OperationEvent::Started));
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("worker entered catalog");
    controller
        .cancel()
        .expect("active scan accepts cancellation");
    {
        let (lock, ready) = &*release;
        *lock.lock().expect("release lock is healthy") = true;
        ready.notify_all();
    }
    assert!(matches!(next_event(&controller), OperationEvent::Canceled));

    let check = IndexStore::open(&database).expect("reopen index");
    assert!(check.list_all().expect("read index").is_empty());
}

#[test]
fn import_preview_requires_explicit_confirmation_before_writing() {
    let store = IndexStore::open(":memory:").expect("open test index");
    let bytes = b"%PDF-1.4\nvalid";
    let fingerprint = PdfFingerprint::new(
        Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
        u64::try_from(bytes.len()).expect("fixture fits in u64"),
        None,
    );
    let local_note = notae_domain::IndexedNote::new(
        RelativePdfPath::new("12 - Autumn Leaves.pdf").expect("valid path"),
        fingerprint.clone(),
        notae_domain::NoteMetadata::new("Local title", None, []).expect("valid metadata"),
    );
    let sidecar_note = notae_domain::IndexedNote::new(
        RelativePdfPath::new("12 - Autumn Leaves.pdf").expect("valid path"),
        fingerprint,
        notae_domain::NoteMetadata::new("PC title", None, []).expect("valid metadata"),
    );
    store
        .reindex_note(&local_note)
        .expect("insert local record");
    let manifest = notae_index::SidecarManifest::new(
        notae_domain::EpochTimestamp::new(0, 0).expect("valid timestamp"),
        "test",
        vec![notae_index::SidecarEntry::new(&sidecar_note, None)],
    );
    let encoded = manifest.to_json().expect("serialize sidecar fixture");
    let parsed = notae_index::SidecarManifest::from_json(&encoded).expect("parse sidecar bytes");
    assert_eq!(parsed.entries().len(), 1);
    let catalog = StaticCatalog {
        files: vec![readable("12 - Autumn Leaves.pdf", b"%PDF-1.4\nvalid")],
        entered: None,
        release: None,
    };
    let preview = notae_index::SidecarImporter::preview(&store, &manifest, &catalog)
        .expect("sidecar can be previewed");
    let mut view_model = ImportPreviewViewModel::default();
    view_model.load(preview);
    let summary = view_model.summary().expect("preview summary");
    assert_eq!(summary.changed, 1);
    assert_eq!(summary.rows[0].path, "12 - Autumn Leaves.pdf");
    assert_eq!(summary.rows[0].local_title.as_deref(), Some("Local title"));
    assert_eq!(summary.rows[0].proposed_title, "PC title");

    assert!(
        view_model.apply(&store).is_err(),
        "preview must not write before confirmation"
    );
    assert_eq!(
        store.list_all().expect("read local index")[0].title(),
        "Local title"
    );

    view_model.confirm();
    view_model.apply(&store).expect("confirmed preview applies");
    assert_eq!(
        store.list_all().expect("read local index")[0].title(),
        "PC title"
    );
}

#[test]
fn worker_progress_and_completion_are_reflected_in_ui_state_and_rows() {
    use notae_android::operations::events::RescanSummary;
    use notae_android::state::{OperationState, Transition, UiEvent};
    use notae_android::ui::UiModel;
    use notae_android::ui::search::SearchRow;

    let mut model = UiModel::new();
    assert_eq!(
        model.dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/library"),
        }),
        Ok(Transition::BeginRescan)
    );
    model.start_rescan().expect("request scan");
    model
        .apply_operation_event(OperationEvent::Started)
        .expect("start event");
    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 5 })
        .expect("finalizing event");
    model
        .apply_operation_event(OperationEvent::Progress {
            completed: 2,
            total: 5,
        })
        .expect("progress event");
    assert_eq!(
        model.state().operation(),
        &OperationState::Rescan {
            phase: RescanPhase::ReadingText {
                completed: 2,
                total: 5,
            },
        }
    );

    let rows = vec![SearchRow {
        id: Some("12".to_owned()),
        title: "Autumn Leaves".to_owned(),
        composer_or_artist: Some("Joseph Kosma".to_owned()),
        tags: vec![],
        status: "Ready".to_owned(),
        note_key: "stable-key".to_owned(),
        relative_path: "12 - Autumn Leaves.pdf".to_owned(),
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
    }];
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: RescanSummary {
                added: 1,
                ..RescanSummary::default()
            },
            rows: rows.clone(),
            desktop_import: DesktopImportCheck::NoSidecar,
        })
        .expect("completion event");
    assert_eq!(model.search_rows(), rows);
    assert_eq!(
        model.state().operation(),
        &OperationState::Completed {
            summary: "1 added".to_owned(),
        }
    );
}
#[test]
fn ui_status_projects_operation_scan_summary_and_pending_rows() {
    use notae_android::operations::events::RescanSummary;
    use notae_android::state::{OperationState, UiEvent};
    use notae_android::ui::UiModel;
    use notae_android::ui::search::SearchRow;

    let mut model = UiModel::new();
    let pending_row = SearchRow {
        id: None,
        title: "Pending".to_owned(),
        composer_or_artist: None,
        tags: vec![],
        status: "OCR pending".to_owned(),
        note_key: "pending".to_owned(),
        relative_path: "pending.pdf".to_owned(),
        text_pending: true,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
    };
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![pending_row.clone()]))
        .expect("rows event");
    let status = model.status();
    assert_eq!(status.latest_rescan, None);
    assert_eq!(status.indexed_count, 1);
    assert_eq!(status.pending_text_count, 1);

    model
        .dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/library"),
        })
        .expect("choose library");
    model.start_rescan().expect("request scan");
    model
        .apply_operation_event(OperationEvent::Started)
        .expect("start");
    assert!(matches!(
        model.status().operation,
        OperationState::Rescan {
            phase: RescanPhase::Enumerating
        }
    ));
    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 1 })
        .expect("finalizing");
    assert_eq!(
        model.status().operation,
        OperationState::Rescan {
            phase: RescanPhase::ReadingText {
                completed: 0,
                total: 1
            }
        }
    );

    let summary = RescanSummary {
        added: 2,
        conflicts: 1,
        failures: 1,
        ..RescanSummary::default()
    };
    model
        .apply_operation_event(OperationEvent::Completed {
            summary,
            rows: vec![pending_row],
            desktop_import: DesktopImportCheck::NoSidecar,
        })
        .expect("complete scan");
    assert_eq!(
        model.status().operation,
        OperationState::Completed {
            summary: "2 added, 1 ID conflicts, 1 failures".to_owned(),
        }
    );
    assert_eq!(model.status().latest_rescan, Some(summary));
    assert_eq!(model.status().pending_text_count, 1);

    model
        .apply_operation_event(OperationEvent::ImportPreviewReady {
            summary: Default::default(),
        })
        .expect("import preview");
    model
        .apply_operation_event(OperationEvent::ImportApplied {
            summary: Default::default(),
            rows: vec![],
        })
        .expect("apply import");
    assert_eq!(model.status().latest_rescan, Some(summary));

    model
        .dispatch(UiEvent::AcknowledgeOperation)
        .expect("dismiss");
    model.start_rescan().expect("request second scan");
    model
        .apply_operation_event(OperationEvent::Started)
        .expect("start second");
    model
        .apply_operation_event(OperationEvent::Canceled)
        .expect("cancel second scan");
    assert_eq!(model.status().operation, OperationState::Idle);
    assert_eq!(model.status().latest_rescan, Some(summary));

    model.start_rescan().expect("request failed scan");
    model
        .apply_operation_event(OperationEvent::Started)
        .expect("start failed");
    model
        .apply_operation_event(OperationEvent::Failed {
            reason: "scan failed".to_owned(),
        })
        .expect("fail scan");
    assert_eq!(
        model.status().operation,
        OperationState::Failed {
            reason: "scan failed".to_owned(),
        }
    );
    assert_eq!(model.status().latest_rescan, Some(summary));
}
#[test]
fn settings_indexed_count_is_not_replaced_by_filtered_search_rows() {
    use notae_android::state::UiEvent;
    use notae_android::ui::UiModel;
    use notae_android::ui::search::SearchRow;

    let row = SearchRow {
        id: None,
        title: "Score".to_owned(),
        composer_or_artist: None,
        tags: vec![],
        status: "Ready".to_owned(),
        note_key: "score".to_owned(),
        relative_path: "score.pdf".to_owned(),
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
    };
    let mut model = UiModel::new();
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![row.clone(), row.clone()]))
        .expect("full library rows");
    model
        .dispatch(UiEvent::UpdateSearchQuery {
            query: "filtered".to_owned(),
        })
        .expect("update query");
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![row]))
        .expect("filtered search rows");

    assert_eq!(model.status().indexed_count, 2);
}

#[test]
fn rescan_previews_root_sidecar_hash_mismatch_without_importing_it() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let database = directory.path().join("index.sqlite");
    drop(IndexStore::open(&database).expect("create test index"));
    let path = "12 - Autumn Leaves.pdf";
    let stale_fingerprint = PdfFingerprint::new(
        Sha256Digest::from_bytes(Sha256::digest(b"%PDF-stale").into()),
        u64::try_from(b"%PDF-stale".len()).expect("fixture fits in u64"),
        None,
    );
    let sidecar_note = notae_domain::IndexedNote::new(
        RelativePdfPath::new(path).expect("valid path"),
        stale_fingerprint,
        notae_domain::NoteMetadata::new("PC-only title", None, []).expect("valid metadata"),
    );
    let manifest = SidecarManifest::new(
        notae_domain::EpochTimestamp::new(0, 0).expect("valid timestamp"),
        "test",
        vec![SidecarEntry::new(&sidecar_note, None)],
    );
    let controller = OperationController::new(
        IndexStore::open(&database).expect("open worker index"),
        Box::new(SidecarCatalog {
            files: vec![readable(path, b"%PDF-current")],
            manifest,
        }),
    )
    .expect("start test worker");

    controller
        .start_rescan()
        .expect("explicit request starts scan");
    let summary = loop {
        match next_event(&controller) {
            OperationEvent::Completed {
                desktop_import: DesktopImportCheck::PreviewReady(summary),
                ..
            } => break summary,
            OperationEvent::Failed { reason } => panic!("worker failed: {reason}"),
            _ => {}
        }
    };
    assert_eq!(summary.mismatches, 1);
    assert_eq!(summary.rows[0].proposed_title, "PC-only title");

    let observer = IndexStore::open(&database).expect("observe worker index");
    let indexed = observer.list_all().expect("read indexed notes");
    assert_eq!(indexed.len(), 1);
    assert_ne!(indexed[0].title(), "PC-only title");
}

#[test]
fn cancellation_returns_ui_to_idle_and_keeps_the_library_screen() {
    use notae_android::state::{OperationState, UiEvent};
    use notae_android::ui::UiModel;

    let mut model = UiModel::new();
    model
        .dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/library"),
        })
        .expect("configure library");
    model.start_rescan().expect("request scan");
    model
        .apply_operation_event(OperationEvent::Canceled)
        .expect("cancel event");
    assert_eq!(model.state().operation(), &OperationState::Idle);
    assert_eq!(
        model.state().screen(),
        &notae_android::state::Screen::Library
    );
}

#[test]
fn worker_loads_edits_and_returns_the_updated_library_row() {
    use notae_android::operations::events::OperationEvent;
    use notae_android::ui::metadata_editor::EditableMetadata;

    let bytes = b"%PDF-1.4\nvalid";
    let mut metadata = NoteMetadata::new(
        "Autumn Leaves",
        Some("Joseph Kosma".to_owned()),
        ["standard".to_owned()],
    )
    .expect("valid initial metadata");
    metadata.assign_id(Some(NoteId::new(12)), MetadataOrigin::FilenameDerived);
    let note = IndexedNote::new(
        RelativePdfPath::new("12 - Autumn Leaves.pdf").expect("valid path"),
        PdfFingerprint::new(
            Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
            u64::try_from(bytes.len()).expect("fixture fits in u64"),
            None,
        ),
        metadata,
    );
    let note_key = note.fingerprint().sha256().to_string();
    let store = IndexStore::open(":memory:").expect("open test index");
    store.reindex_note(&note).expect("insert test note");
    let controller = OperationController::new(
        store,
        Box::new(StaticCatalog {
            files: Vec::new(),
            entered: None,
            release: None,
        }),
    )
    .expect("start test worker");

    controller
        .load_metadata(note_key.clone())
        .expect("request editor values");
    let loaded = match next_event(&controller) {
        OperationEvent::MetadataLoaded(metadata) => metadata,
        other => panic!("unexpected load event: {other:?}"),
    };
    assert_eq!(loaded.title, "Autumn Leaves");
    assert_eq!(loaded.id, "12");
    assert_eq!(loaded.note_key, note_key);

    controller
        .save_metadata(EditableMetadata {
            note_key,
            id: "14".to_owned(),
            title: "Edited Leaves".to_owned(),
            composer_or_artist: "Joseph Kosma".to_owned(),
            tags: vec!["jazz".to_owned()],
        })
        .expect("submit edited metadata");
    let rows = match next_event(&controller) {
        OperationEvent::MetadataSaved { rows } => rows,
        other => panic!("unexpected save event: {other:?}"),
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id.as_deref(), Some("14"));
    assert_eq!(rows[0].title, "Edited Leaves");
    assert_eq!(rows[0].tags, ["jazz"]);
    assert_eq!(rows[0].note_key, note.fingerprint().sha256().to_string());
}

#[test]
fn metadata_events_keep_errors_visible_and_close_after_success() {
    use notae_android::operations::events::OperationEvent;
    use notae_android::ui::UiModel;
    use notae_android::ui::metadata_editor::EditableMetadata;
    use notae_android::ui::search::SearchRow;

    let mut model = UiModel::new();
    model.begin_metadata_edit("content-key".to_owned());
    model
        .apply_operation_event(OperationEvent::MetadataLoaded(EditableMetadata {
            note_key: "content-key".to_owned(),
            id: "12".to_owned(),
            title: "Autumn Leaves".to_owned(),
            composer_or_artist: "Joseph Kosma".to_owned(),
            tags: vec!["standard".to_owned()],
        }))
        .expect("load editor data");
    assert_eq!(
        model
            .metadata_editor()
            .map(|metadata| metadata.title.as_str()),
        Some("Autumn Leaves")
    );

    model
        .apply_operation_event(OperationEvent::MetadataFailed {
            reason: "duplicate note ID".to_owned(),
        })
        .expect("retain save error in editor");
    assert_eq!(model.metadata_error(), Some("duplicate note ID"));
    assert!(model.metadata_editor().is_some());

    model
        .apply_operation_event(OperationEvent::MetadataSaved {
            rows: vec![SearchRow {
                id: Some("14".to_owned()),
                title: "Edited Leaves".to_owned(),
                composer_or_artist: Some("Joseph Kosma".to_owned()),
                tags: vec!["jazz".to_owned()],
                status: "Ready".to_owned(),
                note_key: "content-key".to_owned(),
                relative_path: "12 - Autumn Leaves.pdf".to_owned(),
                text_pending: false,
                title_origin: String::new(),
                composer_origin: String::new(),
                text_snippet: None,
            }],
        })
        .expect("apply saved row projection");
    assert!(model.metadata_editor().is_none());
    assert!(model.metadata_error().is_none());
    assert_eq!(model.search_rows()[0].title, "Edited Leaves");
}

#[test]
fn rescan_finalization_has_progress_state_after_cancel_is_closed() {
    use notae_android::state::{OperationState, Transition, UiEvent};
    use notae_android::ui::UiModel;

    let mut model = UiModel::new();
    assert_eq!(
        model.dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/library"),
        }),
        Ok(Transition::BeginRescan)
    );
    model.start_rescan().expect("request scan");
    model
        .apply_operation_event(OperationEvent::Started)
        .expect("start event");
    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 3 })
        .expect("finalization event");
    model
        .apply_operation_event(OperationEvent::Progress {
            completed: 2,
            total: 3,
        })
        .expect("finalization progress");

    assert_eq!(
        model.state().operation(),
        &OperationState::Rescan {
            phase: RescanPhase::ReadingText {
                completed: 2,
                total: 3,
            },
        }
    );
}

/// How the gated catalog answers its sidecar lookup.
enum SidecarAnswer {
    Absent,
    Stale(&'static str),
    Unreadable,
}

/// Catalog whose sidecar lookup blocks until the test releases it, so the test can
/// observe exactly which events the worker has published while that work is outstanding.
struct GatedSidecarCatalog {
    files: Vec<PdfFile>,
    answer: SidecarAnswer,
    entered: mpsc::Sender<()>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

impl PdfCatalog for GatedSidecarCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(self.files.clone())
    }
}

impl LibraryCatalog for GatedSidecarCatalog {
    fn sidecar_manifest(&self) -> Result<Option<SidecarManifest>, CatalogError> {
        self.entered.send(()).expect("test receiver is alive");
        let (lock, ready) = &*self.release;
        let mut released = lock.lock().expect("release lock is healthy");
        while !*released {
            released = ready.wait(released).expect("release lock is healthy");
        }
        match self.answer {
            SidecarAnswer::Absent => Ok(None),
            SidecarAnswer::Unreadable => Err(CatalogError::Failed("sidecar unreadable".to_owned())),
            SidecarAnswer::Stale(path) => {
                let stale = PdfFingerprint::new(
                    Sha256Digest::from_bytes(Sha256::digest(b"%PDF-stale").into()),
                    u64::try_from(b"%PDF-stale".len()).expect("fixture fits in u64"),
                    None,
                );
                let note = IndexedNote::new(
                    RelativePdfPath::new(path).expect("valid path"),
                    stale,
                    NoteMetadata::new("PC-only title", None, []).expect("valid metadata"),
                );
                Ok(Some(SidecarManifest::new(
                    notae_domain::EpochTimestamp::new(0, 0).expect("valid timestamp"),
                    "test",
                    vec![SidecarEntry::new(&note, None)],
                )))
            }
        }
    }
}

type Gate = Arc<(Mutex<bool>, Condvar)>;

fn gated_sidecar_fixture(
    files: Vec<PdfFile>,
    answer: SidecarAnswer,
) -> (OperationController, mpsc::Receiver<()>, Gate) {
    let (entered_tx, entered_rx) = mpsc::channel();
    let release: Gate = Arc::new((Mutex::new(false), Condvar::new()));
    let controller = OperationController::new(
        IndexStore::open(":memory:").expect("open test index"),
        Box::new(GatedSidecarCatalog {
            files,
            answer,
            entered: entered_tx,
            release: Arc::clone(&release),
        }),
    )
    .expect("start test worker");
    (controller, entered_rx, release)
}

fn release_gate(release: &Gate) {
    let (lock, ready) = &**release;
    *lock.lock().expect("release lock is healthy") = true;
    ready.notify_all();
}

/// Runs a scan to its terminal event, holding the sidecar check open once to capture what
/// was published while it was outstanding. Returns `(before_release, after_release)`.
fn scan_with_gated_sidecar(
    files: Vec<PdfFile>,
    answer: SidecarAnswer,
) -> (Vec<OperationEvent>, Vec<OperationEvent>) {
    let (controller, entered, release) = gated_sidecar_fixture(files, answer);
    controller.start_rescan().expect("start scan");
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("worker reached the sidecar check");
    let mut before = Vec::new();
    while let Some(event) = controller.try_event() {
        before.push(event);
    }
    release_gate(&release);
    let mut after = Vec::new();
    loop {
        let event = next_event(&controller);
        let terminal = matches!(
            event,
            OperationEvent::Completed { .. } | OperationEvent::Failed { .. }
        );
        after.push(event);
        if terminal {
            break;
        }
    }
    (before, after)
}

fn names(events: &[OperationEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            OperationEvent::Started => "started",
            OperationEvent::Indexing { .. } => "indexing",
            OperationEvent::RescanFinalizing { .. } => "reading-text",
            OperationEvent::Progress { .. } => "progress",
            OperationEvent::FileSucceeded { .. } | OperationEvent::FileFailed { .. } => "file",
            OperationEvent::CheckingDesktopImport => "checking-import",
            OperationEvent::Completed { .. } => "completed",
            OperationEvent::Failed { .. } => "failed",
            _ => "other",
        })
        .collect()
}

#[test]
fn completion_is_not_reported_while_the_sidecar_check_is_outstanding() {
    let path = "12 - Autumn Leaves.pdf";
    let (before, after) = scan_with_gated_sidecar(
        vec![readable(path, b"%PDF-current")],
        SidecarAnswer::Stale(path),
    );
    // Text and thumbnail work is finished and the sidecar check is announced, but the scan
    // is not reported complete until the check returns.
    assert_eq!(
        names(&before),
        [
            "started",
            "indexing",
            "reading-text",
            "file",
            "progress",
            "checking-import"
        ]
    );
    // Completed is the last event and carries the preview, so nothing can overwrite it.
    assert_eq!(names(&after), ["completed"]);
    match &after[0] {
        OperationEvent::Completed {
            desktop_import: DesktopImportCheck::PreviewReady(summary),
            summary: scan,
            ..
        } => {
            assert_eq!(summary.mismatches, 1);
            assert_eq!(scan.added, 1);
        }
        other => panic!("expected Completed with a ready preview, got {other:?}"),
    }
}

#[test]
fn scan_without_sidecar_still_announces_the_check_before_completing() {
    let (before, after) =
        scan_with_gated_sidecar(vec![readable("a.pdf", b"%PDF-a")], SidecarAnswer::Absent);
    assert_eq!(names(&before).last(), Some(&"checking-import"));
    assert!(matches!(
        after.as_slice(),
        [OperationEvent::Completed {
            desktop_import: DesktopImportCheck::NoSidecar,
            ..
        }]
    ));
}

#[test]
fn empty_library_skips_the_per_file_phase() {
    let (before, after) = scan_with_gated_sidecar(Vec::new(), SidecarAnswer::Absent);
    assert_eq!(
        names(&before),
        ["started", "indexing", "checking-import"],
        "no per-file phase or progress for zero files"
    );
    assert_eq!(names(&after), ["completed"]);
}

#[test]
fn sidecar_failure_keeps_the_committed_scan_counts_and_names_the_phase() {
    let (_, after) = scan_with_gated_sidecar(
        vec![readable("a.pdf", b"%PDF-a")],
        SidecarAnswer::Unreadable,
    );
    let Some(OperationEvent::Completed {
        summary,
        desktop_import: DesktopImportCheck::Failed { reason },
        ..
    }) = after.into_iter().next()
    else {
        panic!("a sidecar failure after a committed scan completes with the failed check");
    };
    assert_eq!(summary.added, 1);
    assert!(reason.contains("sidecar unreadable"), "{reason}");
}

fn scanning_model() -> notae_android::ui::UiModel {
    use notae_android::state::UiEvent;
    let mut model = notae_android::ui::UiModel::new();
    model
        .dispatch(UiEvent::PickLibraryRoot {
            root: std::path::PathBuf::from("/library"),
        })
        .expect("choose library");
    model.start_rescan().expect("request scan");
    model
        .apply_operation_event(OperationEvent::Started)
        .expect("start event");
    model
}

fn banner(model: &notae_android::ui::UiModel) -> notae_android::ui::OperationBanner {
    notae_android::ui::OperationBanner::project(model.state().operation())
}

#[test]
fn banner_follows_each_real_phase_in_order() {
    let mut model = scanning_model();
    let enumerating = banner(&model);
    assert_eq!(enumerating.text, "Scanning library…");
    assert!(enumerating.busy && enumerating.cancel_enabled);
    assert_eq!(enumerating.fraction, 0.0, "enumeration has no total");

    model
        .apply_operation_event(OperationEvent::Indexing { total: 3 })
        .expect("indexing");
    let indexing = banner(&model);
    assert_eq!(indexing.text, "Indexing 3 PDFs…");
    assert!(indexing.busy && !indexing.cancel_enabled);
    assert_eq!(
        indexing.fraction, 0.0,
        "reconciliation reports no per-file progress"
    );

    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 3 })
        .expect("reading text");
    assert_eq!(
        banner(&model).text,
        "Reading PDF text and thumbnails: 0 of 3"
    );
    model
        .apply_operation_event(OperationEvent::Progress {
            completed: 1,
            total: 3,
        })
        .expect("progress");
    let reading = banner(&model);
    assert_eq!(reading.text, "Reading PDF text and thumbnails: 1 of 3");
    assert!((reading.fraction - 1.0 / 3.0).abs() < f32::EPSILON);
    assert!(reading.busy && !reading.cancel_enabled);

    model
        .apply_operation_event(OperationEvent::CheckingDesktopImport)
        .expect("checking import");
    let checking = banner(&model);
    assert_eq!(checking.text, "Checking for a desktop sidecar…");
    assert!(checking.busy && !checking.cancel_enabled);
    assert_eq!(
        checking.fraction, 0.0,
        "no entry-level progress is exposed for the sidecar check"
    );
}

#[test]
fn zero_files_never_show_a_zero_of_zero_phase() {
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Indexing { total: 0 })
        .expect("indexing");
    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 0 })
        .expect("finalizing an empty library");
    assert!(!banner(&model).text.contains("0 of 0"));
    assert!(
        model
            .apply_operation_event(OperationEvent::Progress {
                completed: 0,
                total: 0,
            })
            .is_err(),
        "progress is only valid while reading text"
    );
}

#[test]
fn progress_before_the_text_phase_is_rejected_instead_of_faking_a_fraction() {
    let mut model = scanning_model();
    assert!(
        model
            .apply_operation_event(OperationEvent::Progress {
                completed: 2,
                total: 5,
            })
            .is_err()
    );
    assert_eq!(banner(&model).fraction, 0.0);
}

#[test]
fn completed_with_a_preview_waits_for_confirmation_and_keeps_scan_counts() {
    use notae_android::operations::events::{ImportPreviewSummary, RescanSummary};
    use notae_android::state::OperationState;
    let mut model = scanning_model();
    let scan = RescanSummary {
        added: 4,
        ..RescanSummary::default()
    };
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: scan,
            rows: vec![],
            desktop_import: DesktopImportCheck::PreviewReady(ImportPreviewSummary {
                new: 2,
                changed: 1,
                ..ImportPreviewSummary::default()
            }),
        })
        .expect("completion with preview");
    assert_eq!(
        model.state().operation(),
        &OperationState::ImportPreviewPending { candidate_count: 3 }
    );
    assert!(model.import_preview_summary().is_some());
    assert_eq!(model.status().latest_rescan, Some(scan));
    assert!(!banner(&model).busy);
}

#[test]
fn sidecar_failure_after_indexing_reports_scan_counts_and_the_failed_phase() {
    use notae_android::operations::events::RescanSummary;
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: RescanSummary {
                added: 4,
                ..RescanSummary::default()
            },
            rows: vec![],
            desktop_import: DesktopImportCheck::Failed {
                reason: "sidecar unreadable".to_owned(),
            },
        })
        .expect("completion with failed check");
    let failed = banner(&model);
    assert!(failed.failed && failed.acknowledge_enabled && !failed.busy);
    assert_eq!(
        failed.text,
        "Scan finished (4 added), but the desktop sidecar check failed: sidecar unreadable"
    );
    assert_eq!(model.status().latest_rescan.map(|scan| scan.added), Some(4));
}

#[test]
fn cancel_is_only_available_before_indexing_and_still_resolves_a_late_cancel() {
    use notae_android::state::OperationState;
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Indexing { total: 2 })
        .expect("indexing");
    // A cancel requested just before the commit boundary is reported after the Indexing event.
    model
        .apply_operation_event(OperationEvent::Canceled)
        .expect("late cancel");
    assert_eq!(model.state().operation(), &OperationState::Idle);

    let mut committed = scanning_model();
    committed
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 2 })
        .expect("reading text");
    assert!(
        committed
            .apply_operation_event(OperationEvent::Canceled)
            .is_err()
    );
}

#[test]
fn screen_changes_do_not_reset_or_fork_the_operation_state() {
    use notae_android::state::UiEvent;
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 4 })
        .expect("reading text");
    model
        .apply_operation_event(OperationEvent::Progress {
            completed: 3,
            total: 4,
        })
        .expect("progress");
    let before = banner(&model);
    for event in [
        UiEvent::OpenSearch,
        UiEvent::CloseSearch,
        UiEvent::OpenSettings,
        UiEvent::CloseSettings,
    ] {
        model.dispatch(event).expect("screen change");
        assert_eq!(banner(&model), before);
    }
}

#[test]
fn successful_result_clears_itself_but_failures_and_decisions_do_not() {
    use notae_android::operations::events::RescanSummary;
    use notae_android::state::OperationState;
    use notae_android::ui::RESULT_VISIBLE_FOR;
    use std::time::Instant;

    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: RescanSummary::default(),
            rows: vec![],
            desktop_import: DesktopImportCheck::NoSidecar,
        })
        .expect("completion");
    let start = Instant::now();
    assert!(
        !model.expire_finished_result(start),
        "clock starts when first seen"
    );
    assert!(!model.expire_finished_result(start + RESULT_VISIBLE_FOR / 2));
    assert!(matches!(
        model.state().operation(),
        OperationState::Completed { .. }
    ));
    assert!(model.expire_finished_result(start + RESULT_VISIBLE_FOR));
    assert_eq!(model.state().operation(), &OperationState::Idle);
    assert_eq!(banner(&model).text, "", "idle shows no bar");

    let mut failed = scanning_model();
    failed
        .apply_operation_event(OperationEvent::Failed {
            reason: "boom".to_owned(),
        })
        .expect("failure");
    assert!(!failed.expire_finished_result(start));
    assert!(!failed.expire_finished_result(start + RESULT_VISIBLE_FOR * 10));
    assert!(matches!(
        failed.state().operation(),
        OperationState::Failed { .. }
    ));
}

#[test]
fn pending_text_explains_the_next_action_without_claiming_work_is_running() {
    use notae_android::ui::browse::{LibrarySection, PENDING_TEXT_ACTION};
    assert_eq!(
        LibrarySection::TextPending.explanation(),
        Some("Text unavailable — run desktop OCR or import a sidecar")
    );
    assert_eq!(
        PENDING_TEXT_ACTION,
        "Text unavailable — run desktop OCR or import a sidecar"
    );
    assert_eq!(LibrarySection::All.explanation(), None);
    // Rows lacking text never make the banner busy: no pending-only progress.
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: Default::default(),
            rows: vec![],
            desktop_import: DesktopImportCheck::NoSidecar,
        })
        .expect("completion");
    assert!(!banner(&model).busy);
}
