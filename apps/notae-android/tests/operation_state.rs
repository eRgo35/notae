use notae_android::operations::events::OperationEvent;
use notae_android::operations::import::ImportPreviewViewModel;
use notae_android::operations::rescan::OperationController;
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
    );

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
    );

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
        .apply_operation_event(OperationEvent::Progress {
            completed: 2,
            total: 5,
        })
        .expect("progress event");
    assert_eq!(
        model.state().operation(),
        &OperationState::RescanInProgress {
            completed: 2,
            total: 5,
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
    );

    controller
        .start_rescan()
        .expect("explicit request starts scan");
    let summary = loop {
        match next_event(&controller) {
            OperationEvent::ImportPreviewReady { summary } => break summary,
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
    );

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
        &OperationState::RescanFinalizing {
            completed: 2,
            total: 3,
        }
    );
}
