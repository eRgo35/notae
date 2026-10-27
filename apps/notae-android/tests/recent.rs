use notae_android::recent::{RECENT_LIMIT, RecentScores};
use notae_android::ui::UiModel;
use notae_android::ui::browse::LibrarySection;
use notae_android::ui::search::SearchRow;

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

fn row(c: char, title: &str) -> SearchRow {
    SearchRow {
        id: None,
        title: title.to_owned(),
        composer_or_artist: None,
        tags: Vec::new(),
        status: "Ready".to_owned(),
        relative_path: format!("{title}.pdf"),
        note_key: key(c),
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
    }
}

#[test]
fn record_moves_key_to_front_without_duplicates() {
    let mut recent = RecentScores::default();
    recent.record(&key('a'));
    recent.record(&key('b'));
    recent.record(&key('a'));
    assert_eq!(recent.keys(), [key('a'), key('b')]);
}

#[test]
fn record_keeps_at_most_the_limit() {
    let mut recent = RecentScores::default();
    for index in 0..(RECENT_LIMIT + 5) {
        recent.record(&format!("{index:064x}"));
    }
    assert_eq!(recent.keys().len(), RECENT_LIMIT);
    assert_eq!(recent.keys()[0], format!("{:064x}", RECENT_LIMIT + 4));
}

#[test]
fn from_text_keeps_only_valid_unique_keys() {
    let text = format!(
        "{}\nnot-a-key\n\n{}\n{}\nABC\n",
        key('a'),
        key('b'),
        key('a')
    );
    assert_eq!(RecentScores::from_text(&text).keys(), [key('a'), key('b')]);
}

#[test]
fn save_then_load_round_trips_and_missing_file_is_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("recent.txt");
    assert!(RecentScores::load(&path).keys().is_empty());

    let mut recent = RecentScores::default();
    recent.record(&key('c'));
    recent.save(&path).expect("save recent");
    assert_eq!(RecentScores::load(&path).keys(), [key('c')]);
    assert!(!path.with_extension("tmp").exists());
}

#[test]
fn ui_model_records_opened_scores_and_filters_recent_section() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut model = UiModel::new();
    model.set_recent_path(dir.path().join("recent.txt"));
    model.set_search_rows(vec![row('a', "One"), row('b', "Two")]);
    model.record_opened(&key('b')).expect("record");
    model
        .select_section(LibrarySection::Recent)
        .expect("no preferences file to save");

    let titles: Vec<String> = model
        .visible_rows()
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Two"]);

    let reloaded = notae_android::recent::RecentScores::load(&dir.path().join("recent.txt"));
    assert_eq!(reloaded.keys(), [key('b')]);
}

#[test]
fn selection_is_dropped_when_the_row_disappears() {
    let mut model = UiModel::new();
    model.set_search_rows(vec![row('a', "One")]);
    model.select_note(Some(key('a')));
    assert_eq!(
        model.selected_row().map(|row| row.title),
        Some("One".to_owned())
    );

    model.set_search_rows(vec![row('b', "Two")]);
    assert!(model.selected_row().is_none());
}
