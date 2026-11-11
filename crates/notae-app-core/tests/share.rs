//! The shared set file and text list.

use notae_app_core::backup::{BackupError, ImportFile, export_json, parse_import};
use notae_app_core::favorites::{Favorites, PieceRef};
use notae_app_core::score_notes::ScoreNotes;
use notae_app_core::sets::{ItemKind, Set};
use notae_app_core::sets::{SetFields, SetStore};
use notae_app_core::share::{
    MatchedSet, SharedSet, keep_both, keep_set_notes, match_pieces, share_file_name, share_json,
    share_text,
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

#[test]
fn share_file_name_caps_the_name_at_100_characters() {
    let long = format!("{}{}", "a".repeat(99), "żółw".repeat(10));
    assert_eq!(
        share_file_name(&long),
        format!("{}ż.notae-set", "a".repeat(99))
    );
    let exact = "ł".repeat(100);
    assert_eq!(share_file_name(&exact), format!("{exact}.notae-set"));
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

/// A set named `name` holding pieces `(key char, path, set note)` in order.
fn set_of(name: &str, pieces: &[(char, &str, Option<&str>)]) -> Set {
    let mut store = SetStore::default();
    let id = store.create(
        SetFields {
            name: name.to_owned(),
            date: None,
            notes: None,
        },
        1,
    );
    for (c, path, note) in pieces {
        let item = store.add_piece(&id, piece(*c, path), 2).expect("set");
        if let Some(note) = note {
            store.edit_set_note(&id, &item, note, 3);
        }
    }
    store.get(&id).expect("set").clone()
}

fn set_notes(set: &Set) -> Vec<Option<&str>> {
    set.items
        .iter()
        .map(|item| item.set_note.as_deref())
        .collect()
}

#[test]
fn replace_keeps_the_local_set_notes_the_file_lacks() {
    let local = set_of(
        "Mass",
        &[
            ('a', "Gloria.pdf", Some("breathe")),
            ('x', "mass/Sanctus.pdf", Some("local")),
            ('y', "Benedictus.pdf", Some("soft")),
            ('c', "Credo.pdf", Some("not in the file")),
        ],
    );
    let mut incoming = set_of(
        "Mass",
        &[
            ('a', "Gloria.pdf", None),
            ('b', "mass/Sanctus.pdf", Some("louder")),
            ('e', "Benedictus.pdf", None),
            ('d', "Agnus.pdf", None),
        ],
    );

    keep_set_notes(&mut incoming, &local);

    assert_eq!(
        set_notes(&incoming),
        [Some("breathe"), Some("louder"), Some("soft"), None],
        "kept by key, the file's note wins, kept by path, no local piece"
    );
}

#[test]
fn replace_pairs_repeated_pieces_in_order() {
    let local = set_of(
        "Mass",
        &[
            ('a', "Gloria.pdf", Some("first")),
            ('a', "Gloria.pdf", Some("second")),
        ],
    );
    let mut incoming = set_of(
        "Mass",
        &[('a', "Gloria.pdf", None), ('a', "Gloria.pdf", None)],
    );

    keep_set_notes(&mut incoming, &local);

    assert_eq!(set_notes(&incoming), [Some("first"), Some("second")]);
}

/// The sender's "Hymns": Missing Hymn (999, `f`, `elsewhere/missing.pdf`), which the recipient lacks, and Sanctus (7,
/// `b`, `mass/Sanctus.pdf`), which the recipient has, matched against the recipient's library.
fn received_hymns() -> MatchedSet {
    let mut store = SetStore::default();
    let id = store.create(
        SetFields {
            name: "Hymns".to_owned(),
            date: None,
            notes: None,
        },
        1,
    );
    store.add_piece(&id, piece('f', "elsewhere/missing.pdf"), 2);
    store.add_piece(&id, piece('b', "mass/Sanctus.pdf"), 3);
    let sender = vec![
        library_row('f', "elsewhere/missing.pdf", Some("999"), "Missing Hymn"),
        row('b', Some("7"), "Sanctus", None),
    ];
    let json = share_json(store.get(&id).expect("set"), &sender, false);
    let Ok(ImportFile::Set(shared)) = parse_import(&json) else {
        panic!("a shared set");
    };
    match_pieces(&shared, &recipient())
}

/// The recipient's library: Sanctus under another number and title, and no Missing Hymn.
fn recipient() -> Vec<SearchRow> {
    vec![library_row(
        'b',
        "mass/Sanctus.pdf",
        Some("8"),
        "Sanctus (choir)",
    )]
}

#[test]
fn an_unmatched_piece_keeps_the_senders_number_and_title() {
    let matched = received_hymns();

    let stored: Vec<_> = matched
        .set
        .items
        .iter()
        .map(|item| (item.number.as_deref(), item.title.as_deref()))
        .collect();
    assert_eq!(
        stored,
        vec![(Some("999"), Some("Missing Hymn")), (None, None)],
        "only the piece the library lacks keeps what the sender called it"
    );

    let entries = set_entries(&recipient(), &matched.set);
    let SetEntry::Piece(hymn) = &entries[0] else {
        panic!("a piece row");
    };
    assert!(hymn.missing);
    assert_eq!(
        (hymn.id.as_deref(), hymn.title.as_str()),
        (Some("999"), "Missing Hymn")
    );
    let SetEntry::Piece(sanctus) = &entries[1] else {
        panic!("a piece row");
    };
    assert_eq!(
        (sanctus.id.as_deref(), sanctus.title.as_str()),
        (Some("8"), "Sanctus (choir)"),
        "a matched piece shows the library row"
    );
}

#[test]
fn a_placeholder_keeps_the_senders_number_and_title_when_shared_on() {
    let matched = received_hymns();

    let text = share_text(&matched.set, &recipient(), false);
    assert!(text.contains("1. 999 – Missing Hymn"), "{text}");

    let json = share_json(&matched.set, &recipient(), false);
    let Ok(ImportFile::Set(again)) = parse_import(&json) else {
        panic!("a shared set");
    };
    let rematched = match_pieces(&again, &[]);
    assert_eq!(
        rematched
            .unmatched
            .iter()
            .map(|piece| (piece.number.as_deref(), piece.title.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (Some("999"), "Missing Hymn"),
            (Some("8"), "Sanctus (choir)")
        ]
    );
    assert_eq!(
        (
            rematched.set.items[0].number.as_deref(),
            rematched.set.items[0].title.as_deref()
        ),
        (Some("999"), Some("Missing Hymn"))
    );
}

#[test]
fn a_relinked_placeholder_shows_the_library_row() {
    let matched = received_hymns();
    let mut rows = recipient();
    rows.push(library_row(
        'f',
        "elsewhere/missing.pdf",
        Some("12"),
        "Hymn found",
    ));

    let entries = set_entries(&rows, &matched.set);
    let SetEntry::Piece(hymn) = &entries[0] else {
        panic!("a piece row");
    };
    assert!(!hymn.missing);
    assert_eq!(
        (hymn.id.as_deref(), hymn.title.as_str()),
        (Some("12"), "Hymn found")
    );
    assert!(share_text(&matched.set, &rows, false).contains("1. 12 – Hymn found"));
}

#[test]
fn a_placeholder_keeps_the_senders_number_and_title_in_the_sets_file() {
    let matched = received_hymns();
    let mut store = SetStore::default();
    store.put(matched.set.clone());

    let json = store.to_json();
    assert!(
        json.contains(r#""number":"999","title":"Missing Hymn""#),
        "{json}"
    );
    assert_eq!(
        json.matches(r#""title""#).count(),
        1,
        "omitted when absent: {json}"
    );
    let loaded = SetStore::from_json(&json);
    assert_eq!(loaded.sets()[0].items, matched.set.items);
}

#[test]
fn a_piece_number_or_title_that_is_not_text_reads_as_absent() {
    let key = key('a');
    let json = format!(
        r#"{{"schema_version":2,"app":"notae","kind":"set","set":{{"id":"s","name":"Mass","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[{{"id":"i1","kind":"piece","key":"{key}","path":"Gloria.pdf","number":123,"title":{{"x":1}}}}]}}}}"#
    );

    let Ok(ImportFile::Set(shared)) = parse_import(&json) else {
        panic!("a shared set");
    };

    let item = &shared.set.items[0];
    assert_eq!(
        (item.number.as_deref(), item.title.as_deref()),
        (None, None)
    );
}
