//! Behaviour of the generated Library UI: layout toggle and collapsible sidebar.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use notae_android::{AppWindow, LibraryRow, SectionItem};
use slint::{ComponentHandle, LogicalSize, ModelRc, SharedString, VecModel};
use support::{app, app_sized};

fn score(index: usize) -> LibraryRow {
    LibraryRow {
        id: SharedString::from(format!("{index}")),
        title: SharedString::from(format!("Score {index}")),
        composer_or_artist: SharedString::from(format!("Composer {index}")),
        tags: SharedString::from(if index == 1 { "folk, choir" } else { "" }),
        note_key: SharedString::from(format!("{index:064}")),
        relative_path: SharedString::from(format!("score-{index}.pdf")),
        text_pending: index == 2,
        ..LibraryRow::default()
    }
}

fn with_library(app: &AppWindow, count: usize) {
    let rows: Vec<LibraryRow> = (1..=count).map(score).collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
    let section = |key: &str, label: &str, count: i32| SectionItem {
        key: key.into(),
        label: label.into(),
        count,
        kind: "".into(),
    };
    app.set_sections(ModelRc::new(VecModel::from(vec![
        section("all", "All scores", count as i32),
        section("recent", "Recent", 0),
    ])));
}

fn count(app: &AppWindow, type_name: &str) -> usize {
    ElementHandle::find_by_element_type_name(app, type_name).count()
}

fn labelled(app: &AppWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(app, label).next()
}

fn press(app: &AppWindow, label: &str) {
    labelled(app, label)
        .unwrap_or_else(|| panic!("`{label}` is exposed"))
        .invoke_accessible_default_action();
}

#[test]
fn toggle_switches_between_grid_and_list_and_reports_the_choice() {
    let app = app();
    with_library(&app, 3);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&changes);
    app.on_library_view_changed(move |view| recorded.borrow_mut().push(view.to_string()));

    assert_eq!((count(&app, "ScoreCard"), count(&app, "ScoreRow")), (3, 0));
    assert!(labelled(&app, "Switch to grid view").is_none());

    press(&app, "Switch to list view");
    assert!(app.get_library_list_view());
    assert_eq!((count(&app, "ScoreCard"), count(&app, "ScoreRow")), (0, 3));
    assert!(labelled(&app, "Switch to list view").is_none());

    press(&app, "Switch to grid view");
    assert!(!app.get_library_list_view());
    assert_eq!((count(&app, "ScoreCard"), count(&app, "ScoreRow")), (3, 0));
    assert_eq!(*changes.borrow(), ["list", "grid"]);
}

#[test]
fn toggle_is_a_48px_target_in_both_modes() {
    let app = app();
    with_library(&app, 1);
    for label in ["Switch to list view", "Switch to grid view"] {
        let toggle = labelled(&app, label).expect("toggle is exposed");
        assert!(toggle.size().width >= 48.0 && toggle.size().height >= 48.0);
        toggle.invoke_accessible_default_action();
    }
}

#[test]
fn list_rows_show_every_row_in_order_and_select_by_stable_key() {
    let app = app();
    app.set_library_list_view(true);
    with_library(&app, 4);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&selected);
    app.on_score_selected(move |key| recorded.borrow_mut().push(key.to_string()));

    let mut previous_y = f32::MIN;
    for index in 1..=4 {
        let row = labelled(&app, &format!("Score {index}")).expect("row is exposed");
        assert!(row.size().height >= 48.0, "row is a 48px touch target");
        assert!(
            row.absolute_position().y > previous_y,
            "rows keep model order"
        );
        previous_y = row.absolute_position().y;
    }
    press(&app, "Score 3");
    assert_eq!(*selected.borrow(), [format!("{:064}", 3)]);
}

#[test]
fn list_row_describes_composer_and_pending_text_like_the_card() {
    let app = app();
    app.set_library_list_view(true);
    with_library(&app, 2);
    let row = labelled(&app, "Score 2").expect("row is exposed");
    assert_eq!(
        row.accessible_description().as_deref(),
        Some("Composer 2. Text pending")
    );
}

#[test]
fn list_virtualizes_long_libraries() {
    let app = app();
    app.set_library_list_view(true);
    with_library(&app, 5_000);
    assert!(
        count(&app, "ScoreRow") < 100,
        "only visible rows are instantiated, found {}",
        count(&app, "ScoreRow")
    );
}

#[test]
fn grid_and_list_preserve_the_same_rows_and_section_selection() {
    let app = app();
    with_library(&app, 3);
    app.set_selected_section("recent".into());
    let sections = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&sections);
    app.on_section_selected(move |key| recorded.borrow_mut().push(key.to_string()));
    for list in [false, true] {
        app.set_library_list_view(list);
        for index in 1..=3 {
            assert!(labelled(&app, &format!("Score {index}")).is_some());
        }
    }
    assert_eq!(app.get_selected_section(), "recent");
}

#[test]
fn landscape_defaults_to_a_visible_sidebar_that_can_be_hidden_and_restored() {
    let app = app();
    with_library(&app, 2);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&changes);
    app.on_sidebar_collapsed_changed(move |portrait, collapsed| {
        recorded.borrow_mut().push((portrait, collapsed));
    });
    assert_eq!((count(&app, "SidePanel"), count(&app, "Chip")), (1, 0));

    press(&app, "Hide sidebar");
    assert_eq!((count(&app, "SidePanel"), count(&app, "Chip")), (0, 2));
    assert!(
        labelled(&app, "Settings").is_some(),
        "Settings stays reachable from the header"
    );
    press(&app, "Show sidebar");
    assert_eq!((count(&app, "SidePanel"), count(&app, "Chip")), (1, 0));
    assert_eq!(*changes.borrow(), [(false, true), (false, false)]);
}

#[test]
fn portrait_defaults_to_collapsed_with_sections_reachable_as_chips() {
    let app = app_sized(900, 1400);
    with_library(&app, 2);
    assert_eq!((count(&app, "SidePanel"), count(&app, "Chip")), (0, 2));
    let toggle = labelled(&app, "Show sidebar").expect("toggle is exposed at tablet width");
    assert!(toggle.size().width >= 48.0 && toggle.size().height >= 48.0);
    let sections = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&sections);
    app.on_section_selected(move |key| recorded.borrow_mut().push(key.to_string()));
    press(&app, "Recent");
    assert_eq!(*sections.borrow(), ["recent"]);
}

#[test]
fn portrait_and_landscape_choices_are_independent() {
    let app = app_sized(900, 1400);
    with_library(&app, 2);
    press(&app, "Show sidebar");
    assert_eq!(count(&app, "SidePanel"), 1);
    assert!(!app.get_sidebar_collapsed_portrait());
    assert!(!app.get_sidebar_collapsed_landscape());

    // Rotate: landscape keeps its own default, and hiding it there leaves portrait alone.
    app.window().set_size(LogicalSize::new(1280.0, 800.0));
    assert_eq!(count(&app, "SidePanel"), 1);
    press(&app, "Hide sidebar");
    assert!(app.get_sidebar_collapsed_landscape());
    assert!(!app.get_sidebar_collapsed_portrait());
    assert_eq!(count(&app, "SidePanel"), 0);

    app.window().set_size(LogicalSize::new(900.0, 1400.0));
    assert_eq!(
        count(&app, "SidePanel"),
        1,
        "portrait choice was not changed"
    );
}

#[test]
fn saved_preferences_set_before_the_first_layout_are_honoured() {
    let app = app_sized(900, 1400);
    app.set_sidebar_collapsed_portrait(false);
    with_library(&app, 1);
    assert_eq!(count(&app, "SidePanel"), 1);
    assert!(labelled(&app, "Hide sidebar").is_some());
}

#[test]
fn compact_width_ignores_the_sidebar_choice_and_offers_no_toggle() {
    let app = app_sized(700, 600);
    app.set_sidebar_collapsed_landscape(false);
    with_library(&app, 2);
    assert_eq!((count(&app, "SidePanel"), count(&app, "Chip")), (0, 2));
    assert!(labelled(&app, "Hide sidebar").is_none());
    assert!(labelled(&app, "Show sidebar").is_none());
    assert!(labelled(&app, "Settings").is_some());
}

#[test]
fn sidebar_breakpoint_is_840_logical_pixels() {
    let app = app_sized(839, 700);
    with_library(&app, 1);
    assert_eq!(count(&app, "SidePanel"), 0);
    app.window().set_size(LogicalSize::new(840.0, 700.0));
    assert_eq!(count(&app, "SidePanel"), 1);
}

#[test]
fn content_fills_the_window_when_the_sidebar_is_collapsed() {
    for (width, height, collapse) in [(1280, 800, true), (900, 1400, false), (700, 600, false)] {
        let app = app_sized(width, height);
        app.set_library_list_view(true);
        if collapse {
            app.set_sidebar_collapsed_landscape(true);
        }
        with_library(&app, 2);
        let row = labelled(&app, "Score 1").expect("row is exposed");
        // Page padding, card inset and the list's scrollbar take under 80 px; a capped
        // column (as the chip row once caused) would be a fraction of the window.
        assert!(
            row.size().width > width as f32 - 80.0,
            "{width}x{height}: row spans the content card, got {}",
            row.size().width
        );
    }
}
