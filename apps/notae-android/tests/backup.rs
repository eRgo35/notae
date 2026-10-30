//! The sets and favorites export file: building, parsing, planning and applying an import.

use notae_android::backup::{
    Backup, BackupError, ImportPlan, apply_import, export_json, parse_export, plan_import,
    suggested_file_name,
};
use notae_android::favorites::{Favorites, PieceRef};
use notae_android::sets::{SetFields, SetStore};
use notae_android::ui::UiModel;

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

fn piece(c: char, path: &str) -> PieceRef {
    PieceRef {
        key: key(c),
        path: path.to_owned(),
    }
}

fn fields(name: &str) -> SetFields {
    SetFields {
        name: name.to_owned(),
        date: None,
        notes: None,
    }
}

fn favorites(pieces: &[PieceRef]) -> Favorites {
    let mut favorites = Favorites::default();
    for piece in pieces.iter().rev() {
        favorites.toggle(piece.clone());
    }
    favorites
}

#[test]
fn export_then_parse_round_trips() {
    let favorites = favorites(&[piece('a', "Psalmy/005.pdf"), piece('b', "b.pdf")]);
    let mut sets = SetStore::default();
    let first = sets.create(
        SetFields {
            name: "Wszystkich Świętych".to_owned(),
            date: Some("2026-11-01".to_owned()),
            notes: Some("Organy".to_owned()),
        },
        100,
    );
    let repeat = sets
        .add_piece(&first, piece('a', "Psalmy/005.pdf"), 101)
        .expect("set exists");
    sets.add_note(&first, "Cisza".to_owned(), 102);
    sets.add_piece(&first, piece('a', "Psalmy/005.pdf"), 103);
    sets.mark_opened(&first, &repeat);
    sets.create(fields("Zaduszki"), 200);

    let text = export_json(&favorites, &sets);
    assert!(
        text.starts_with(r#"{"schema_version":1,"app":"notae","favorites":["#),
        "{text}"
    );
    assert!(text.contains(r#""sets":["#), "{text}");

    let backup = parse_export(&text).expect("our own export parses");
    assert_eq!(backup.favorites, favorites);
    assert_eq!(backup.sets.as_slice(), sets.sets());
}

#[test]
fn plan_counts_new_and_names_replaced() {
    let mut current = SetStore::default();
    let kept = current.create(fields("Zaduszki"), 100);
    current.create(fields("Adwent"), 100);
    let mut theirs = SetStore::default();
    theirs.create(fields("Boże Narodzenie"), 200);
    theirs.create(fields("Wielkanoc"), 200);
    let mut backup = parse_export(&export_json(&Favorites::default(), &theirs)).expect("parses");
    let mut copy = current.get(&kept).expect("set").clone();
    copy.name = "Zaduszki 2025".to_owned();
    backup.sets.push(copy);

    assert_eq!(
        plan_import(&current, &backup),
        ImportPlan {
            new_sets: 2,
            replaced: vec!["Zaduszki".to_owned()],
        }
    );
}

#[test]
fn apply_merges_favorites_without_duplicates() {
    let mut mine = favorites(&[piece('a', "a.pdf"), piece('b', "b.pdf")]);
    let mut sets = SetStore::default();
    let replaced = sets.create(fields("Zaduszki"), 100);
    let untouched = sets.create(fields("Adwent"), 100);

    let mut theirs = SetStore::default();
    let new = theirs.create(fields("Wielkanoc"), 200);
    let mut backup = parse_export(&export_json(
        &favorites(&[piece('c', "c.pdf"), piece('a', "elsewhere.pdf")]),
        &theirs,
    ))
    .expect("parses");
    let mut copy = sets.get(&replaced).expect("set").clone();
    copy.name = "Zaduszki 2025".to_owned();
    backup.sets.push(copy);

    apply_import(&mut mine, &mut sets, backup.clone());
    assert_eq!(
        mine.items(),
        [
            piece('a', "a.pdf"),
            piece('b', "b.pdf"),
            piece('c', "c.pdf")
        ]
    );
    assert_eq!(sets.sets().len(), 3);
    assert_eq!(sets.get(&replaced).expect("kept id").name, "Zaduszki 2025");
    assert_eq!(sets.get(&untouched).expect("untouched").name, "Adwent");
    assert_eq!(sets.get(&new).expect("added").name, "Wielkanoc");

    // Importing the same file again changes nothing.
    let (before_favorites, before_sets) = (mine.clone(), sets.clone());
    apply_import(&mut mine, &mut sets, backup);
    assert_eq!(mine, before_favorites);
    assert_eq!(sets, before_sets);
}

#[test]
fn rejects_other_json_and_future_versions() {
    let other: [(&str, BackupError); 7] = [
        ("not json", BackupError::Invalid),
        ("[1, 2]", BackupError::NotNotae),
        (r#"{"schema_version":1,"items":[]}"#, BackupError::NotNotae),
        (
            r#"{"schema_version":1,"app":"other","favorites":[],"sets":[]}"#,
            BackupError::NotNotae,
        ),
        (
            r#"{"schema_version":2,"app":"notae","favorites":[],"sets":[]}"#,
            BackupError::UnsupportedVersion,
        ),
        (
            r#"{"app":"notae","favorites":[],"sets":[]}"#,
            BackupError::Invalid,
        ),
        (
            r#"{"schema_version":1,"app":"notae","favorites":"x","sets":[]}"#,
            BackupError::Invalid,
        ),
    ];
    for (text, error) in other {
        assert_eq!(parse_export(text), Err(error), "{text}");
    }
    let favorites_file = favorites(&[piece('a', "a.pdf")]).to_json();
    assert_eq!(parse_export(&favorites_file), Err(BackupError::NotNotae));
    assert_eq!(
        parse_export(r#"{"schema_version":1,"app":"notae","favorites":[],"sets":[]}"#),
        Ok(Backup {
            favorites: Favorites::default(),
            sets: Vec::new(),
        })
    );
}

#[test]
fn suggested_name_is_the_utc_date() {
    // 2026-10-30 23:59 UTC.
    assert_eq!(
        suggested_file_name(1_793_404_740),
        "notae-sets-2026-10-30.json"
    );
    assert_eq!(suggested_file_name(0), "notae-sets-1970-01-01.json");
    // 2028-02-29 12:00 UTC.
    assert_eq!(
        suggested_file_name(1_835_438_400),
        "notae-sets-2028-02-29.json"
    );
}

#[test]
fn ui_model_exports_both_stores_with_the_suggested_name() {
    let dir = std::env::temp_dir().join(format!("notae-backup-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temporary directory is created");
    let path = dir.join("sets.json");
    let mut store = SetStore::default();
    store.create(fields("Zaduszki"), 100);
    store.save(&path).expect("saved");
    let mut model = UiModel::new();
    model.set_sets_path(path);

    let (name, text) = model.export_sets(1_793_404_740);
    assert_eq!(name, "notae-sets-2026-10-30.json");
    let backup = parse_export(&text).expect("parses");
    assert_eq!(backup.sets.as_slice(), store.sets());
    assert!(backup.favorites.items().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
