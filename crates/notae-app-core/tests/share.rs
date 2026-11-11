//! The shared set file and text list.

use notae_app_core::backup::{BackupError, ImportFile, export_json, parse_import};
use notae_app_core::favorites::{Favorites, PieceRef};
use notae_app_core::score_notes::ScoreNotes;
use notae_app_core::sets::{ItemKind, Set};
use notae_app_core::sets::{SetFields, SetStore};
use notae_app_core::share::{
    MatchedSet, SharedSet, keep_both, match_pieces, share_file_name, share_json, share_text,
};
use notae_app_core::ui::browse::{SetEntry, set_entries};
use notae_app_core::ui::search::SearchRow;

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

fn row(c: char, number: Option<&str>, title: &str, composer: Option<&str>) -> SearchRow {
    SearchRow {
        id: number.map(str::to_owned),
        title: title.to_owned(),
        composer_or_artist: composer.map(str::to_owned),
        tags: Vec::new(),
        status: notae_app_core::text::RowStatus::Ready,
        relative_path: format!("{title}.pdf"),
        note_key: key(c),
        text_pending: false,
        title_origin: notae_domain::MetadataOrigin::Unset,
        composer_origin: notae_domain::MetadataOrigin::Unset,
        text_snippet: None,
        item_id: None,
        position: None,
        missing: false,
        set_note: None,
    }
}

fn piece(c: char, path: &str) -> PieceRef {
    PieceRef {
        key: key(c),
        path: path.to_owned(),
    }
}

/// "Sunday": Gloria (123, Bach, set note), a text note, then Alleluia, which the library lacks.
fn sunday() -> (SetStore, String, Vec<SearchRow>) {
    let mut store = SetStore::default();
    let id = store.create(
        SetFields {
            name: "Sunday".to_owned(),
            date: Some("2026-11-15".to_owned()),
            notes: Some("Bring pencils".to_owned()),
        },
        1,
    );
    let gloria = store
        .add_piece(&id, piece('a', "Gloria.pdf"), 2)
        .expect("set");
    store.add_note(&id, "Kyrie after".to_owned(), 3);
    store.add_piece(&id, piece('b', "mass/Alleluia.pdf"), 4);
    store.edit_set_note(&id, &gloria, "slowly", 5);
    let rows = vec![row('a', Some("123"), "Gloria", Some("Bach"))];
    (store, id, rows)
}

#[test]
fn share_json_has_one_set_and_no_private_fields() {
    let (store, id, rows) = sunday();
    let set = store.get(&id).expect("set");

    let without: serde_json::Value =
        serde_json::from_str(&share_json(set, &rows, false)).expect("json");
    assert_eq!(without["kind"], "set");
    assert_eq!(without["app"], "notae");
    assert_eq!(without["schema_version"], 2);
    assert!(without.get("favorites").is_none());
    assert!(without.get("score_notes").is_none());
    assert!(without["set"]["last_opened_item"].is_null());
    assert!(!without.to_string().contains("set_note"));
    let first = &without["set"]["items"][0];
    assert_eq!(first["number"], "123");
    assert_eq!(first["title"], "Gloria");
    assert_eq!(first["composer"], "Bach");
    assert_eq!(first["key"], key('a'));

    let with: serde_json::Value =
        serde_json::from_str(&share_json(set, &rows, true)).expect("json");
    assert_eq!(with["set"]["items"][0]["set_note"], "slowly");
}

#[test]
fn share_text_lists_the_set() {
    let (store, id, rows) = sunday();
    let set = store.get(&id).expect("set");
    assert_eq!(
        share_text(set, &rows, false),
        "Sunday · 2026-11-15\n1. 123 – Gloria (Bach)\n   Kyrie after\n2. Alleluia"
    );
    assert_eq!(
        share_text(set, &rows, true),
        "Sunday · 2026-11-15\n1. 123 – Gloria (Bach)\n   – slowly\n   Kyrie after\n2. Alleluia"
    );
}

#[test]
fn parse_export_tells_backup_from_shared_set() {
    let (store, id, rows) = sunday();
    let set = store.get(&id).expect("set");

    let backup = export_json(&Favorites::default(), &store, &ScoreNotes::default());
    assert!(matches!(parse_import(&backup), Ok(ImportFile::Backup(_))));

    let Ok(ImportFile::Set(shared)) = parse_import(&share_json(set, &rows, true)) else {
        panic!("a shared set");
    };
    assert_eq!(shared.set.name, "Sunday");
    assert_eq!(shared.set.items.len(), 3);
    assert_eq!(shared.set.items[0].set_note.as_deref(), Some("slowly"));
    let gloria = &shared.pieces[&shared.set.items[0].id];
    assert_eq!(gloria.number.as_deref(), Some("123"));
    assert_eq!(gloria.title, "Gloria");
    assert_eq!(gloria.composer.as_deref(), Some("Bach"));
    let alleluia = &shared.pieces[&shared.set.items[2].id];
    assert_eq!(
        (alleluia.number.clone(), alleluia.title.as_str()),
        (None, "Alleluia")
    );
    assert!(!shared.pieces.contains_key(&shared.set.items[1].id));

    assert_eq!(
        parse_import(r#"{"schema_version":2,"app":"notae","kind":"other"}"#),
        Err(BackupError::NotNotae)
    );
}

#[test]
fn share_file_name_sanitises() {
    assert_eq!(share_file_name("A/B: Mass"), "A_B_ Mass.notae-set");
    assert_eq!(share_file_name("  "), "set.notae-set");
}

/// The sender's "Mass": Gloria (123, `a`, `Gloria.pdf`) and Sanctus (7, `b`, `mass/Sanctus.pdf`), as received.
fn received_mass() -> SharedSet {
    let mut store = SetStore::default();
    let id = store.create(
        SetFields {
            name: "Mass".to_owned(),
            date: None,
            notes: None,
        },
        1,
    );
    store.add_piece(&id, piece('a', "Gloria.pdf"), 2);
    store.add_piece(&id, piece('b', "mass/Sanctus.pdf"), 3);
    let sender = vec![
        row('a', Some("123"), "Gloria", None),
        row('b', Some("7"), "Sanctus", None),
    ];
    let json = share_json(store.get(&id).expect("set"), &sender, false);
    let Ok(ImportFile::Set(shared)) = parse_import(&json) else {
        panic!("a shared set");
    };
    shared
}

/// A recipient's library row with its own key, path, number and title.
fn library_row(c: char, path: &str, number: Option<&str>, title: &str) -> SearchRow {
    SearchRow {
        relative_path: path.to_owned(),
        ..row(c, number, title, None)
    }
}

/// The `(key, path)` of each piece item, in set order.
fn pieces_of(set: &Set) -> Vec<(String, String)> {
    set.items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Piece(piece) => Some((piece.key.clone(), piece.path.clone())),
            ItemKind::Note { .. } => None,
        })
        .collect()
}

#[test]
fn matches_identical_files_by_key() {
    let shared = received_mass();
    let rows = vec![
        library_row('a', "moved/Gloria.pdf", None, "Other"),
        library_row('b', "Sanctus.pdf", Some("7"), "Sanctus"),
    ];

    let MatchedSet {
        set,
        matched,
        unmatched,
    } = match_pieces(&shared, &rows);

    assert_eq!(matched, 2);
    assert!(unmatched.is_empty());
    assert_eq!(
        pieces_of(&set),
        vec![
            (key('a'), "moved/Gloria.pdf".to_owned()),
            (key('b'), "Sanctus.pdf".to_owned())
        ]
    );
}

#[test]
fn matches_renamed_files_by_path_then_number_and_title() {
    let shared = received_mass();
    let rows = vec![
        // Same number and title as Gloria, but Gloria's path below wins.
        library_row('e', "other/Gloria.pdf", Some("123"), "Gloria"),
        library_row('c', "Gloria.pdf", None, "Gloria (new edition)"),
        library_row('d', "renamed/Sanctus 2.pdf", Some("7"), "  SANCTUS "),
    ];

    let matched = match_pieces(&shared, &rows);

    assert_eq!(matched.matched, 2);
    assert_eq!(
        pieces_of(&matched.set),
        vec![
            (key('c'), "Gloria.pdf".to_owned()),
            (key('d'), "renamed/Sanctus 2.pdf".to_owned())
        ]
    );
}

#[test]
fn same_number_and_title_different_file_matches() {
    let shared = received_mass();
    let rows = vec![
        // Without a number a title alone never matches.
        library_row('c', "Gloria.v2.pdf", None, "Gloria"),
        library_row('d', "s.pdf", Some("7"), "sanctus"),
        library_row('e', "g.pdf", Some("124"), "Gloria"),
    ];

    let matched = match_pieces(&shared, &rows);

    assert_eq!(matched.matched, 1);
    assert_eq!(pieces_of(&matched.set)[1], (key('d'), "s.pdf".to_owned()));
    assert_eq!(
        pieces_of(&matched.set)[0],
        (key('a'), "Gloria.pdf".to_owned())
    );
    assert_eq!(matched.unmatched.len(), 1);
    assert_eq!(matched.unmatched[0].title, "Gloria");
    assert_eq!(matched.unmatched[0].number.as_deref(), Some("123"));
}

#[test]
fn unmatched_pieces_stay_as_placeholders() {
    let shared = received_mass();
    let rows = vec![library_row('b', "mass/Sanctus.pdf", Some("7"), "Sanctus")];

    let matched = match_pieces(&shared, &rows);

    assert_eq!(matched.matched, 1);
    assert_eq!(
        matched
            .unmatched
            .iter()
            .map(|piece| (piece.number.as_deref(), piece.title.as_str()))
            .collect::<Vec<_>>(),
        vec![(Some("123"), "Gloria")]
    );
    assert_eq!(matched.set.items.len(), 2, "the unmatched piece is kept");
    assert_eq!(
        pieces_of(&matched.set)[0],
        (key('a'), "Gloria.pdf".to_owned()),
        "the sender's key and path"
    );
    let entries = set_entries(&rows, &matched.set);
    let SetEntry::Piece(gloria) = &entries[0] else {
        panic!("a piece row");
    };
    assert!(gloria.missing);
    assert_eq!(gloria.title, "Gloria");
    let SetEntry::Piece(sanctus) = &entries[1] else {
        panic!("a piece row");
    };
    assert!(!sanctus.missing);
}

#[test]
fn keep_both_names_and_ids() {
    let shared = received_mass();
    let mut store = SetStore::default();
    store.put(shared.set.clone());
    let mut second = shared.set.clone();
    second.id = "other".to_owned();
    second.name = "Mass (2)".to_owned();
    store.put(second);

    let copy = keep_both(shared.set.clone(), &store);

    assert_eq!(copy.name, "Mass (3)");
    assert_ne!(copy.id, shared.set.id);
    assert!(store.get(&copy.id).is_none());
    assert_eq!(copy.items.len(), shared.set.items.len());
    let mut ids: Vec<&str> = copy.items.iter().map(|item| item.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), copy.items.len(), "item ids stay unique");
}
