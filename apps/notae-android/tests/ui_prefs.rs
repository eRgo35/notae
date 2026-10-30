use notae_android::language::LanguageChoice;
use notae_android::sets::{SetFields, SetStore};
use notae_android::ui::UiModel;
use notae_android::ui::browse::LibrarySection;
use notae_android::ui::search::SearchRow;
use notae_android::ui_prefs::{LibraryView, UiPrefs};

#[test]
fn defaults_are_grid_portrait_collapsed_landscape_expanded() {
    let prefs = UiPrefs::default();
    assert_eq!(prefs.library_view, LibraryView::Grid);
    assert!(prefs.sidebar_collapsed_portrait);
    assert!(!prefs.sidebar_collapsed_landscape);
    assert_eq!(UiPrefs::from_text(""), prefs);
}

#[test]
fn portrait_choice_does_not_change_landscape() {
    let prefs = UiPrefs::from_text("sidebar_collapsed_portrait=false\n");
    assert!(!prefs.sidebar_collapsed_portrait);
    assert!(!prefs.sidebar_collapsed_landscape);
}

#[test]
fn unknown_view_does_not_discard_valid_sidebar_choice() {
    let prefs = UiPrefs::from_text("library_view=unknown\nsidebar_collapsed_landscape=true\n");
    assert_eq!(prefs.library_view, LibraryView::Grid);
    assert!(prefs.sidebar_collapsed_landscape);
}

#[test]
fn invalid_booleans_and_noise_default_individually() {
    let prefs = UiPrefs::from_text(
        "library_view=list\nsidebar_collapsed_portrait=maybe\nnot a pair\n=\nfuture_key=1\nsidebar_collapsed_landscape=true\n",
    );
    assert_eq!(prefs.library_view, LibraryView::List);
    assert!(
        prefs.sidebar_collapsed_portrait,
        "invalid value keeps its default"
    );
    assert!(prefs.sidebar_collapsed_landscape);
}

#[test]
fn whitespace_and_crlf_are_tolerated_and_last_valid_entry_wins() {
    let prefs =
        UiPrefs::from_text(" library_view = list \r\nlibrary_view=grid\r\nlibrary_view=bogus\r\n");
    assert_eq!(prefs.library_view, LibraryView::Grid);
}

#[test]
fn text_round_trips_every_combination() {
    for view in [LibraryView::Grid, LibraryView::List] {
        for portrait in [false, true] {
            for landscape in [false, true] {
                let prefs = UiPrefs {
                    library_view: view,
                    sidebar_collapsed_portrait: portrait,
                    sidebar_collapsed_landscape: landscape,
                    ..UiPrefs::default()
                };
                assert_eq!(UiPrefs::from_text(&prefs.to_text()), prefs);
            }
        }
    }
}

#[test]
fn save_then_load_keeps_list_mode_and_both_orientation_choices() {
    let dir = tempfile_dir("round-trip");
    let path = dir.join("ui-prefs.txt");
    let prefs = UiPrefs {
        library_view: LibraryView::List,
        sidebar_collapsed_portrait: false,
        sidebar_collapsed_landscape: true,
        ..UiPrefs::default()
    };
    prefs.save(&path).expect("preferences are saved");
    assert_eq!(UiPrefs::load(&path), prefs);
    assert!(
        !path.with_extension("tmp").exists(),
        "the temporary file is renamed away"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_or_unreadable_files_load_defaults() {
    let dir = tempfile_dir("missing");
    assert_eq!(UiPrefs::load(&dir.join("absent.txt")), UiPrefs::default());
    // A directory in place of the file is unreadable.
    assert_eq!(UiPrefs::load(&dir), UiPrefs::default());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn corrupt_bytes_load_defaults() {
    let dir = tempfile_dir("corrupt");
    let path = dir.join("ui-prefs.txt");
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0x9f]).expect("fixture written");
    assert_eq!(UiPrefs::load(&path), UiPrefs::default());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_failure_is_reported_and_leaves_the_value_with_the_caller() {
    let dir = tempfile_dir("failure");
    let missing_parent = dir.join("no-such-dir").join("ui-prefs.txt");
    let prefs = UiPrefs {
        library_view: LibraryView::List,
        ..UiPrefs::default()
    };
    assert!(prefs.save(&missing_parent).is_err());
    assert_eq!(prefs.library_view, LibraryView::List);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn model_persists_user_changes_and_a_restart_restores_them() {
    let dir = tempfile_dir("model");
    let path = dir.join("ui-prefs.txt");
    let mut model = UiModel::new();
    model.set_ui_prefs_path(path.clone());
    assert_eq!(model.ui_prefs(), &UiPrefs::default());
    assert!(!path.exists(), "loading alone writes nothing");

    model.set_library_view(LibraryView::List).expect("saved");
    model.set_sidebar_collapsed(true, false).expect("saved");
    model.set_language(LanguageChoice::Polish).expect("saved");

    let mut restarted = UiModel::new();
    restarted.set_ui_prefs_path(path.clone());
    let prefs = restarted.ui_prefs();
    assert_eq!(prefs.library_view, LibraryView::List);
    assert_eq!(prefs.language, LanguageChoice::Polish, "language restored");
    assert!(
        !prefs.sidebar_collapsed_portrait,
        "portrait choice restored"
    );
    assert!(
        !prefs.sidebar_collapsed_landscape,
        "landscape keeps its own default"
    );

    restarted.set_sidebar_collapsed(false, true).expect("saved");
    let mut third = UiModel::new();
    third.set_ui_prefs_path(path);
    assert!(!third.ui_prefs().sidebar_collapsed_portrait);
    assert!(third.ui_prefs().sidebar_collapsed_landscape);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unchanged_choices_do_not_rewrite_the_file() {
    let dir = tempfile_dir("unchanged");
    let path = dir.join("ui-prefs.txt");
    let mut model = UiModel::new();
    model.set_ui_prefs_path(path.clone());
    model.set_library_view(LibraryView::Grid).expect("no-op");
    model.set_sidebar_collapsed(true, true).expect("no-op");
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn model_keeps_the_choice_when_saving_fails() {
    let dir = tempfile_dir("model-failure");
    let mut model = UiModel::new();
    model.set_ui_prefs_path(dir.join("no-such-dir").join("ui-prefs.txt"));
    assert!(model.set_library_view(LibraryView::List).is_err());
    assert_eq!(model.ui_prefs().library_view, LibraryView::List);
    assert!(model.set_sidebar_collapsed(true, false).is_err());
    assert!(!model.ui_prefs().sidebar_collapsed_portrait);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn section_round_trips() {
    assert_eq!(UiPrefs::default().section, "all");
    let prefs = UiPrefs::from_text("section=set:0123456789abcdef\n");
    assert_eq!(prefs.section, "set:0123456789abcdef");
    assert_eq!(UiPrefs::from_text(&prefs.to_text()), prefs);

    // Through the model: the section on display survives a restart.
    let dir = tempfile_dir("section");
    let path = dir.join("ui-prefs.txt");
    let mut model = UiModel::new();
    model.set_ui_prefs_path(path.clone());
    model.select_section(LibrarySection::Sets).expect("saved");
    let mut restarted = UiModel::new();
    restarted.set_ui_prefs_path(path);
    assert_eq!(restarted.section(), &LibrarySection::Sets);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_set_falls_back_to_sets() {
    let dir = tempfile_dir("missing-set");
    let sets_path = dir.join("sets.json");
    let prefs_path = dir.join("ui-prefs.txt");
    let mut store = SetStore::default();
    let kept = store.create(
        SetFields {
            name: "Wszystkich Świętych".to_owned(),
            date: None,
            notes: None,
        },
        10,
    );
    store.save(&sets_path).expect("sets are saved");

    // The open set still exists: Notae comes back to it.
    std::fs::write(&prefs_path, format!("section=set:{kept}\n")).expect("prefs are written");
    let mut model = UiModel::new();
    model.set_sets_path(sets_path.clone());
    model.set_ui_prefs_path(prefs_path.clone());
    assert_eq!(model.section(), &LibrarySection::Set(kept));

    // The set was deleted while Notae was closed: the list of sets, not a blank screen.
    std::fs::write(&prefs_path, "section=set:ffffffffffffffff\n").expect("prefs are written");
    let mut model = UiModel::new();
    model.set_sets_path(sets_path);
    model.set_ui_prefs_path(prefs_path);
    assert_eq!(model.section(), &LibrarySection::Sets);
    let _ = std::fs::remove_dir_all(dir);
}

fn row(composer: &str) -> SearchRow {
    SearchRow {
        id: None,
        title: "Pavane".to_owned(),
        composer_or_artist: Some(composer.to_owned()),
        tags: Vec::new(),
        status: "Ready".to_owned(),
        relative_path: "Pavane.pdf".to_owned(),
        note_key: "d".repeat(64),
        text_pending: false,
        title_origin: "From filename".to_owned(),
        composer_origin: String::new(),
        text_snippet: None,
        item_id: None,
        missing: false,
    }
}

#[test]
fn persisted_composer_waits_for_rows_before_falling_back_to_all() {
    let dir = tempfile_dir("composer");
    let path = dir.join("ui-prefs.txt");
    std::fs::write(&path, "section=composer:Gabriel Fauré\n").expect("prefs are written");
    let mut model = UiModel::new();
    model.set_ui_prefs_path(path);
    let faure = LibrarySection::Composer("Gabriel Fauré".to_owned());
    assert_eq!(model.section(), &faure, "rows load later; keep the choice");
    model.set_search_rows(Vec::new());
    assert_eq!(model.section(), &faure, "an empty library proves nothing");
    model.set_search_rows(vec![row("gabriel fauré")]);
    assert_eq!(model.section(), &faure, "matched case-insensitively");
    model.set_search_rows(vec![row("Claude Debussy")]);
    assert_eq!(model.section(), &LibrarySection::All);
    let _ = std::fs::remove_dir_all(dir);
}

fn tempfile_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("notae-ui-prefs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temporary directory is created");
    dir
}
