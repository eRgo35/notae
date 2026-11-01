//! The UniFFI API: `NotaeCore` driven the way the Kotlin app drives it, with a scripted
//! platform and a listener that records what Kotlin would receive.

#[path = "support/fake_platform.rs"]
mod fake_platform;

use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use fake_platform::{FakePlatform, entry, readable};
use notae_app_core::api::{
    Action, CoreListener, Effect, ExportStatus, NotaeCore, Screen, SetEntry, SetupError,
    SetupScreen, Snapshot, StatusMessage,
};
use notae_app_core::platform::{DocumentContent, PlatformError};
use notae_app_core::text::Failure;

const ROOT: &str = "content://tree/scores";

/// What Kotlin would receive: every snapshot and effect, in arrival order.
#[derive(Default)]
struct RecordingListener {
    received: Mutex<Received>,
    changed: Condvar,
}

#[derive(Default)]
struct Received {
    snapshots: Vec<Snapshot>,
    effects: Vec<Effect>,
}

impl CoreListener for RecordingListener {
    fn on_snapshot(&self, snapshot: Snapshot) {
        self.lock().snapshots.push(snapshot);
        self.changed.notify_all();
    }

    fn on_effect(&self, effect: Effect) {
        self.lock().effects.push(effect);
        self.changed.notify_all();
    }
}

impl RecordingListener {
    fn lock(&self) -> MutexGuard<'_, Received> {
        self.received.lock().expect("listener lock is healthy")
    }

    /// The newest snapshot, as Kotlin keeps it: the highest revision wins.
    fn latest(received: &Received) -> Option<&Snapshot> {
        received
            .snapshots
            .iter()
            .max_by_key(|snapshot| snapshot.revision)
    }

    /// Waits until the newest snapshot satisfies `done`, and returns it.
    fn wait_for(&self, what: &str, done: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut received = self.lock();
        loop {
            if let Some(snapshot) = Self::latest(&received).filter(|snapshot| done(snapshot)) {
                return snapshot.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "timed out waiting for {what}; last snapshot: {:#?}",
                Self::latest(&received)
            );
            received = self
                .changed
                .wait_timeout(received, left)
                .expect("listener lock is healthy")
                .0;
        }
    }

    fn effects(&self) -> Vec<Effect> {
        self.lock().effects.clone()
    }

    fn snapshot_count(&self) -> usize {
        self.lock().snapshots.len()
    }
}

struct Harness {
    core: Arc<NotaeCore>,
    platform: Arc<FakePlatform>,
    listener: Arc<RecordingListener>,
}

fn start(data_dir: &Path, platform: Arc<FakePlatform>) -> Harness {
    let listener = Arc::new(RecordingListener::default());
    let core = NotaeCore::new(
        data_dir.to_string_lossy().into_owned(),
        Arc::clone(&platform) as _,
        Arc::clone(&listener) as _,
    );
    Harness {
        core,
        platform,
        listener,
    }
}

fn library_platform(titles: &[&str]) -> Arc<FakePlatform> {
    let platform = Arc::new(FakePlatform::default());
    platform.set_entries(Ok(titles
        .iter()
        .map(|title| readable(&format!("{title}.pdf"), title.as_bytes()))
        .collect()));
    platform
}

fn is_library(snapshot: &Snapshot) -> bool {
    snapshot.screen == Screen::Library
}

fn is_finished(snapshot: &Snapshot) -> bool {
    matches!(
        snapshot.banner.message,
        Some(StatusMessage::Finished { .. })
    )
}

fn titles(snapshot: &Snapshot) -> Vec<&str> {
    snapshot.rows.iter().map(|row| row.title.as_str()).collect()
}

/// Opens the fake library and scans it; returns the snapshot with the rows.
fn open_and_scan(harness: &Harness) -> Snapshot {
    harness
        .core
        .library_root_picked("Scores".to_owned(), ROOT.to_owned());
    harness.listener.wait_for("the library", is_library);
    harness.core.dispatch(Action::Rescan);
    harness.listener.wait_for("the finished scan", is_finished)
}

/// Creates a set holding the first row's piece and leaves it open.
fn set_with_first_piece(harness: &Harness, library: &Snapshot) -> Snapshot {
    harness.core.dispatch(Action::NewSet);
    harness.core.dispatch(Action::SaveSetDialog {
        name: "Mass".to_owned(),
        date: None,
        notes: None,
    });
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[0].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);
    harness.core.snapshot()
}

#[test]
fn notae_core_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NotaeCore>();
}

#[test]
fn fresh_core_starts_unconfigured() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), Arc::new(FakePlatform::default()));

    let snapshot = harness.core.snapshot();

    assert_eq!(
        snapshot.screen,
        Screen::Setup {
            setup: SetupScreen::Unconfigured
        }
    );
    assert!(snapshot.rows.is_empty());
    assert_eq!(snapshot.banner.message, None);
    assert!(snapshot.rescan_enabled);
    assert!(!harness.core.back(), "nothing to close on the setup screen");
    assert!(harness.platform.calls().is_empty());
}

#[test]
fn picking_root_and_rescan_produces_library_rows() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));

    harness.core.dispatch(Action::PickLibraryRoot);
    assert_eq!(
        harness.listener.effects(),
        vec![Effect::PickLibraryRoot { initial_uri: None }]
    );

    let scanned = open_and_scan(&harness);
    harness.core.dispatch(Action::PickLibraryRoot);
    assert_eq!(
        harness.listener.effects().last(),
        Some(&Effect::PickLibraryRoot {
            initial_uri: Some(ROOT.to_owned())
        }),
        "Change starts the picker at the open folder"
    );

    assert_eq!(titles(&scanned), vec!["Ave Maria", "Barka"]);
    assert_eq!(scanned.settings.library_name.as_deref(), Some("Scores"));
    let Some(StatusMessage::Finished { summary }) = scanned.banner.message else {
        unreachable!("is_finished matched");
    };
    assert_eq!(summary.added, 2);
    assert_eq!(summary.failures, 0);
    assert_eq!(scanned.sections[0].count, 2, "All scores counts both");
    let calls = harness.platform.calls();
    assert_eq!(calls[0], format!("check_root {ROOT}"));
    assert!(calls.contains(&format!("enumerate {ROOT}")));
    let test_thread = thread::current().id();
    assert!(
        harness
            .platform
            .threads()
            .iter()
            .all(|thread| *thread != test_thread),
        "the platform is never called on the dispatching thread"
    );
}

#[test]
fn back_closes_innermost_first() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let in_set = set_with_first_piece(&harness, &library);
    let LibrarySetKey(set_id) = open_set_key(&in_set);
    let Some(SetEntry::Piece { row: piece }) = in_set.set_entries.first() else {
        panic!("the set holds the piece: {:#?}", in_set.set_entries);
    };
    // Reorder mode, then details, search, a dialog and a sheet, all open at once.
    harness.core.dispatch(Action::ReorderStarted);
    assert!(harness.core.snapshot().reordering);
    harness.core.dispatch(Action::SelectScore {
        selection_id: piece.selection_id.clone(),
    });
    harness.core.dispatch(Action::OpenSearch);
    harness.core.dispatch(Action::DeleteSet {
        set_id: set_id.clone(),
    });
    harness.core.dispatch(Action::AddToSet {
        note_key: piece.note_key.clone(),
    });
    let all_open = harness.core.snapshot();
    assert!(all_open.add_to_set.is_some());
    assert!(all_open.delete_set.is_some());
    assert!(all_open.selection.is_some());
    assert_eq!(all_open.screen, Screen::Search);

    assert!(harness.core.back(), "closes the sheet");
    let after = harness.core.snapshot();
    assert!(after.add_to_set.is_none());
    assert!(after.delete_set.is_some());

    assert!(harness.core.back(), "closes the dialog");
    let after = harness.core.snapshot();
    assert!(after.delete_set.is_none());
    assert!(after.selection.is_some());

    assert!(harness.core.back(), "closes the details");
    let after = harness.core.snapshot();
    assert!(after.selection.is_none());
    assert_eq!(after.screen, Screen::Search);

    assert!(harness.core.back(), "closes search");
    let after = harness.core.snapshot();
    assert_eq!(after.screen, Screen::Library);
    assert_eq!(after.section_key, format!("set:{set_id}"));

    harness.core.dispatch(Action::OpenSettings);
    assert_eq!(harness.core.snapshot().screen, Screen::Settings);
    assert!(harness.core.back(), "closes settings");
    assert_eq!(harness.core.snapshot().screen, Screen::Library);
    assert!(harness.core.snapshot().reordering);

    assert!(harness.core.back(), "finishes reorder mode");
    let after = harness.core.snapshot();
    assert!(!after.reordering);
    assert_eq!(
        after.section_key,
        format!("set:{set_id}"),
        "still in the set"
    );

    assert!(harness.core.back(), "leaves the set for the list of sets");
    assert_eq!(harness.core.snapshot().section_key, "sets");

    assert!(
        !harness.core.back(),
        "the library itself has nothing to close"
    );
    assert_eq!(harness.core.snapshot().section_key, "sets", "unchanged");
    // Every back that closed something told Kotlin.
    let received = harness.listener.snapshot_count();
    assert!(received >= 8, "{received} snapshots");
}

fn entry_ids(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .set_entries
        .iter()
        .map(|entry| match entry {
            SetEntry::Piece { row } => row.item_id.clone().unwrap_or_default(),
            SetEntry::Note { item_id, .. } => item_id.clone(),
        })
        .collect()
}

#[test]
fn reorder_moves_items_keeps_the_layout_and_persists() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(
        dir.path(),
        library_platform(&["Ave Maria", "Barka", "Credo"]),
    );
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::OpenAddPicker);
    for row in &library.rows[1..] {
        harness.core.dispatch(Action::ToggleAddPickerPiece {
            note_key: row.note_key.clone(),
        });
    }
    harness.core.dispatch(Action::CloseAddPicker);
    let in_set = harness.core.snapshot();
    let LibrarySetKey(set_id) = open_set_key(&in_set);
    let ids = entry_ids(&in_set);
    assert_eq!(ids.len(), 3);
    let view = in_set.layout.library_view;
    assert!(!in_set.reordering);

    // Starting reorder mode closes the details beside the set.
    harness.core.dispatch(Action::SelectScore {
        selection_id: ids[0].clone(),
    });
    harness.core.dispatch(Action::ReorderStarted);
    let started = harness.core.snapshot();
    assert!(started.reordering);
    assert!(started.selection.is_none());

    harness.core.dispatch(Action::MoveSetItem {
        item_id: ids[0].clone(),
        delta: 1,
    });
    assert_eq!(
        entry_ids(&harness.core.snapshot()),
        [ids[1].clone(), ids[0].clone(), ids[2].clone()]
    );
    harness.core.dispatch(Action::MoveSetItemTo {
        item_id: ids[2].clone(),
        index: 0,
    });
    let moved = harness.core.snapshot();
    let order = vec![ids[2].clone(), ids[1].clone(), ids[0].clone()];
    assert_eq!(entry_ids(&moved), order);
    assert_eq!(moved.layout.library_view, view, "the layout choice is kept");

    harness.core.dispatch(Action::ReorderFinished);
    let done = harness.core.snapshot();
    assert!(!done.reordering);
    assert_eq!(done.layout.library_view, view);

    // Leaving the set ends reorder mode.
    harness.core.dispatch(Action::ReorderStarted);
    harness.core.dispatch(Action::SelectSection {
        key: "sets".to_owned(),
    });
    assert!(!harness.core.snapshot().reordering);
    harness.core.dispatch(Action::OpenSet {
        set_id: set_id.clone(),
    });
    assert!(!harness.core.snapshot().reordering);
    // Reorder mode needs an open set.
    harness.core.dispatch(Action::SelectSection {
        key: "sets".to_owned(),
    });
    harness.core.dispatch(Action::ReorderStarted);
    assert!(!harness.core.snapshot().reordering);
    drop(harness);

    // The order is saved: a new core reads it back.
    let harness = start(
        dir.path(),
        library_platform(&["Ave Maria", "Barka", "Credo"]),
    );
    open_and_scan(&harness);
    harness.core.dispatch(Action::OpenSet { set_id });
    assert_eq!(entry_ids(&harness.core.snapshot()), order);
}

struct LibrarySetKey(String);

fn open_set_key(snapshot: &Snapshot) -> LibrarySetKey {
    LibrarySetKey(
        snapshot
            .section_key
            .strip_prefix("set:")
            .unwrap_or_else(|| panic!("a set is open: {}", snapshot.section_key))
            .to_owned(),
    )
}

#[test]
fn revoked_root_snapshot_is_permission_lost() {
    let lost = Screen::Setup {
        setup: SetupScreen::PermissionLost {
            folder: "Scores".to_owned(),
        },
    };

    // At open: the folder picked is not readable.
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria"]);
    platform.set_root(Err(PlatformError::PermissionLost));
    let harness = start(dir.path(), Arc::clone(&platform));
    harness
        .core
        .library_root_picked("Scores".to_owned(), ROOT.to_owned());
    harness
        .listener
        .wait_for("permission lost at open", |snapshot| {
            snapshot.screen == lost
        });
    assert!(!harness.core.back());
    drop(harness);

    // At reopen: a later launch finds the saved folder revoked.
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria"]);
    let first = start(dir.path(), Arc::clone(&platform));
    open_and_scan(&first);
    drop(first);
    platform.set_root(Err(PlatformError::PermissionLost));
    let relaunched = start(dir.path(), Arc::clone(&platform));
    let first_frame = relaunched.core.snapshot().screen;
    assert!(
        first_frame == Screen::Loading || first_frame == lost,
        "the first frame waits for the saved folder: {first_frame:?}"
    );
    relaunched
        .listener
        .wait_for("permission lost at reopen", |snapshot| {
            snapshot.screen == lost
        });
    let welcome = Screen::Setup {
        setup: SetupScreen::Unconfigured,
    };
    assert!(
        relaunched
            .listener
            .lock()
            .snapshots
            .iter()
            .all(|snapshot| snapshot.screen != welcome),
        "never the Welcome screen for a saved folder"
    );
    relaunched.core.dispatch(Action::PickLibraryRoot);
    assert_eq!(
        relaunched.listener.effects(),
        vec![Effect::PickLibraryRoot {
            initial_uri: Some(ROOT.to_owned())
        }],
        "Reconnect starts the picker at the saved folder"
    );

    // During a scan: the grant goes while the library is open.
    platform.set_root(Ok(()));
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), Arc::clone(&platform));
    open_and_scan(&harness);
    harness.core.dispatch(Action::AcknowledgeOperation);
    platform.set_entries(Err(PlatformError::PermissionLost));
    harness.core.dispatch(Action::Rescan);
    let revoked = harness
        .listener
        .wait_for("permission lost mid-scan", |snapshot| {
            snapshot.screen == lost
        });
    assert_eq!(revoked.banner.message, None, "no stale failure banner");

    // Picking the folder again reconnects it.
    platform.set_entries(Ok(vec![readable("Ave Maria.pdf", b"Ave Maria")]));
    harness
        .core
        .library_root_picked("Scores".to_owned(), ROOT.to_owned());
    let reconnected = harness.listener.wait_for("the library again", is_library);
    assert_eq!(titles(&reconnected), vec!["Ave Maria"]);
}

#[test]
fn platform_error_becomes_file_failure() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = Arc::new(FakePlatform::default());
    platform.set_entries(Ok(vec![
        readable("Ave Maria.pdf", b"Ave Maria"),
        entry("Broken.pdf", DocumentContent::Unreadable, 10),
    ]));
    // A Kotlin exception inside the text call reaches the core as `Unavailable`.
    platform.set_text(Err(PlatformError::Unavailable {
        detail: "java.io.IOException: stream closed".to_owned(),
    }));
    let harness = start(dir.path(), platform);

    let scanned = open_and_scan(&harness);

    let Some(StatusMessage::Finished { summary }) = scanned.banner.message else {
        unreachable!("is_finished matched");
    };
    assert_eq!(summary.failures, 2, "{summary:?}");
    assert!(!scanned.banner.busy);
    assert!(scanned.banner.acknowledge_enabled);
    assert!(titles(&scanned).contains(&"Ave Maria"));
    assert_eq!(scanned.screen, Screen::Library);
}

#[test]
fn opening_a_set_piece_emits_open_pdf_with_item_id() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let in_set = set_with_first_piece(&harness, &library);
    let row = in_set.rows[0].clone();
    let item_id = row.item_id.clone().expect("a set row has an item id");
    assert_eq!(row.selection_id, item_id);

    harness.core.dispatch(Action::OpenPdf {
        relative_path: row.relative_path.clone(),
        note_key: row.note_key.clone(),
        item_id: Some(item_id.clone()),
    });

    assert_eq!(
        harness.listener.effects().last(),
        Some(&Effect::OpenPdf {
            uri: ROOT.to_owned(),
            relative_path: "Ave Maria.pdf".to_owned(),
            note_key: row.note_key.clone(),
            item_id: Some(item_id),
        })
    );
    assert_eq!(
        harness.core.snapshot().sections[1].count,
        0,
        "not opened yet"
    );
    harness.core.pdf_opened(None);
    let opened = harness.core.snapshot();
    assert_eq!(opened.sections[1].count, 1, "Recent holds the piece");
    assert_eq!(opened.pdf_open_error, None);
}

#[test]
fn snapshot_of_2000_rows_is_built() {
    const ROWS: usize = 2000;
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = Arc::new(FakePlatform::default());
    let entries: Vec<_> = (0..ROWS)
        .map(|index| {
            readable(
                &format!("Choir/Score {index:04}.pdf"),
                format!("score {index}").as_bytes(),
            )
        })
        .collect();
    // Every score has a rendered thumbnail, so each row carries its path.
    let thumbnails = dir.path().join("thumbnails");
    std::fs::create_dir_all(&thumbnails).expect("thumbnail directory");
    for entry in &entries {
        let DocumentContent::Sha256 { hex } = &entry.content else {
            unreachable!("readable entries carry a hash");
        };
        std::fs::write(thumbnails.join(format!("{hex}.png")), b"png").expect("thumbnail");
    }
    platform.set_entries(Ok(entries));
    let harness = start(dir.path(), platform);
    let scanned = open_and_scan(&harness);
    assert_eq!(scanned.rows.len(), ROWS);

    let started = Instant::now();
    let snapshot = harness.core.snapshot();
    let built = started.elapsed();
    let started = Instant::now();
    harness.core.dispatch(Action::SelectScore {
        selection_id: snapshot.rows[1000].selection_id.clone(),
    });
    let dispatched = started.elapsed();
    assert!(snapshot.rows.iter().all(|row| row.thumbnail.is_some()));
    let started = Instant::now();
    let lowered = <Snapshot as uniffi::Lower<notae_app_core::UniFfiTag>>::lower(snapshot);
    let serialized = started.elapsed();
    let bytes = lowered.len();
    lowered.destroy();
    eprintln!(
        "snapshot of {ROWS} rows: built in {built:?}, dispatch round trip {dispatched:?}, \
         serialized to {bytes} bytes in {serialized:?}"
    );
}

/// Runs `calls` on another thread and fails, instead of hanging, if it does not return soon.
fn returns_promptly<T: Send + 'static>(
    what: &str,
    calls: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let _ = done.send(calls());
    });
    finished
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| panic!("{what} did not return while the platform call was blocked"))
}

#[test]
fn dispatch_never_waits_for_a_platform_call() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    harness.core.dispatch(Action::AcknowledgeOperation);
    // Dropping `release` (also on a failed assertion) lets the scan finish.
    let release = harness.platform.hold_enumerate();
    // The worker may already be enumerating when this dispatch builds its snapshot.
    let core = Arc::clone(&harness.core);
    returns_promptly("dispatch(Rescan)", move || core.dispatch(Action::Rescan));
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness
        .platform
        .calls()
        .iter()
        .filter(|call| call.starts_with("enumerate"))
        .count()
        < 2
    {
        assert!(
            Instant::now() < deadline,
            "the second scan never enumerated"
        );
        thread::sleep(Duration::from_millis(5));
    }

    let core = Arc::clone(&harness.core);
    let row = library.rows[0].clone();
    let (during, closed) = returns_promptly("dispatch, snapshot and back", move || {
        core.dispatch(Action::SelectScore {
            selection_id: row.selection_id.clone(),
        });
        core.dispatch(Action::OpenPdf {
            relative_path: row.relative_path.clone(),
            note_key: row.note_key.clone(),
            item_id: None,
        });
        let during = core.snapshot();
        (during, core.back())
    });

    assert!(during.banner.busy, "the scan is still enumerating");
    assert!(during.selection.is_some());
    assert_eq!(during.settings.library_name.as_deref(), Some("Scores"));
    assert!(closed, "back closed the details");
    assert!(matches!(
        harness.listener.effects().last(),
        Some(Effect::OpenPdf { .. })
    ));
    drop(release);
    harness.listener.wait_for("the second scan", is_finished);
}

#[test]
fn a_stopped_worker_is_reported_without_spinning() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria"]));
    open_and_scan(&harness);
    harness.core.dispatch(Action::AcknowledgeOperation);
    harness.platform.panic_on_enumerate();

    harness.core.dispatch(Action::Rescan);

    let stopped = |snapshot: &Snapshot| {
        matches!(
            &snapshot.banner.message,
            Some(StatusMessage::Failed {
                failure: Failure::WorkerStopped
            })
        )
    };
    let reported = harness
        .listener
        .wait_for("the stopped worker in the banner", stopped);
    assert!(!reported.banner.busy, "no banner stuck on scanning");
    // The core keeps answering: a new scan fails at once instead of waiting on a dead worker.
    let core = Arc::clone(&harness.core);
    let retried = returns_promptly("a second scan", move || {
        core.dispatch(Action::AcknowledgeOperation);
        core.dispatch(Action::Rescan);
        core.snapshot()
    });
    assert!(stopped(&retried), "{:?}", retried.banner);
}

#[test]
fn export_failure_keeps_the_platform_reason() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), Arc::new(FakePlatform::default()));

    harness.core.dispatch(Action::ExportSets);
    let Some(Effect::WriteExport { suggested_name, .. }) = harness.listener.effects().pop() else {
        panic!("export asks Kotlin to write the file");
    };
    assert!(
        suggested_name.starts_with("notae-sets-"),
        "{suggested_name}"
    );

    harness
        .core
        .file_written(Some("No space left on device".to_owned()));
    assert_eq!(
        harness.core.snapshot().settings.export,
        Some(ExportStatus::Failed {
            detail: "No space left on device".to_owned()
        })
    );
    harness.core.file_written(None);
    assert_eq!(
        harness.core.snapshot().settings.export,
        Some(ExportStatus::Exported)
    );
}

#[test]
fn a_new_core_restores_the_open_set_and_selection() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria", "Barka"]);
    let harness = start(dir.path(), Arc::clone(&platform));
    let library = open_and_scan(&harness);
    let in_set = set_with_first_piece(&harness, &library);
    let section_key = in_set.section_key.clone();
    let piece = in_set.rows[0].selection_id.clone();
    harness.core.dispatch(Action::SelectScore {
        selection_id: piece.clone(),
    });
    assert!(harness.core.snapshot().selection.is_some());
    // Android ends the process while the PDF app is in front.
    drop(harness);

    let restarted = start(dir.path(), platform);
    let restored = restarted
        .listener
        .wait_for("the restored rows", |snapshot| {
            is_library(snapshot) && !snapshot.rows.is_empty()
        });

    assert_eq!(restored.section_key, section_key);
    assert_eq!(
        restored
            .selection
            .map(|selection| selection.row.selection_id),
        Some(piece)
    );
}

#[test]
fn failed_folder_picker_shows_a_setup_error_and_import_is_offered() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), Arc::new(FakePlatform::default()));
    let before = harness.core.snapshot();
    assert!(before.import_available);

    harness
        .core
        .library_root_pick_failed("no folder picker app".to_owned());

    let failed = harness.core.snapshot();
    assert_eq!(
        failed.screen,
        Screen::Setup {
            setup: SetupScreen::Error {
                error: SetupError::Other {
                    detail: "no folder picker app".to_owned()
                }
            }
        }
    );
    assert!(failed.revision > before.revision);
}

#[test]
fn failed_folder_picker_with_a_library_open_shows_the_setup_error() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria"]));
    let library = open_and_scan(&harness);

    harness
        .core
        .library_root_pick_failed("Change failed".to_owned());

    let failed = harness.core.snapshot();
    assert_eq!(
        failed.screen,
        Screen::Setup {
            setup: SetupScreen::Error {
                error: SetupError::Other {
                    detail: "Change failed".to_owned()
                }
            }
        }
    );
    assert!(failed.revision > library.revision);
}

#[test]
fn import_is_offered_only_without_favorites_and_sets() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria"]));
    let library = open_and_scan(&harness);
    assert!(library.import_available);

    harness.core.dispatch(Action::ToggleFavorite {
        note_key: library.rows[0].note_key.clone(),
    });

    assert!(!harness.core.snapshot().import_available);
}
