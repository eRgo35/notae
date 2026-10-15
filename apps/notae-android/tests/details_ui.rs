use std::path::Path;

#[test]
fn app_exposes_score_details_and_two_argument_open_callback() {
    let app = include_str!("../ui/app.slint");
    let details_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/details.slint");
    let details = std::fs::read_to_string(details_path).expect("details.slint exists");

    assert!(details.contains("export component ScoreList"));
    assert!(details.contains("export component ScoreDetails"));
    assert!(app.contains("in property <LibraryRow> selected;"));
    assert!(app.contains("callback selection_cleared();"));
    assert!(app.contains("callback open_pdf_requested(string, string);"));
    assert!(app.contains("ScoreDetails {"));
}

#[test]
fn score_list_stays_hidden_until_details_have_room_for_both_panes() {
    let app = include_str!("../ui/app.slint");
    assert!(app.contains("root.window-width >= 1280px"));
    assert!(app.contains("root.window-width < 1280px"));
}

#[test]
fn scan_lives_in_each_screen_header_and_not_in_app_chrome() {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let read = |name: &str| std::fs::read_to_string(ui.join(name)).expect("slint source exists");
    let app = read("app.slint");
    let library = read("library.slint");
    let search = read("search.slint");
    let setup = read("setup.slint");

    // A scan row in `app.slint` pushes every screen down and leaves Scan off the header.
    assert!(!app.contains("ScanButton"));
    assert!(!app.contains("Scan library"));
    assert!(library.contains("ScanButton {"));
    assert!(search.contains("ScanButton {"));
    assert!(setup.contains("ScanButton {"));
    assert_eq!(
        app.matches("rescan-clicked => { root.rescan_clicked(); }")
            .count(),
        3,
        "Library, Search and Settings each forward Scan"
    );
}

#[test]
fn search_screen_owns_search_and_keyboard_interactions() {
    let search_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/search.slint");
    let search = std::fs::read_to_string(search_path).expect("search.slint exists");
    let app = include_str!("../ui/app.slint");

    assert!(search.contains("export component SearchScreen"));
    assert!(search.contains("query-edited(value)"));
    assert!(search.contains("scope-changed(\"text\")"));
    assert!(!search.contains("property <LibraryRow> row"));
    assert!(app.contains("callback search_closed();"));
    assert!(app.contains("callback search_scope_changed(string);"));
    assert!(app.contains("callback search_result_chosen(string);"));
    assert!(app.contains("event.modifiers.control"));
    assert!(app.contains("Key.Escape"));
}
