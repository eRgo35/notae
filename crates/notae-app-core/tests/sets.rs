//! Singing sets: the store, its file and the set actions on the UI model.

use notae_app_core::favorites::PieceRef;
use notae_app_core::operations::events::{OperationEvent, RescanSummary};
use notae_app_core::sets::{ItemKind, SetFieldError, SetFields, SetStore, new_id};
use notae_app_core::ui::UiModel;
use notae_app_core::ui::search::SearchRow;

fn fields(name: &str) -> SetFields {
    SetFields {
        name: name.to_owned(),
        date: None,
        notes: None,
    }
}

fn piece(c: char) -> PieceRef {
    PieceRef {
        key: c.to_string().repeat(64),
        path: format!("{c}.pdf"),
    }
}

/// The library row for `Kyrie.pdf`, whose content key is `c` repeated.
fn kyrie_row(c: char) -> SearchRow {
    SearchRow {
        id: None,
        title: "Kyrie".to_owned(),
        composer_or_artist: None,
        tags: Vec::new(),
        status: notae_app_core::text::RowStatus::Ready,
        relative_path: "Kyrie.pdf".to_owned(),
        note_key: c.to_string().repeat(64),
        text_pending: false,
        title_origin: notae_domain::MetadataOrigin::Unset,
        composer_origin: notae_domain::MetadataOrigin::Unset,
        text_snippet: None,
        item_id: None,
        position: None,
        missing: false,
    }
}

/// The worker's rescan result carrying `rows`.
fn completed(rows: Vec<SearchRow>) -> OperationEvent {
    OperationEvent::Completed {
        summary: RescanSummary::default(),
        rows,
    }
}

fn item_ids(store: &SetStore, set: &str) -> Vec<String> {
    store
        .get(set)
        .unwrap()
        .items
        .iter()
        .map(|i| i.id.clone())
        .collect()
}

#[test]
fn create_validates_name_and_date() {
    let blank = fields("  ").validate();
    assert_eq!(blank, Err(SetFieldError::EmptyName));
    let bad = SetFields {
        date: Some("2026-02-30".to_owned()),
        ..fields("Mass")
    };
    assert_eq!(bad.validate(), Err(SetFieldError::InvalidDate));
    for date in [
        "2026-13-01",
        "2026-1-01",
        "2027-02-29",
        "abcd-ef-gh",
        "2026-04-31",
        "2026-00-10",
        "é026-01-01",
    ] {
        let f = SetFields {
            date: Some(date.to_owned()),
            ..fields("Mass")
        };
        assert_eq!(f.validate(), Err(SetFieldError::InvalidDate), "{date}");
    }
    let leap = SetFields {
        name: "  Mass ".to_owned(),
        date: Some("2028-02-29".to_owned()),
        notes: Some("  ".to_owned()),
    };
    let ok = leap.validate().unwrap();
    assert_eq!(ok.name, "Mass");
    assert_eq!(ok.date.as_deref(), Some("2028-02-29"));
    assert_eq!(ok.notes, None);
    let empty_date = SetFields {
        date: Some(String::new()),
        ..fields("Mass")
    };
    assert_eq!(empty_date.validate().unwrap().date, None);
}

#[test]
fn sets_order_most_recently_edited_first() {
    let mut store = SetStore::default();
    let a = store.create(fields("A"), 10);
    let b = store.create(fields("B"), 20);
    let c = store.create(fields("C"), 20);
    let order: Vec<&str> = store
        .sets_by_modified()
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(order, [b.as_str(), c.as_str(), a.as_str()]);
    assert!(store.update(&a, fields("A"), 30));
    assert_eq!(store.sets_by_modified()[0].id, a);
    assert!(!store.update("nope", fields("X"), 40));
}

#[test]
fn add_piece_appends_and_allows_repeats() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let first = store.add_piece(&set, piece('a'), 2).unwrap();
    let second = store.add_piece(&set, piece('a'), 3).unwrap();
    let note = store.add_note(&set, "Intro".to_owned(), 4).unwrap();
    assert_ne!(first, second);
    assert_eq!(item_ids(&store, &set), [first, second, note]);
    assert_eq!(store.get(&set).unwrap().modified, 4);
    assert_eq!(store.add_piece("nope", piece('a'), 5), None);
    assert_eq!(store.add_note("nope", "x".to_owned(), 5), None);
}

#[test]
fn remove_and_move_bump_modified() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let a = store.add_piece(&set, piece('a'), 1).unwrap();
    let b = store.add_piece(&set, piece('b'), 1).unwrap();
    assert!(store.move_item(&set, &b, -1, 5));
    assert_eq!(item_ids(&store, &set), [b.clone(), a.clone()]);
    assert_eq!(store.get(&set).unwrap().modified, 5);
    assert!(store.remove_item(&set, &b, 9));
    assert_eq!(item_ids(&store, &set), [a]);
    assert_eq!(store.get(&set).unwrap().modified, 9);
    assert!(!store.remove_item(&set, &b, 12));
    assert_eq!(store.get(&set).unwrap().modified, 9);
}

#[test]
fn move_item_stops_at_edges() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let a = store.add_piece(&set, piece('a'), 1).unwrap();
    let b = store.add_piece(&set, piece('b'), 1).unwrap();
    let c = store.add_piece(&set, piece('c'), 1).unwrap();
    assert!(!store.move_item(&set, &a, -1, 7));
    assert!(!store.move_item(&set, &c, 1, 7));
    assert!(!store.move_item(&set, &b, 0, 7));
    assert!(!store.move_item(&set, "nope", 1, 7));
    assert_eq!(store.get(&set).unwrap().modified, 1);
    assert!(store.move_item(&set, &a, 5, 8));
    assert_eq!(item_ids(&store, &set), [b, c, a]);
}

#[test]
fn move_item_to_clamps() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let a = store.add_piece(&set, piece('a'), 1).unwrap();
    let b = store.add_piece(&set, piece('b'), 1).unwrap();
    let c = store.add_note(&set, "Psalm".to_owned(), 1).unwrap();
    assert!(
        store.move_item_to(&set, &a, 99, 2),
        "past the end is the last place"
    );
    assert_eq!(item_ids(&store, &set), [b.clone(), c.clone(), a.clone()]);
    assert!(store.move_item_to(&set, &a, 0, 3));
    assert_eq!(item_ids(&store, &set), [a.clone(), b.clone(), c.clone()]);
    assert!(
        store.move_item_to(&set, &a, 1, 4),
        "the index is the place after the move"
    );
    assert_eq!(item_ids(&store, &set), [b.clone(), a.clone(), c.clone()]);
    assert!(!store.move_item_to(&set, &a, 1, 5), "already there");
    assert!(!store.move_item_to(&set, &c, usize::MAX, 5), "already last");
    assert!(!store.move_item_to(&set, "nope", 0, 5));
    assert!(!store.move_item_to("nope", &a, 0, 5));
    assert_eq!(item_ids(&store, &set), [b, a, c]);
}

#[test]
fn move_item_to_bumps_modified() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let a = store.add_piece(&set, piece('a'), 1).unwrap();
    store.add_piece(&set, piece('b'), 1).unwrap();
    assert!(store.move_item_to(&set, &a, 1, 7));
    assert_eq!(store.get(&set).unwrap().modified, 7);
    assert!(!store.move_item_to(&set, &a, 1, 9), "no move, no change");
    assert_eq!(store.get(&set).unwrap().modified, 7);
}

#[test]
fn edit_note_bumps_modified() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let a = store.add_piece(&set, piece('a'), 1).unwrap();
    let note = store.add_note(&set, "Psalm".to_owned(), 1).unwrap();
    assert!(store.edit_note(&set, &note, "Psalm 23".to_owned(), 7));
    let stored = store.get(&set).unwrap();
    assert_eq!(stored.modified, 7);
    assert_eq!(
        stored.items[1].kind,
        ItemKind::Note {
            text: "Psalm 23".to_owned()
        }
    );
    assert!(
        !store.edit_note(&set, &note, "Psalm 23".to_owned(), 9),
        "unchanged text"
    );
    assert!(!store.edit_note(&set, &a, "x".to_owned(), 9), "a piece");
    assert!(!store.edit_note(&set, "nope", "x".to_owned(), 9));
    assert!(!store.edit_note("nope", &note, "x".to_owned(), 9));
    assert_eq!(store.get(&set).unwrap().modified, 7);
}

#[test]
fn mark_opened_keeps_modified() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    let a = store.add_piece(&set, piece('a'), 2).unwrap();
    assert!(store.mark_opened(&set, &a));
    assert_eq!(
        store.get(&set).unwrap().last_opened_item.as_deref(),
        Some(a.as_str())
    );
    assert_eq!(store.get(&set).unwrap().modified, 2);
    assert!(!store.mark_opened(&set, "nope"));
    assert!(store.remove_item(&set, &a, 3));
    assert_eq!(store.get(&set).unwrap().last_opened_item, None);
}

#[test]
fn delete_removes_set() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    assert!(store.delete(&set));
    assert!(store.get(&set).is_none());
    assert!(!store.delete(&set));
}

#[test]
fn sets_containing_finds_pieces_by_key() {
    let mut store = SetStore::default();
    let one = store.create(fields("One"), 1);
    let two = store.create(fields("Two"), 1);
    store.add_piece(&one, piece('a'), 2);
    store.add_piece(&one, piece('a'), 2);
    store.add_note(&two, "a".repeat(64), 2);
    assert_eq!(store.sets_containing(&"a".repeat(64)), [one]);
    assert!(store.sets_containing(&"b".repeat(64)).is_empty());
}

#[test]
fn json_round_trip_with_notes_and_pieces() {
    let mut store = SetStore::default();
    let set = store.create(
        SetFields {
            name: "Mass".to_owned(),
            date: Some("2026-11-01".to_owned()),
            notes: Some("Bring candles".to_owned()),
        },
        1_793_000_000,
    );
    store.add_piece(&set, piece('a'), 1_793_000_001);
    let note = store
        .add_note(&set, "Pause".to_owned(), 1_793_000_002)
        .unwrap();
    store.mark_opened(&set, &note);
    let json = store.to_json();
    assert!(json.contains(r#""schema_version":1"#));
    assert!(json.contains(r#""kind":"piece""#));
    assert!(json.contains(r#""kind":"note","text":"Pause""#));
    assert_eq!(SetStore::from_json(&json), store);
    assert!(matches!(
        store.get(&set).unwrap().items[0].kind,
        ItemKind::Piece(_)
    ));
}

#[test]
fn corrupt_and_future_version_json_load_empty() {
    assert_eq!(SetStore::from_json("not json"), SetStore::default());
    assert_eq!(SetStore::from_json(""), SetStore::default());
    let future = r#"{"schema_version":2,"sets":[{"id":"a","name":"x","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[]}]}"#;
    assert_eq!(SetStore::from_json(future), SetStore::default());
}

#[test]
fn invalid_piece_keys_are_dropped_on_load() {
    let json = r#"{"schema_version":1,"sets":[{"id":"s","name":"x","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[{"id":"i1","kind":"piece","key":"BAD","path":"a.pdf"},{"id":"i2","kind":"note","text":"keep"}]}]}"#;
    let store = SetStore::from_json(json);
    assert_eq!(item_ids(&store, "s"), ["i2"]);
}

#[test]
fn load_and_save_use_the_file_without_writing_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sets.json");
    assert_eq!(SetStore::load(&path), SetStore::default());
    assert!(!path.exists());
    std::fs::write(&path, r#"{"schema_version":9,"sets":[]}"#).unwrap();
    assert_eq!(SetStore::load(&path), SetStore::default());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        r#"{"schema_version":9,"sets":[]}"#
    );
    let mut store = SetStore::default();
    store.create(fields("Mass"), 1);
    store.save(&path).unwrap();
    assert_eq!(SetStore::load(&path), store);
}

#[test]
fn item_ids_are_unique_within_a_set() {
    let mut store = SetStore::default();
    let set = store.create(fields("Mass"), 1);
    for _ in 0..100 {
        store.add_piece(&set, piece('a'), 2);
    }
    let mut ids = item_ids(&store, &set);
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 100);
}

#[test]
fn new_id_is_sixteen_lowercase_hex() {
    let id = new_id("seed");
    assert_eq!(id.len(), 16);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_ne!(id, new_id("seed"));
}

#[test]
fn relink_by_path_when_key_changes() {
    let mut store = SetStore::default();
    let id = store.create(fields("Mass"), 10);
    store.add_piece(&id, piece('a'), 10);
    store.add_note(&id, "Amen".to_owned(), 10);
    let library = [("b".repeat(64), "a.pdf".to_owned())];
    assert!(store.relink(&library));
    let set = store.get(&id).unwrap();
    assert_eq!(
        set.items[0].kind,
        ItemKind::Piece(PieceRef {
            key: "b".repeat(64),
            path: "a.pdf".to_owned()
        })
    );
    assert!(matches!(set.items[1].kind, ItemKind::Note { .. }));
    assert!(!store.relink(&library));
}

#[test]
fn relink_leaves_missing_entries_in_place() {
    let mut store = SetStore::default();
    let id = store.create(fields("Mass"), 10);
    store.add_piece(&id, piece('a'), 10);
    store.add_piece(&id, piece('c'), 10);
    let before = store.clone();
    let library = [("c".repeat(64), "c.pdf".to_owned())];
    assert!(!store.relink(&library));
    assert_eq!(store, before);
}

#[test]
fn relink_does_not_change_modified() {
    let mut store = SetStore::default();
    let id = store.create(fields("Mass"), 10);
    store.add_piece(&id, piece('a'), 10);
    assert!(store.relink(&[("b".repeat(64), "a.pdf".to_owned())]));
    assert_eq!(store.get(&id).unwrap().modified, 10);
}

#[test]
fn rescan_with_new_key_at_same_path_rewrites_both_files() {
    let row = kyrie_row;
    let saved_keys = |favorites_path: &std::path::Path, sets_path: &std::path::Path, id: &str| {
        let favorites = notae_app_core::favorites::Favorites::load(favorites_path);
        let saved = SetStore::load(sets_path);
        let set = saved.get(id).unwrap();
        let ItemKind::Piece(piece) = &set.items[0].kind else {
            panic!("a piece")
        };
        (
            favorites.items()[0].key.clone(),
            piece.key.clone(),
            set.modified,
        )
    };
    let dir = tempfile::tempdir().unwrap();
    let favorites_path = dir.path().join("favorites.json");
    let sets_path = dir.path().join("sets.json");
    let mut model = UiModel::new();
    model.set_favorites_path(favorites_path.clone());
    model.set_sets_path(sets_path.clone());
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![row('a')]))
        .unwrap();
    model.toggle_favorite(&"a".repeat(64)).unwrap();
    let id = model.create_set(fields("Mass")).unwrap();
    let mut store = SetStore::load(&sets_path);
    store.add_piece(
        &id,
        PieceRef {
            key: "a".repeat(64),
            path: "Kyrie.pdf".to_owned(),
        },
        1,
    );
    store.save(&sets_path).unwrap();
    model.set_sets_path(sets_path.clone());

    // A rescan replaces the PDF: the worker delivers the new key at the same path.
    model
        .apply_operation_event(completed(vec![row('b')]))
        .unwrap();
    assert_eq!(
        saved_keys(&favorites_path, &sets_path, &id),
        ("b".repeat(64), "b".repeat(64), 1),
        "a rescan re-links both files without touching the set's modified time"
    );

    // A search after another replacement re-links the same way.
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![row('c')]))
        .unwrap();
    assert_eq!(
        saved_keys(&favorites_path, &sets_path, &id),
        ("c".repeat(64), "c".repeat(64), 1),
        "search rows re-link both files without touching the set's modified time"
    );
}

#[test]
fn future_version_sets_survive_favorite_changes_and_relink() {
    let dir = tempfile::tempdir().unwrap();
    let favorites_path = dir.path().join("favorites.json");
    let sets_path = dir.path().join("sets.json");
    let future_sets = format!(
        r#"{{"schema_version":2,"sets":[{{"id":"s","name":"x","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[{{"id":"i1","kind":"piece","key":"{}","path":"Kyrie.pdf"}}]}}]}}"#,
        "a".repeat(64)
    );
    std::fs::write(&sets_path, &future_sets).unwrap();
    std::fs::write(&favorites_path, "not json {").unwrap();
    let mut model = UiModel::new();
    model.set_favorites_path(favorites_path.clone());
    model.set_sets_path(sets_path.clone());
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![kyrie_row('a')]))
        .unwrap();

    // Starring is a change to favorites, so the corrupt favorites file may be replaced.
    model.toggle_favorite(&"a".repeat(64)).unwrap();
    // A rescan that replaces the PDF re-links favorites only.
    model
        .apply_operation_event(completed(vec![kyrie_row('b')]))
        .unwrap();

    let favorites = notae_app_core::favorites::Favorites::load(&favorites_path);
    assert_eq!(favorites.items()[0].key, "b".repeat(64));
    assert_eq!(std::fs::read_to_string(&sets_path).unwrap(), future_sets);
}

#[test]
fn future_version_favorites_survive_set_changes_and_relink() {
    let dir = tempfile::tempdir().unwrap();
    let favorites_path = dir.path().join("favorites.json");
    let sets_path = dir.path().join("sets.json");
    let future_favorites = format!(
        r#"{{"schema_version":2,"items":[{{"key":"{}","path":"Kyrie.pdf"}}]}}"#,
        "a".repeat(64)
    );
    std::fs::write(&favorites_path, &future_favorites).unwrap();
    let mut model = UiModel::new();
    model.set_favorites_path(favorites_path.clone());
    model.set_sets_path(sets_path.clone());
    model
        .apply_operation_event(OperationEvent::SearchRows(vec![kyrie_row('a')]))
        .unwrap();

    let id = model.create_set(fields("Mass")).unwrap();
    model.set_membership_toggled(&id, &"a".repeat(64));
    // A rescan that replaces the PDF re-links the set only.
    model
        .apply_operation_event(completed(vec![kyrie_row('b')]))
        .unwrap();

    let saved = SetStore::load(&sets_path);
    let ItemKind::Piece(piece) = &saved.get(&id).unwrap().items[0].kind else {
        panic!("a piece")
    };
    assert_eq!(piece.key, "b".repeat(64));
    assert_eq!(
        std::fs::read_to_string(&favorites_path).unwrap(),
        future_favorites
    );
}
