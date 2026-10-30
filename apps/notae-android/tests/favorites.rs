use notae_android::favorites::{Favorites, PieceRef};

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

fn piece(c: char, path: &str) -> PieceRef {
    PieceRef {
        key: key(c),
        path: path.to_owned(),
    }
}

#[test]
fn toggle_adds_newest_first() {
    let mut favorites = Favorites::default();
    assert!(favorites.toggle(piece('a', "a.pdf")));
    assert!(favorites.toggle(piece('b', "b.pdf")));
    assert_eq!(
        favorites.items(),
        [piece('b', "b.pdf"), piece('a', "a.pdf")]
    );
    assert!(favorites.contains(&key('a')));
}

#[test]
fn toggle_twice_removes() {
    let mut favorites = Favorites::default();
    assert!(favorites.toggle(piece('a', "a.pdf")));
    assert!(!favorites.toggle(piece('a', "a.pdf")));
    assert!(favorites.items().is_empty());
    assert!(!favorites.contains(&key('a')));
}

#[test]
fn json_round_trip() {
    let mut favorites = Favorites::default();
    favorites.toggle(piece('a', "Psalmy/005.pdf"));
    favorites.toggle(piece('b', "b.pdf"));
    let json = favorites.to_json();
    assert!(json.contains("\"schema_version\":1"));
    assert_eq!(Favorites::from_json(&json), favorites);
}

#[test]
fn invalid_keys_and_repeats_are_dropped_on_load() {
    let json = format!(
        r#"{{"schema_version":1,"items":[
            {{"key":"{a}","path":"a.pdf"}},
            {{"key":"short","path":"x.pdf"}},
            {{"key":"{upper}","path":"y.pdf"}},
            {{"key":"{a}","path":"again.pdf"}},
            {{"key":"{b}","path":"b.pdf"}}]}}"#,
        a = key('a'),
        b = key('b'),
        upper = "A".repeat(64),
    );
    let favorites = Favorites::from_json(&json);
    assert_eq!(
        favorites.items(),
        [piece('a', "a.pdf"), piece('b', "b.pdf")]
    );
}

#[test]
fn corrupt_json_loads_empty() {
    assert!(Favorites::from_json("not json {").items().is_empty());
    assert!(Favorites::from_json("").items().is_empty());
}

#[test]
fn future_schema_version_loads_empty() {
    let json = format!(
        r#"{{"schema_version":2,"items":[{{"key":"{}","path":"a.pdf"}}]}}"#,
        key('a')
    );
    assert!(Favorites::from_json(&json).items().is_empty());
}

#[test]
fn save_writes_through_tmp_and_leaves_no_tmp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("favorites.json");
    let mut favorites = Favorites::default();
    favorites.toggle(piece('a', "a.pdf"));
    favorites.save(&path).unwrap();
    assert_eq!(Favorites::load(&path), favorites);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["favorites.json"]);
    assert!(
        Favorites::load(&dir.path().join("missing.json"))
            .items()
            .is_empty()
    );
}

#[test]
fn merge_keeps_order_and_adds_missing_at_end() {
    let mut mine = Favorites::default();
    mine.toggle(piece('a', "a.pdf"));
    mine.toggle(piece('b', "b.pdf"));
    let mut theirs = Favorites::default();
    theirs.toggle(piece('c', "c.pdf"));
    theirs.toggle(piece('a', "other.pdf"));
    mine.merge(&theirs);
    assert_eq!(
        mine.items(),
        [
            piece('b', "b.pdf"),
            piece('a', "a.pdf"),
            piece('c', "c.pdf")
        ]
    );
}

#[test]
fn relink_by_path_when_key_changes() {
    let mut favorites = Favorites::default();
    favorites.toggle(piece('a', "a.pdf"));
    let library = [(key('b'), "a.pdf".to_owned())];
    assert!(favorites.relink(&library));
    assert_eq!(favorites.items(), [piece('b', "a.pdf")]);
    assert!(!favorites.relink(&library));
}

#[test]
fn relink_leaves_missing_entries_in_place() {
    let mut favorites = Favorites::default();
    favorites.toggle(piece('a', "a.pdf"));
    favorites.toggle(piece('c', "c.pdf"));
    let library = [(key('c'), "c.pdf".to_owned())];
    assert!(!favorites.relink(&library));
    assert_eq!(
        favorites.items(),
        [piece('c', "c.pdf"), piece('a', "a.pdf")]
    );
}
