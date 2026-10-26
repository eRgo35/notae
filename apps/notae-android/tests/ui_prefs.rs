use notae_android::language::LanguageChoice;
use notae_android::ui::UiModel;
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

fn tempfile_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("notae-ui-prefs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temporary directory is created");
    dir
}
