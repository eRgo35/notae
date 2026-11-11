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
use notae_app_core::SetStore;
use notae_app_core::api::{
    Action, CoreListener, Effect, ExportStatus, ImportConfirmation, ImportError, ImportStatus,
    NotaeCore, Screen, SetEntry, SetupError, SetupScreen, SharedSetReview, Snapshot, StatusMessage,
    Subtitle, UnmatchedPiece,
};
use notae_app_core::platform::{DocumentContent, PlatformError};
use notae_app_core::score_notes::ScoreNotes;
use notae_app_core::text::{Failure, RowStatus};

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

    // Search is drawn over the details (ruling R45), so it closes first.
    assert!(harness.core.back(), "closes search");
    let after = harness.core.snapshot();
    assert_eq!(after.screen, Screen::Library);
    assert_eq!(after.section_key, format!("set:{set_id}"));
    assert!(after.selection.is_some(), "the details stay open");

    assert!(harness.core.back(), "closes the details");
    assert!(harness.core.snapshot().selection.is_none());

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

#[test]
fn back_closes_search_and_settings_before_the_details() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let selection_id = library.rows[1].selection_id.clone();
    harness.core.dispatch(Action::SelectScore {
        selection_id: selection_id.clone(),
    });
    let selected = |snapshot: &Snapshot| {
        snapshot
            .selection
            .as_ref()
            .map(|s| s.row.selection_id.clone())
    };

    harness.core.dispatch(Action::OpenSearch);
    assert!(harness.core.back(), "closes search");
    let after = harness.core.snapshot();
    assert_eq!(after.screen, Screen::Library);
    assert_eq!(
        selected(&after),
        Some(selection_id.clone()),
        "back keeps the details"
    );

    harness.core.dispatch(Action::OpenSearch);
    harness.core.dispatch(Action::CloseSearch);
    let after = harness.core.snapshot();
    assert_eq!(after.screen, Screen::Library);
    assert_eq!(
        selected(&after),
        Some(selection_id.clone()),
        "Escape keeps the details too"
    );

    harness.core.dispatch(Action::OpenSettings);
    assert!(harness.core.back(), "closes settings");
    let after = harness.core.snapshot();
    assert_eq!(after.screen, Screen::Library);
    assert_eq!(
        selected(&after),
        Some(selection_id),
        "back keeps the details"
    );

    assert!(harness.core.back(), "closes the details");
    assert!(harness.core.snapshot().selection.is_none());
}

fn selected_id(snapshot: &Snapshot) -> Option<String> {
    snapshot
        .selection
        .as_ref()
        .map(|selection| selection.row.selection_id.clone())
}

/// Types `query` in Search and waits until its hits are the rows.
fn search_for(harness: &Harness, query: &str, hits: &[&str]) {
    harness.core.dispatch(Action::SearchChanged {
        query: query.to_owned(),
    });
    harness
        .listener
        .wait_for("the search hits", |snapshot| titles(snapshot) == hits);
}

/// Leaves Search without a result and waits for the whole library's rows.
fn close_search_and_wait(harness: &Harness, rows: usize) -> Snapshot {
    harness.core.dispatch(Action::CloseSearch);
    harness.listener.wait_for("the library rows", |snapshot| {
        is_library(snapshot)
            && snapshot.rows.len() == rows
            && (snapshot.rows.iter()).all(|row| row.status != RowStatus::MissingFromLibrary)
    })
}

#[test]
fn a_search_that_excludes_the_selection_keeps_the_details() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria", "Barka"]);
    let harness = start(dir.path(), Arc::clone(&platform));
    let library = open_and_scan(&harness);
    let barka = library.rows[1].selection_id.clone();
    harness.core.dispatch(Action::SelectScore {
        selection_id: barka.clone(),
    });

    harness.core.dispatch(Action::OpenSearch);
    search_for(&harness, "Ave", &["Ave Maria"]);
    let closed = close_search_and_wait(&harness, 2);
    assert_eq!(selected_id(&closed), Some(barka.clone()), "in the library");
    drop(harness);
    let restarted = start(dir.path(), Arc::clone(&platform));
    let restored = restarted
        .listener
        .wait_for("the restored rows", |snapshot| snapshot.rows.len() == 2);
    assert_eq!(selected_id(&restored), Some(barka), "and in ui-prefs");

    // Inside a set, the selection is the item id of the set's copy of Ave Maria.
    let in_set = set_with_first_piece(&restarted, &restored);
    let item = in_set.rows[0].selection_id.clone();
    restarted.core.dispatch(Action::SelectScore {
        selection_id: item.clone(),
    });
    restarted.core.dispatch(Action::OpenSearch);
    search_for(&restarted, "Barka", &["Barka"]);
    let closed = close_search_and_wait(&restarted, 1);
    assert_eq!(selected_id(&closed), Some(item), "in a set");
}

#[test]
fn a_score_deleted_during_search_is_unselected_when_search_closes() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria", "Barka"]);
    let harness = start(dir.path(), Arc::clone(&platform));
    let library = open_and_scan(&harness);
    harness.core.dispatch(Action::SelectScore {
        selection_id: library.rows[1].selection_id.clone(),
    });
    harness.core.dispatch(Action::OpenSearch);
    search_for(&harness, "Ave", &["Ave Maria"]);
    platform.set_entries(Ok(vec![
        readable("Ave Maria.pdf", b"Ave Maria"),
        readable("Credo.pdf", b"Credo"),
    ]));
    harness.core.dispatch(Action::AcknowledgeOperation);
    assert!(
        !is_finished(&harness.core.snapshot()),
        "the first result is gone"
    );
    harness.core.dispatch(Action::Rescan);
    harness.listener.wait_for("the second scan", is_finished);

    let closed = close_search_and_wait(&harness, 1);
    assert_eq!(selected_id(&closed), None);
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

fn notes(snapshot: &Snapshot) -> Vec<(u32, &str)> {
    snapshot
        .set_entries
        .iter()
        .filter_map(|entry| match entry {
            SetEntry::Note { position, text, .. } => Some((*position, text.as_str())),
            SetEntry::Piece { .. } => None,
        })
        .collect()
}

#[test]
fn add_note_appends_at_the_end_and_closes_the_picker() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::AddNote {
        text: "  Psalm 23\n".to_owned(),
    });
    let added = harness.core.snapshot();
    assert!(added.add_picker.is_none(), "Add note closes the picker");
    assert_eq!(notes(&added), [(2, "Psalm 23")], "at the end, trimmed");
    assert_eq!(
        added.subtitle,
        Subtitle::Set {
            date: None,
            pieces: 1,
            notes: 1
        }
    );
    assert_eq!(added.rows.len(), 1, "a note is not a score row");
    let ids = entry_ids(&added);
    let LibrarySetKey(set_id) = open_set_key(&added);

    harness.core.dispatch(Action::EditNote {
        item_id: ids[1].clone(),
        text: "Psalm 23, verses 1-3 ".to_owned(),
    });
    assert_eq!(
        notes(&harness.core.snapshot()),
        [(2, "Psalm 23, verses 1-3")]
    );
    // A new piece goes after the note.
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[1].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);
    assert_eq!(entry_ids(&harness.core.snapshot())[..2], ids[..]);
    harness.core.dispatch(Action::RemoveSetItem {
        item_id: ids[1].clone(),
    });
    assert!(notes(&harness.core.snapshot()).is_empty(), "removed");
    harness.core.dispatch(Action::AddNote {
        text: "Homily".to_owned(),
    });
    drop(harness);

    // Notes are saved with the set.
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    open_and_scan(&harness);
    harness.core.dispatch(Action::OpenSet { set_id });
    assert_eq!(notes(&harness.core.snapshot()), [(3, "Homily")]);
}

#[test]
fn empty_note_cannot_be_added() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::AddNote {
        text: "Psalm".to_owned(),
    });
    let before = harness.core.snapshot();
    let note_id = entry_ids(&before)[1].clone();
    harness.core.dispatch(Action::OpenAddPicker);
    for blank in ["", " ", " \n\t "] {
        harness.core.dispatch(Action::AddNote {
            text: blank.to_owned(),
        });
        harness.core.dispatch(Action::EditNote {
            item_id: note_id.clone(),
            text: blank.to_owned(),
        });
    }
    let after = harness.core.snapshot();
    assert_eq!(entry_ids(&after), entry_ids(&before), "nothing added");
    assert_eq!(notes(&after), [(2, "Psalm")], "the note keeps its text");
    assert!(
        after.add_picker.is_some(),
        "a blank note leaves the picker open"
    );
}

#[test]
fn details_previous_next_skip_notes() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::AddNote {
        text: "Psalm".to_owned(),
    });
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[1].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);
    let ids = entry_ids(&harness.core.snapshot());
    assert_eq!(ids.len(), 3);

    harness.core.dispatch(Action::SelectScore {
        selection_id: ids[0].clone(),
    });
    let first = harness.core.snapshot().selection.expect("details open");
    assert_eq!(first.previous, None);
    assert_eq!(
        first.next.as_deref(),
        Some(ids[2].as_str()),
        "over the note"
    );
    let place = first.position.expect("in the set");
    assert_eq!(
        (place.position, place.total),
        (1, 2),
        "notes are not counted"
    );

    harness.core.dispatch(Action::SelectScore {
        selection_id: ids[2].clone(),
    });
    let last = harness.core.snapshot().selection.expect("details open");
    assert_eq!(
        last.previous.as_deref(),
        Some(ids[0].as_str()),
        "over the note"
    );
    assert_eq!(last.next, None);
    let place = last.position.expect("in the set");
    assert_eq!((place.position, place.total), (2, 2));
    assert_eq!(
        last.row.position,
        Some(3),
        "the badge still counts the note"
    );

    // A note has no details.
    harness.core.dispatch(Action::SelectScore {
        selection_id: ids[1].clone(),
    });
    assert!(harness.core.snapshot().selection.is_none());
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

#[test]
fn a_thumbnail_rendered_after_a_snapshot_shows_in_the_next() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria"]));
    let library = open_and_scan(&harness);
    assert_eq!(library.rows[0].thumbnail, None);

    let path = dir
        .path()
        .join("thumbnails")
        .join(format!("{}.png", library.rows[0].note_key));
    std::fs::write(&path, b"png").expect("thumbnail");
    let next = harness.core.snapshot();
    assert_eq!(
        next.rows[0].thumbnail.as_deref(),
        Some(path.to_string_lossy().as_ref())
    );
}

#[test]
fn set_card_previews_use_rendered_thumbnails() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[1].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);

    let path = dir
        .path()
        .join("thumbnails")
        .join(format!("{}.png", library.rows[0].note_key));
    std::fs::create_dir_all(path.parent().expect("thumbnail directory")).expect("directory");
    std::fs::write(&path, b"png").expect("thumbnail");
    assert_eq!(
        harness.core.snapshot().set_cards[0].previews,
        [Some(path.to_string_lossy().into_owned()), None]
    );
}

#[test]
fn set_choice_previews_match_the_set_card() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[1].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);

    let path = dir
        .path()
        .join("thumbnails")
        .join(format!("{}.png", library.rows[0].note_key));
    std::fs::create_dir_all(path.parent().expect("thumbnail directory")).expect("directory");
    std::fs::write(&path, b"png").expect("thumbnail");
    harness.core.dispatch(Action::AddToSet {
        note_key: library.rows[1].note_key.clone(),
    });
    let snapshot = harness.core.snapshot();
    let sheet = snapshot.add_to_set.expect("checklist");
    assert_eq!(
        sheet.choices[0].previews,
        [Some(path.to_string_lossy().into_owned()), None]
    );
    assert_eq!(sheet.choices[0].previews, snapshot.set_cards[0].previews);

    // From a search result, Ave Maria is outside the hits but still in the library.
    harness.core.dispatch(Action::CloseAddToSet);
    harness.core.dispatch(Action::OpenSearch);
    search_for(&harness, "Barka", &["Barka"]);
    harness.core.dispatch(Action::AddToSet {
        note_key: library.rows[1].note_key.clone(),
    });
    let sheet = harness.core.snapshot().add_to_set.expect("checklist");
    assert_eq!(
        sheet.choices[0].previews,
        [Some(path.to_string_lossy().into_owned()), None]
    );
}

#[test]
fn set_choice_previews_during_a_search_show_a_piece_gone_from_the_library_as_missing() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria", "Barka", "Credo"]);
    let harness = start(dir.path(), Arc::clone(&platform));
    let library = open_and_scan(&harness);
    assert_eq!(titles(&library), ["Ave Maria", "Barka", "Credo"]);
    set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[1].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);

    // Barka leaves the library; a thumbnail of it is still in the cache.
    platform.set_entries(Ok(vec![
        readable("Ave Maria.pdf", b"Ave Maria"),
        readable("Credo.pdf", b"Credo"),
    ]));
    harness.core.dispatch(Action::AcknowledgeOperation);
    harness.core.dispatch(Action::Rescan);
    harness
        .listener
        .wait_for("the scan without Barka", is_finished);
    let thumbnail = |row: &Snapshot, index: usize| {
        let path = dir
            .path()
            .join("thumbnails")
            .join(format!("{}.png", row.rows[index].note_key));
        std::fs::create_dir_all(path.parent().expect("thumbnail directory")).expect("directory");
        std::fs::write(&path, b"png").expect("thumbnail");
        path.to_string_lossy().into_owned()
    };
    let (ave, _) = (thumbnail(&library, 0), thumbnail(&library, 1));

    harness.core.dispatch(Action::OpenSearch);
    search_for(&harness, "Ave", &["Ave Maria"]);
    harness.core.dispatch(Action::AddToSet {
        note_key: library.rows[0].note_key.clone(),
    });
    let sheet = harness.core.snapshot().add_to_set.expect("checklist");
    assert_eq!(sheet.choices[0].previews, [Some(ave), None]);
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
fn dropping_the_core_cancels_a_running_scan() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria", "Barka"]);
    let harness = start(dir.path(), Arc::clone(&platform));
    harness
        .core
        .library_root_picked("Scores".to_owned(), ROOT.to_owned());
    harness.listener.wait_for("the library", is_library);
    let release = platform.hold_enumerate();
    harness.core.dispatch(Action::Rescan);
    harness.listener.wait_for("the running scan", |snapshot| {
        snapshot.banner.message.is_some() && !is_finished(snapshot)
    });

    // The drop waits for the worker, which is inside the platform call until this releases it.
    let releaser = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let _ = release.send(());
    });
    drop(harness);
    releaser.join().expect("releaser thread");

    let restarted = start(dir.path(), platform);
    let reopened = restarted
        .listener
        .wait_for("the reopened library", is_library);
    assert!(
        reopened.rows.is_empty(),
        "the scan did not finish: {reopened:#?}"
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

#[test]
fn score_note_survives_rescan_and_rename() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = library_platform(&["Ave Maria"]);
    let harness = start(dir.path(), Arc::clone(&platform));
    let library = open_and_scan(&harness);
    let key = library.rows[0].note_key.clone();
    harness.core.dispatch(Action::SaveScoreNote {
        note_key: key.clone(),
        text: "  slow start ".to_owned(),
    });
    let file = dir.path().join("score-notes.json");
    assert_eq!(ScoreNotes::load(&file).get(&key), Some("slow start"));

    harness.core.dispatch(Action::AcknowledgeOperation);
    platform.set_entries(Ok(vec![readable("Renamed.pdf", b"Ave Maria")]));
    harness.core.dispatch(Action::Rescan);
    let rescanned = harness.listener.wait_for("the second scan", is_finished);
    assert_eq!(rescanned.rows[0].relative_path, "Renamed.pdf");

    let notes = ScoreNotes::load(&file);
    assert_eq!(notes.get(&key), Some("slow start"));
}

#[test]
fn opening_item_marks_just_opened_without_bumping_modified() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    // A repeat, so only the copy opened is marked.
    harness.core.dispatch(Action::OpenAddPicker);
    harness.core.dispatch(Action::ToggleAddPickerPiece {
        note_key: library.rows[0].note_key.clone(),
    });
    harness.core.dispatch(Action::CloseAddPicker);
    assert_eq!(harness.core.snapshot().rows.len(), 2);
    let sets_file = dir.path().join("sets.json");
    let before = SetStore::load(&sets_file).sets()[0].clone();
    assert!(
        harness
            .core
            .snapshot()
            .rows
            .iter()
            .all(|row| !row.just_opened)
    );

    let second = harness.core.snapshot().rows[1].clone();
    harness.core.dispatch(Action::OpenPdf {
        relative_path: second.relative_path.clone(),
        note_key: second.note_key.clone(),
        item_id: second.item_id.clone(),
    });

    let opened = harness.core.snapshot();
    assert_eq!(
        opened
            .rows
            .iter()
            .map(|row| row.just_opened)
            .collect::<Vec<_>>(),
        vec![false, true]
    );
    let Some(SetEntry::Piece { row }) = opened.set_entries.get(1) else {
        panic!("the second entry is the piece: {:?}", opened.set_entries);
    };
    assert!(row.just_opened);
    let after = SetStore::load(&sets_file).sets()[0].clone();
    assert_eq!(
        after.last_opened_item, second.item_id,
        "saved for the next launch"
    );
    assert_eq!(after.modified, before.modified, "opening is not an edit");

    // Outside the set no row is marked.
    harness.core.dispatch(Action::SelectSection {
        key: "all".to_owned(),
    });
    assert!(
        harness
            .core
            .snapshot()
            .rows
            .iter()
            .all(|row| !row.just_opened)
    );
}

/// Creates the set "Mass", exports it, renames it "Evening Mass" and stars a piece; returns the
/// export file's text.
fn export_then_change(harness: &Harness, library: &Snapshot) -> String {
    set_with_first_piece(harness, library);
    harness.core.dispatch(Action::ExportSets);
    let Some(Effect::WriteExport { json, .. }) = harness.listener.effects().pop() else {
        panic!("export asks Kotlin to write the file");
    };
    let set_id = harness.core.snapshot().set_cards[0].id.clone();
    harness.core.dispatch(Action::EditSet { set_id });
    harness.core.dispatch(Action::SaveSetDialog {
        name: "Evening Mass".to_owned(),
        date: None,
        notes: None,
    });
    harness.core.dispatch(Action::ToggleFavorite {
        note_key: library.rows[1].note_key.clone(),
    });
    json
}

#[test]
fn import_cancel_changes_nothing() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let json = export_then_change(&harness, &library);
    let sets_before = std::fs::read(dir.path().join("sets.json")).expect("sets saved");
    let favorites_before =
        std::fs::read(dir.path().join("favorites.json")).expect("favorites saved");

    harness.core.dispatch(Action::ImportRequested);
    assert_eq!(
        harness.listener.effects().last(),
        Some(&Effect::PickImportFile)
    );
    harness.core.import_file_read(json);
    assert_eq!(
        harness.core.snapshot().import_confirmation,
        Some(ImportConfirmation {
            sets: 1,
            replaced: vec!["Evening Mass".to_owned()],
            shared: None,
        })
    );
    harness.core.dispatch(Action::ImportCancelled);

    let cancelled = harness.core.snapshot();
    assert_eq!(cancelled.import_confirmation, None);
    assert_eq!(cancelled.settings.import_status, None);
    assert_eq!(cancelled.set_cards[0].name, "Evening Mass");
    assert_eq!(
        std::fs::read(dir.path().join("sets.json")).ok(),
        Some(sets_before)
    );
    assert_eq!(
        std::fs::read(dir.path().join("favorites.json")).ok(),
        Some(favorites_before)
    );
    // Confirming after a cancel has nothing to apply.
    harness.core.dispatch(Action::ImportConfirmed);
    assert_eq!(harness.core.snapshot().set_cards[0].name, "Evening Mass");
}

#[test]
fn import_confirmed_replaces_named_sets_and_merges_favorites() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let json = export_then_change(&harness, &library);
    harness.core.dispatch(Action::ImportRequested);
    harness.core.import_file_read(json);

    harness.core.dispatch(Action::ImportConfirmed);

    let imported = harness.core.snapshot();
    assert_eq!(imported.import_confirmation, None);
    assert_eq!(
        imported.settings.import_status,
        Some(ImportStatus::Imported)
    );
    assert_eq!(imported.set_cards.len(), 1);
    assert_eq!(
        imported.set_cards[0].name, "Mass",
        "the file's set replaced it"
    );
    assert_eq!(
        imported.sections[2].count, 1,
        "the star made after the export stays"
    );
    let saved = SetStore::load(&dir.path().join("sets.json"));
    assert_eq!(saved.sets()[0].name, "Mass");
}

#[test]
fn import_of_a_wrong_file_says_why_and_back_closes_the_confirmation() {
    let dir = tempfile::tempdir().expect("temporary directory");
    // On the setup screen, before any folder: the first-launch offer.
    let harness = start(dir.path(), Arc::new(FakePlatform::default()));
    harness.core.dispatch(Action::ImportRequested);

    harness
        .core
        .import_file_read(r#"{"app":"other"}"#.to_owned());
    assert_eq!(
        harness.core.snapshot().settings.import_status,
        Some(ImportStatus::Failed {
            error: ImportError::NotNotae
        })
    );
    harness
        .core
        .import_file_read(r#"{"app":"notae","schema_version":3}"#.to_owned());
    assert_eq!(
        harness.core.snapshot().settings.import_status,
        Some(ImportStatus::Failed {
            error: ImportError::UnsupportedVersion
        })
    );
    harness.core.import_file_read("not json".to_owned());
    assert_eq!(
        harness.core.snapshot().settings.import_status,
        Some(ImportStatus::Failed {
            error: ImportError::Invalid
        })
    );
    harness.core.import_file_failed("no file picker".to_owned());
    assert_eq!(
        harness.core.snapshot().settings.import_status,
        Some(ImportStatus::Failed {
            error: ImportError::Unreadable {
                detail: "no file picker".to_owned()
            }
        })
    );

    harness.core.import_file_read(
        r#"{"schema_version":1,"app":"notae","favorites":[],"sets":[]}"#.to_owned(),
    );
    let asking = harness.core.snapshot();
    assert_eq!(
        asking.import_confirmation,
        Some(ImportConfirmation {
            sets: 0,
            replaced: Vec::new(),
            shared: None,
        })
    );
    assert_eq!(asking.settings.import_status, None);
    assert!(
        harness.core.back(),
        "Back closes the confirmation over the setup screen"
    );
    assert_eq!(harness.core.snapshot().import_confirmation, None);
    assert!(!harness.core.back());
}

#[test]
fn rows_carry_score_and_set_notes() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let key = library.rows[0].note_key.clone();
    let in_set = set_with_first_piece(&harness, &library);
    let item_id = in_set.rows[0].item_id.clone().expect("a set row");
    harness.core.dispatch(Action::SaveScoreNote {
        note_key: key.clone(),
        text: "Slowly".to_owned(),
    });
    harness.core.dispatch(Action::SaveSetNote {
        item_id: item_id.clone(),
        text: "Repeat twice".to_owned(),
    });

    harness.core.dispatch(Action::SelectScore {
        selection_id: item_id,
    });
    let row = harness.core.snapshot().selection.expect("details open").row;
    assert_eq!(row.score_note.as_deref(), Some("Slowly"));
    assert_eq!(row.set_note.as_deref(), Some("Repeat twice"));

    harness.core.dispatch(Action::SelectSection {
        key: "all".to_owned(),
    });
    harness
        .core
        .dispatch(Action::SelectScore { selection_id: key });
    let row = harness.core.snapshot().selection.expect("details open").row;
    assert_eq!(row.score_note.as_deref(), Some("Slowly"));
    assert_eq!(row.set_note, None);
}

#[test]
fn share_set_returns_the_file_and_text_and_ignores_unknown_sets() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    set_with_first_piece(&harness, &library);
    let set_id = harness.core.snapshot().set_cards[0].id.clone();

    harness.core.dispatch(Action::ShareSet {
        set_id: "missing".to_owned(),
        with_notes: true,
    });
    assert!(
        !harness
            .listener
            .effects()
            .iter()
            .any(|effect| matches!(effect, Effect::ShareSet { .. }))
    );

    harness.core.dispatch(Action::ShareSet {
        set_id: set_id.clone(),
        with_notes: false,
    });
    let Some(Effect::ShareSet {
        file_name,
        json,
        text,
    }) = harness.listener.effects().pop()
    else {
        panic!("sharing asks Kotlin to send the set");
    };
    assert_eq!(file_name, "Mass.notae-set");
    assert!(json.contains(r#""kind":"set""#), "{json}");
    assert!(text.starts_with("Mass\n1. "), "{text}");
    assert!(text.contains("Ave Maria"), "{text}");

    // A search's hits lack Ave Maria's title, so the set is not shared from them.
    harness.core.dispatch(Action::OpenSearch);
    search_for(&harness, "Barka", &["Barka"]);
    let shared = harness.listener.effects().len();
    harness.core.dispatch(Action::ShareSet {
        set_id,
        with_notes: false,
    });
    assert_eq!(harness.listener.effects().len(), shared);
}

/// A shared set file: "Ave Maria" under another key but the library's path, then "Lost" (9), which no library
/// holds.
fn shared_set_file(id: &str, name: &str) -> String {
    let (other, lost) = ("f".repeat(64), "e".repeat(64));
    format!(
        r#"{{"schema_version":2,"app":"notae","kind":"set","set":{{"id":"{id}","name":"{name}","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[{{"id":"i1","kind":"piece","key":"{other}","path":"Ave Maria.pdf","title":"Ave Maria"}},{{"id":"i2","kind":"piece","key":"{lost}","path":"Lost.pdf","number":"9","title":"Lost"}}]}}}}"#
    )
}

fn review(name: &str, exists: bool) -> SharedSetReview {
    SharedSetReview {
        name: name.to_owned(),
        matched: 1,
        unmatched: vec![UnmatchedPiece {
            number: Some("9".to_owned()),
            title: "Lost".to_owned(),
        }],
        exists,
    }
}

#[test]
fn shared_set_waits_for_the_library() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    assert!(matches!(
        harness.core.snapshot().screen,
        Screen::Setup { .. }
    ));

    harness.core.import_file_read(shared_set_file("s1", "Mass"));
    let waiting = harness.core.snapshot();
    assert_eq!(waiting.import_confirmation, None);
    assert_eq!(waiting.settings.import_status, None);
    assert!(!harness.core.back(), "Back does not drop the waiting set");

    open_and_scan(&harness);
    assert_eq!(
        harness.core.snapshot().import_confirmation,
        Some(ImportConfirmation {
            sets: 1,
            replaced: Vec::new(),
            shared: Some(review("Mass", false)),
        })
    );
}

#[test]
fn shared_set_replace_and_keep_both() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let harness = start(dir.path(), library_platform(&["Ave Maria", "Barka"]));
    let library = open_and_scan(&harness);
    let ave = library
        .rows
        .iter()
        .find(|row| row.title == "Ave Maria")
        .expect("Ave Maria")
        .note_key
        .clone();
    let in_set = set_with_first_piece(&harness, &library);
    harness.core.dispatch(Action::SaveSetNote {
        item_id: in_set.rows[0].selection_id.clone(),
        text: "breathe".to_owned(),
    });
    let set_id = harness.core.snapshot().set_cards[0].id.clone();
    let file = shared_set_file(&set_id, "Choir Mass");
    let before_import = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs();

    harness.core.import_file_read(file.clone());
    assert_eq!(
        harness.core.snapshot().import_confirmation,
        Some(ImportConfirmation {
            sets: 1,
            replaced: vec!["Mass".to_owned()],
            shared: Some(review("Choir Mass", true)),
        })
    );
    harness.core.dispatch(Action::ImportConfirmed);
    let replaced = harness.core.snapshot();
    assert_eq!(replaced.import_confirmation, None);
    assert_eq!(
        replaced.settings.import_status,
        Some(ImportStatus::Imported)
    );
    assert_eq!(replaced.set_cards.len(), 1);
    assert_eq!(replaced.set_cards[0].name, "Choir Mass");
    let saved = SetStore::load(&dir.path().join("sets.json"));
    let set = saved.get(&set_id).expect("the replaced set is saved");
    let notae_app_core::sets::ItemKind::Piece(first) = &set.items[0].kind else {
        panic!("a piece");
    };
    assert_eq!(first.key, ave, "the matched piece takes the library's key");
    assert_eq!(
        set.items[0].set_note.as_deref(),
        Some("breathe"),
        "the file has no set note there, so the local one stays"
    );
    let notae_app_core::sets::ItemKind::Piece(lost) = &set.items[1].kind else {
        panic!("a piece");
    };
    assert_eq!(
        lost.path, "Lost.pdf",
        "the unmatched piece is a placeholder"
    );
    assert!(
        set.modified >= before_import,
        "a received set is edited now, not at the sender's time (1)"
    );

    harness.core.import_file_read(file);
    harness.core.dispatch(Action::ImportKeepBoth);
    let both = harness.core.snapshot();
    assert_eq!(both.import_confirmation, None);
    let mut names: Vec<&str> = both
        .set_cards
        .iter()
        .map(|card| card.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, vec!["Choir Mass", "Choir Mass (2)"]);
    let saved = SetStore::load(&dir.path().join("sets.json"));
    assert_eq!(saved.sets().len(), 2);
    assert_ne!(saved.sets()[0].id, saved.sets()[1].id);
    assert!(saved.sets().iter().all(|set| set.modified >= before_import));
}

#[test]
fn shared_set_received_during_a_search_matches_the_whole_library() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let platform = Arc::new(FakePlatform::default());
    platform.set_entries(Ok(vec![
        readable("Ave Maria.pdf", b"Ave Maria"),
        readable("12 - Barka.pdf", b"Barka"),
    ]));
    let harness = start(dir.path(), platform);
    let library = open_and_scan(&harness);
    let barka = library
        .rows
        .iter()
        .find(|row| row.title == "Barka")
        .expect("Barka")
        .note_key
        .clone();
    harness.core.dispatch(Action::OpenSearch);
    search_for(&harness, "Ave", &["Ave Maria"]);

    // Barka under another key and path: only its number and title can match it.
    let other = "f".repeat(64);
    harness.core.import_file_read(format!(
        r#"{{"schema_version":2,"app":"notae","kind":"set","set":{{"id":"s1","name":"Mass","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[{{"id":"i1","kind":"piece","key":"{other}","path":"Old/Barka.pdf","number":"12","title":"Barka"}}]}}}}"#
    ));
    let review = harness.listener.wait_for("the review", |snapshot| {
        snapshot.import_confirmation.is_some()
    });
    let screen = review.screen.clone();
    let shared = review
        .import_confirmation
        .and_then(|confirmation| confirmation.shared)
        .expect("a shared set review");
    assert_eq!((shared.matched, shared.unmatched.len()), (1, 0));
    assert_eq!(screen, Screen::Library, "the file closes Search");

    harness.core.dispatch(Action::ImportConfirmed);
    let saved = SetStore::load(&dir.path().join("sets.json"));
    let set = saved.get("s1").expect("the imported set is saved");
    let notae_app_core::sets::ItemKind::Piece(piece) = &set.items[0].kind else {
        panic!("a piece");
    };
    assert_eq!(piece.key, barka, "matched by number and title");
}
