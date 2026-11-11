//! The shared set file and text list.

use notae_app_core::backup::{BackupError, ImportFile, export_json, parse_import};
use notae_app_core::favorites::{Favorites, PieceRef};
use notae_app_core::score_notes::ScoreNotes;
use notae_app_core::sets::{SetFields, SetStore};
use notae_app_core::share::{share_file_name, share_json, share_text};
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
