//! The sets and favorites export file: building, parsing, planning and applying an import.

use notae_app_core::backup::ItemOverlay;
use notae_app_core::backup::{
    Backup, BackupError, ImportFile, ImportPlan, apply_import, export_json, parse_import,
    plan_import, suggested_file_name,
};
use notae_app_core::favorites::{Favorites, PieceRef};
use notae_app_core::overlays::{Overlay, PageOverlay, Stroke, Tool};
use notae_app_core::score_notes::ScoreNotes;
use notae_app_core::sets::{SetFields, SetStore};
use notae_app_core::ui::UiModel;

/// Parses `text` as a backup; any other kind of import file is a failure.
fn parse_backup(text: &str) -> Result<Backup, BackupError> {
    match parse_import(text)? {
        ImportFile::Backup(backup) => Ok(backup),
        ImportFile::Set(_) => Err(BackupError::Invalid),
    }
}

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
fn parse_keeps_the_first_of_repeated_item_ids() {
    let key = key('a');
    let text = format!(
        r#"{{"schema_version":1,"app":"notae","favorites":[],"sets":[{{"id":"s","name":"x","date":null,"notes":null,"modified":1,"last_opened_item":null,"items":[{{"id":"i1","kind":"note","text":"first"}},{{"id":"i1","kind":"piece","key":"{key}","path":"a.pdf"}},{{"id":"i2","kind":"note","text":"second"}}]}}]}}"#
    );
    let backup = parse_backup(&text).expect("parses");
    let ids: Vec<&str> = backup.sets[0]
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(ids, ["i1", "i2"]);
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

    let text = export_json(&favorites, &sets, &ScoreNotes::default(), &[]);
    assert!(
        text.starts_with(r#"{"schema_version":3,"app":"notae","favorites":["#),
        "{text}"
    );
    assert!(text.contains(r#""sets":["#), "{text}");

    let backup = parse_backup(&text).expect("our own export parses");
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
    let mut backup = parse_backup(&export_json(
        &Favorites::default(),
        &theirs,
        &ScoreNotes::default(),
        &[],
    ))
    .expect("parses");
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
    let mut backup = parse_backup(&export_json(
        &favorites(&[piece('c', "c.pdf"), piece('a', "elsewhere.pdf")]),
        &theirs,
        &ScoreNotes::default(),
        &[],
    ))
    .expect("parses");
    let mut copy = sets.get(&replaced).expect("set").clone();
    copy.name = "Zaduszki 2025".to_owned();
    backup.sets.push(copy);

    apply_import(
        &mut mine,
        &mut sets,
        &mut ScoreNotes::default(),
        backup.clone(),
    );
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
    apply_import(&mut mine, &mut sets, &mut ScoreNotes::default(), backup);
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
            r#"{"schema_version":4,"app":"notae","favorites":[],"sets":[]}"#,
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
        assert_eq!(parse_backup(text), Err(error), "{text}");
    }
    let favorites_file = favorites(&[piece('a', "a.pdf")]).to_json();
    assert_eq!(parse_backup(&favorites_file), Err(BackupError::NotNotae));
    assert_eq!(
        parse_backup(r#"{"schema_version":1,"app":"notae","favorites":[],"sets":[]}"#),
        Ok(Backup {
            favorites: Favorites::default(),
            sets: Vec::new(),
            score_notes: ScoreNotes::default(),
            overlays: Vec::new(),
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
    let backup = parse_backup(&text).expect("parses");
    assert_eq!(backup.sets.as_slice(), store.sets());
    assert!(backup.favorites.items().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

fn notes_and_set_with_notes() -> (ScoreNotes, SetStore, String) {
    let mut score_notes = ScoreNotes::default();
    score_notes.set(piece('a', "Psalmy/005.pdf"), "Capo na 2");
    let mut sets = SetStore::default();
    let id = sets.create(fields("Zaduszki"), 100);
    let item = sets
        .add_piece(&id, piece('a', "Psalmy/005.pdf"), 101)
        .expect("set exists");
    assert!(sets.edit_set_note(&id, &item, "Wolniej", 102));
    (score_notes, sets, id)
}

#[test]
fn export_round_trips_score_and_set_notes() {
    let (score_notes, sets, _) = notes_and_set_with_notes();
    let text = export_json(&Favorites::default(), &sets, &score_notes, &[]);
    assert!(
        text.starts_with(r#"{"schema_version":3,"app":"notae""#),
        "{text}"
    );
    let backup = parse_backup(&text).expect("parses");
    assert_eq!(backup.score_notes, score_notes);
    assert_eq!(backup.sets.as_slice(), sets.sets());
    assert_eq!(backup.sets[0].items[0].set_note.as_deref(), Some("Wolniej"));
}

#[test]
fn v1_export_still_imports() {
    let text = r#"{"schema_version":1,"app":"notae","favorites":[],"sets":[]}"#;
    let backup = parse_backup(text).expect("v1 parses");
    assert_eq!(backup.score_notes, ScoreNotes::default());
}

#[test]
fn import_on_clean_install_restores_both_kinds() {
    let (score_notes, sets, id) = notes_and_set_with_notes();
    let text = export_json(&Favorites::default(), &sets, &score_notes, &[]);
    let mut model = UiModel::new();
    model.import_backup(parse_backup(&text).expect("parses"));
    assert_eq!(model.score_note(&key('a')), Some("Capo na 2"));
    let set = model.sets().get(&id).expect("set restored");
    assert_eq!(set.items[0].set_note.as_deref(), Some("Wolniej"));
}

fn drawing(content_key: &str, x: f32) -> Overlay {
    Overlay {
        content_key: content_key.to_owned(),
        pages: vec![PageOverlay {
            page: 0,
            strokes: vec![Stroke {
                tool: Tool::Pen,
                color: 0xFF00_0000,
                width: 0.003,
                points: vec![x, x, x, x],
            }],
            texts: Vec::new(),
        }],
    }
}

/// A model keeping its sets and drawings in `dir`, with set "Zaduszki" holding pieces a and b,
/// each drawn on; returns the model, the set id and the item ids.
fn model_with_drawings(dir: &std::path::Path) -> (UiModel, String, Vec<String>) {
    let mut model = UiModel::new();
    model.set_sets_path(dir.join("sets.json"));
    model.set_overlays_dir(dir.join("overlays"));
    let mut sets = SetStore::default();
    let id = sets.create(fields("Zaduszki"), 100);
    let items: Vec<String> = ['a', 'b']
        .iter()
        .map(|&c| {
            sets.add_piece(&id, piece(c, &format!("{c}.pdf")), 101)
                .expect("set exists")
        })
        .collect();
    sets.save(&dir.join("sets.json")).expect("saved");
    model.set_sets_path(dir.join("sets.json"));
    for (item, x) in items.iter().zip([0.1, 0.2]) {
        model.save_overlay(&id, item, &drawing(&key('a'), x));
    }
    (model, id, items)
}

#[test]
fn export_includes_overlays_as_schema_3() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (model, id, items) = model_with_drawings(dir.path());
    let (_, text) = model.export_sets(100);
    assert!(
        text.starts_with(r#"{"schema_version":3,"app":"notae""#),
        "{text}"
    );
    let backup = parse_backup(&text).expect("parses");
    let mut expected: Vec<ItemOverlay> = items
        .iter()
        .zip([0.1, 0.2])
        .map(|(item, x)| ItemOverlay {
            set_id: id.clone(),
            item_id: item.clone(),
            overlay: drawing(&key('a'), x),
        })
        .collect();
    expected.sort_by(|a, b| a.item_id.cmp(&b.item_id));
    assert_eq!(backup.overlays, expected);
}

#[test]
fn schema_2_backup_still_imports_and_keeps_local_drawings() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (mut model, id, items) = model_with_drawings(dir.path());
    let set = model.sets().get(&id).expect("set").clone();
    let mut store = SetStore::default();
    store.put(set);
    let v3 = export_json(&Favorites::default(), &store, &ScoreNotes::default(), &[]);
    let v2 = v3.replacen(r#""schema_version":3"#, r#""schema_version":2"#, 1);
    let v2 = v2.replace(r#","overlays":[]"#, "");
    assert!(!v2.contains("overlays"), "{v2}");
    model.import_backup(parse_backup(&v2).expect("v2 parses"));
    let overlays = model.overlays().expect("store");
    assert_eq!(overlays.get(&id, &items[0]), Some(drawing(&key('a'), 0.1)));
    assert_eq!(overlays.get(&id, &items[1]), Some(drawing(&key('a'), 0.2)));
}

#[test]
fn imported_drawings_win_and_local_ones_fill_gaps() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (mut model, id, items) = model_with_drawings(dir.path());
    // The file's copy of the set has new item ids for the same pieces.
    let mut set = model.sets().get(&id).expect("set").clone();
    set.items[0].id = "n1".to_owned();
    set.items[1].id = "n2".to_owned();
    let mut store = SetStore::default();
    store.put(set);
    let text = export_json(
        &Favorites::default(),
        &store,
        &ScoreNotes::default(),
        &[ItemOverlay {
            set_id: id.clone(),
            item_id: "n1".to_owned(),
            overlay: drawing(&key('a'), 0.9),
        }],
    );
    model.import_backup(parse_backup(&text).expect("parses"));
    let overlays = model.overlays().expect("store");
    assert_eq!(
        overlays.get(&id, "n1"),
        Some(drawing(&key('a'), 0.9)),
        "the file's drawings win"
    );
    assert_eq!(
        overlays.get(&id, "n2"),
        Some(drawing(&key('a'), 0.2)),
        "local drawings fill gaps"
    );
    assert_eq!(
        overlays.get(&id, &items[0]),
        None,
        "the replaced items' files are gone"
    );
    assert_eq!(overlays.get(&id, &items[1]), None);
}

#[test]
fn a_new_set_takes_the_files_drawings() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut theirs = SetStore::default();
    let id = theirs.create(fields("Wielkanoc"), 200);
    let item = theirs
        .add_piece(&id, piece('c', "c.pdf"), 201)
        .expect("set");
    let text = export_json(
        &Favorites::default(),
        &theirs,
        &ScoreNotes::default(),
        &[
            ItemOverlay {
                set_id: id.clone(),
                item_id: item.clone(),
                overlay: drawing(&key('c'), 0.5),
            },
            ItemOverlay {
                set_id: id.clone(),
                item_id: "not-in-the-set".to_owned(),
                overlay: drawing(&key('c'), 0.5),
            },
        ],
    );
    let mut model = UiModel::new();
    model.set_sets_path(dir.path().join("sets.json"));
    model.set_overlays_dir(dir.path().join("overlays"));
    model.import_backup(parse_backup(&text).expect("parses"));
    let overlays = model.overlays().expect("store");
    assert_eq!(overlays.get(&id, &item), Some(drawing(&key('c'), 0.5)));
    assert_eq!(overlays.get(&id, "not-in-the-set"), None);
}
