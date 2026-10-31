use notae_android::operations::events::OperationEvent;
use notae_android::operations::rescan::OperationController;
use notae_android::state::RescanPhase;
use notae_domain::{
    IndexedNote, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath,
    Sha256Digest,
};
use notae_index::{CatalogError, FileCondition, IndexStore, PdfCatalog, PdfFile};
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
        item_id: None,
        position: None,
        missing: false,
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
        item_id: None,
        position: None,
        missing: false,
    };
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![pending_row.clone()]))
        .expect("rows event");
    let status = model.status();
    assert_eq!(status.latest_rescan, None);
    assert_eq!(
        status.indexed_count, 0,
        "row projections do not define the whole-library count"
    );
    assert_eq!(status.no_text_count, 1);

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
        })
        .expect("complete scan");
    assert_eq!(
        model.status().operation,
        OperationState::Completed {
            summary: "2 added, 1 ID conflicts, 1 failures".to_owned(),
        }
    );
    assert_eq!(model.status().latest_rescan, Some(summary));
    assert_eq!(model.status().no_text_count, 1);

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
fn settings_indexed_count_comes_from_worker_whole_library_count_not_filtered_rows() {
    use notae_android::operations::events::OperationEvent;
    use notae_index::SearchScope;

    let store = IndexStore::open(":memory:").expect("open test index");
    for (id, title) in [
        (1u32, "Autumn Leaves"),
        (2, "Blue Bossa"),
        (3, "Cantaloupe"),
    ] {
        let bytes = format!("%PDF-1.4\n{title}").into_bytes();
        let mut metadata =
            NoteMetadata::new(title, None, Vec::<String>::new()).expect("valid metadata");
        metadata.assign_id(Some(NoteId::new(id)), MetadataOrigin::FilenameDerived);
        store
            .reindex_note(&IndexedNote::new(
                RelativePdfPath::new(format!("{id} - {title}.pdf")).expect("valid path"),
                PdfFingerprint::new(
                    Sha256Digest::from_bytes(Sha256::digest(&bytes).into()),
                    u64::try_from(bytes.len()).expect("fixture fits in u64"),
                    None,
                ),
                metadata,
            ))
            .expect("insert test note");
    }
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
        .set_search("Autumn".to_owned(), SearchScope::Everything)
        .expect("filtered query");
    let mut model = notae_android::ui::UiModel::new();
    let mut filtered_rows = None;
    for _ in 0..2 {
        match next_event(&controller) {
            OperationEvent::SearchRows(rows) => {
                filtered_rows = Some(rows.len());
                model
                    .apply_operation_event(OperationEvent::SearchRows(rows))
                    .expect("rows");
            }
            event @ OperationEvent::IndexedCount(_) => {
                model.apply_operation_event(event).expect("count");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert_eq!(filtered_rows, Some(1), "the query filters visible rows");
    assert_eq!(model.status().indexed_count, 3);
}

#[test]
fn picker_search_answers_with_its_own_rows_only() {
    let store = IndexStore::open(":memory:").expect("open test index");
    for (id, title) in [(1u32, "Autumn Leaves"), (2, "Blue Bossa")] {
        let bytes = format!("%PDF-1.4\n{title}").into_bytes();
        let mut metadata =
            NoteMetadata::new(title, None, Vec::<String>::new()).expect("valid metadata");
        metadata.assign_id(Some(NoteId::new(id)), MetadataOrigin::FilenameDerived);
        store
            .reindex_note(&IndexedNote::new(
                RelativePdfPath::new(format!("{id} - {title}.pdf")).expect("valid path"),
                PdfFingerprint::new(
                    Sha256Digest::from_bytes(Sha256::digest(&bytes).into()),
                    u64::try_from(bytes.len()).expect("fixture fits in u64"),
                    None,
                ),
                metadata,
            ))
            .expect("insert test note");
    }
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
        .search_picker("Blue".to_owned())
        .expect("picker query");
    match next_event(&controller) {
        OperationEvent::PickerRows { query, rows } => {
            assert_eq!(query, "Blue");
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].title, "Blue Bossa");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    assert!(
        controller
            .recv_event_timeout(Duration::from_millis(200))
            .is_none(),
        "no library rows: the picker's search leaves them alone"
    );
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
                item_id: None,
                position: None,
                missing: false,
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

/// Runs a scan over `files` to its terminal event and returns every event it published.
fn scan_events(files: Vec<PdfFile>) -> Vec<OperationEvent> {
    let controller = OperationController::new(
        IndexStore::open(":memory:").expect("open test index"),
        Box::new(StaticCatalog {
            files,
            entered: None,
            release: None,
        }),
    )
    .expect("start test worker");
    controller.start_rescan().expect("start scan");
    let mut events = Vec::new();
    loop {
        let event = next_event(&controller);
        let terminal = matches!(
            event,
            OperationEvent::Completed { .. } | OperationEvent::Failed { .. }
        );
        events.push(event);
        if terminal {
            return events;
        }
    }
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
            OperationEvent::IndexedCount(_) => "indexed-count",
            OperationEvent::Completed { .. } => "completed",
            OperationEvent::Failed { .. } => "failed",
            _ => "other",
        })
        .collect()
}

#[test]
fn completion_is_the_last_event_and_follows_the_library_count() {
    let events = scan_events(vec![readable("12 - Autumn Leaves.pdf", b"%PDF-current")]);
    assert_eq!(
        names(&events),
        [
            "started",
            "indexing",
            "reading-text",
            "file",
            "progress",
            // The whole-library count is published before `Completed`, never after it.
            "indexed-count",
            "completed"
        ]
    );
    match events.last() {
        Some(OperationEvent::Completed { summary, .. }) => assert_eq!(summary.added, 1),
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn empty_library_skips_the_per_file_phase() {
    let events = scan_events(Vec::new());
    assert_eq!(
        names(&events),
        ["started", "indexing", "indexed-count", "completed"],
        "no per-file phase or progress for zero files"
    );
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
    assert_eq!(enumerating.fraction, None, "enumeration has no total");

    model
        .apply_operation_event(OperationEvent::Indexing { total: 3 })
        .expect("indexing");
    let indexing = banner(&model);
    assert_eq!(indexing.text, "Indexing 3 PDFs…");
    assert!(indexing.busy && !indexing.cancel_enabled);
    assert_eq!(
        indexing.fraction, None,
        "reconciliation reports no per-file progress"
    );

    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 3 })
        .expect("reading text");
    let started = banner(&model);
    assert_eq!(started.text, "Reading PDF text and thumbnails: 0 of 3");
    assert_eq!(
        started.fraction,
        Some(0.0),
        "the total is real, so the bar is determinate and empty"
    );
    model
        .apply_operation_event(OperationEvent::Progress {
            completed: 1,
            total: 3,
        })
        .expect("progress");
    let reading = banner(&model);
    assert_eq!(reading.text, "Reading PDF text and thumbnails: 1 of 3");
    assert_eq!(reading.fraction, Some(1.0 / 3.0));
    assert!(reading.busy && !reading.cancel_enabled);
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
    assert_eq!(banner(&model).fraction, None);
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
fn successful_result_clears_itself_but_failures_do_not() {
    use notae_android::operations::events::RescanSummary;
    use notae_android::state::OperationState;
    use notae_android::ui::RESULT_VISIBLE_FOR;
    use std::time::Instant;

    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: RescanSummary::default(),
            rows: vec![],
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
fn rows_without_text_are_counted_but_never_make_the_banner_busy() {
    // The library keeps image-only PDFs; Settings reports a plain count of them.
    // Rows lacking text never make the banner busy: no pending-only progress.
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Completed {
            summary: Default::default(),
            rows: vec![],
        })
        .expect("completion");
    assert!(!banner(&model).busy);
}

#[test]
fn reading_progress_never_moves_backwards() {
    let mut model = scanning_model();
    model
        .apply_operation_event(OperationEvent::Indexing { total: 4 })
        .expect("indexing");
    model
        .apply_operation_event(OperationEvent::RescanFinalizing { total: 4 })
        .expect("reading text");
    let mut previous = banner(&model)
        .fraction
        .expect("determinate once total is known");
    for completed in [1, 3, 2, 3, 4, 0] {
        model
            .apply_operation_event(OperationEvent::Progress {
                completed,
                total: 4,
            })
            .expect("progress is accepted or ignored, never an error");
        let fraction = banner(&model)
            .fraction
            .expect("still determinate while reading");
        assert!(
            fraction >= previous,
            "{completed} of 4 moved the bar back from {previous} to {fraction}"
        );
        previous = fraction;
    }
    assert_eq!(previous, 1.0);
}
