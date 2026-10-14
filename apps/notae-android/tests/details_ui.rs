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
    assert!(app.contains("root.width >= 1024px"));
    assert!(app.contains("root.width < 1024px"));
}
