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

/// Lets every transition finish so leaving views are unmounted.
fn settle() {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
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
fn list_rows_show_every_row_in_order_and_open_by_stable_key() {
    let app = app();
    app.set_library_list_view(true);
    with_library(&app, 4);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&opened);
    app.on_open_pdf_requested(move |path, key, _item| {
        recorded
            .borrow_mut()
            .push((path.to_string(), key.to_string()))
    });

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
    assert_eq!(
        *opened.borrow(),
        [("score-3.pdf".to_owned(), format!("{:064}", 3))]
    );
}

#[test]
fn card_and_list_row_describe_composer_and_that_they_open_the_score() {
    for list in [false, true] {
        let app = app();
        app.set_library_list_view(list);
        with_library(&app, 2);
        let item = labelled(&app, "Score 2").expect("item is exposed");
        assert_eq!(
            item.accessible_description().as_deref(),
            Some("Composer 2. Opens the score"),
            "list={list}"
        );
    }
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
    settle();
    assert_eq!((count(&app, "SidePanel"), count(&app, "Chip")), (0, 2));
    assert!(
        labelled(&app, "Settings").is_some(),
        "Settings stays reachable from the header"
    );
    press(&app, "Show sidebar");
    settle();
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

fn open_preview(app: &AppWindow, index: usize, count: usize) {
    let row = score(index);
    app.set_selected(row);
    app.set_selected_previous_key(if index > 1 {
        score(index - 1).note_key
    } else {
        "".into()
    });
    app.set_selected_next_key(if index < count {
        score(index + 1).note_key
    } else {
        "".into()
    });
    app.set_selected_position(format!("{index} of {count}").into());
    app.set_details_open(true);
}

#[test]
fn preview_uses_the_full_width_without_sidebar_chips_or_companion_list() {
    for (width, height, list) in [
        (1280, 800, false),
        (1280, 800, true),
        (1920, 1080, false),
        (900, 1400, true),
    ] {
        let app = app_sized(width, height);
        app.set_library_list_view(list);
        app.set_sidebar_collapsed_portrait(false);
        with_library(&app, 3);
        open_preview(&app, 2, 3);

        for hidden in ["SidePanel", "Chip", "ScoreCard", "ScoreRow", "ScoreList"] {
            assert_eq!(
                count(&app, hidden),
                0,
                "{hidden} at {width}x{height} list={list}"
            );
        }
        assert!(
            labelled(&app, "Hide sidebar").is_none() && labelled(&app, "Show sidebar").is_none()
        );
        let details = ElementHandle::find_by_element_type_name(&app, "ScoreDetails")
            .next()
            .expect("preview is shown");
        assert!(
            (details.size().width - (width as f32 - 32.0)).abs() < 1.0,
            "{width}x{height}: preview spans the content width, got {}",
            details.size().width
        );
        assert_eq!(
            app.get_library_list_view(),
            list,
            "preview does not touch the layout choice"
        );
    }
}

#[test]
fn preview_back_returns_with_the_same_layout_and_section() {
    let app = app();
    app.set_library_list_view(true);
    app.set_selected_section("recent".into());
    with_library(&app, 3);
    open_preview(&app, 1, 3);
    let cleared = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&cleared);
    let weak = app.as_weak();
    app.on_selection_cleared(move || {
        *counter.borrow_mut() += 1;
        if let Some(app) = weak.upgrade() {
            app.set_details_open(false);
        }
    });
    let back = labelled(&app, "Back to library").expect("Back is available at wide width");
    assert!(back.size().width >= 48.0 && back.size().height >= 48.0);
    back.invoke_accessible_default_action();
    assert_eq!(*cleared.borrow(), 1);
    assert_eq!(count(&app, "ScoreRow"), 3, "list layout is restored");
    assert_eq!(count(&app, "SidePanel"), 1);
    assert_eq!(app.get_selected_section(), "recent");
}

/// The sheet's Score details action (#26) and the animated preview layer (#10, #31) work as one
/// flow: choosing it cross-fades into the preview, and Back cross-fades to the same Library again.
#[test]
fn score_details_opens_the_animated_preview_and_back_returns() {
    let app = app();
    app.set_library_list_view(true);
    with_library(&app, 3);
    settle();
    let weak = app.as_weak();
    app.on_score_selected(move |key| {
        if let Some(app) = weak.upgrade() {
            app.set_selected(LibraryRow {
                title: "Score 2".into(),
                note_key: key,
                relative_path: "score-2.pdf".into(),
                ..LibraryRow::default()
            });
            app.set_details_open(true);
        }
    });
    let weak = app.as_weak();
    app.on_selection_cleared(move || {
        if let Some(app) = weak.upgrade() {
            app.set_details_open(false);
        }
    });
    let tick = |milliseconds: u64| {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(milliseconds));
        slint::platform::update_timers_and_animations();
    };

    labelled(&app, "More actions for Score 2")
        .expect("overflow button")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    settle();
    ElementHandle::find_by_accessible_label(&app, "Score details")
        .find(|element| element.type_name().as_deref() == Some("SheetAction"))
        .expect("the sheet offers Score details")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    assert!(app.get_details_open());
    tick(60);
    let preview = ElementHandle::find_by_element_type_name(&app, "ScoreDetails")
        .next()
        .expect("preview mounts as soon as Score details is chosen");
    assert!(
        preview.computed_opacity() > 0.0 && preview.computed_opacity() < 1.0,
        "the preview fades in rather than snapping: {}",
        preview.computed_opacity()
    );
    tick(500);
    assert_eq!(preview.computed_opacity(), 1.0);
    assert_eq!(count(&app, "ScoreRow"), 0, "the Library chrome has left");

    labelled(&app, "Back to library")
        .expect("Back is available")
        .invoke_accessible_default_action();
    tick(60);
    assert!(
        ElementHandle::find_by_element_type_name(&app, "ScoreDetails")
            .next()
            .expect("preview stays while it leaves")
            .computed_opacity()
            < 1.0
    );
    tick(500);
    assert_eq!(count(&app, "ScoreRow"), 3, "list layout is restored");
    assert_eq!(count(&app, "ScoreDetails"), 0, "the preview has unmounted");
}

#[test]
fn compact_preview_also_offers_back() {
    let app = app_sized(360, 640);
    with_library(&app, 2);
    open_preview(&app, 1, 2);
    assert!(labelled(&app, "Back to library").is_some());
}

#[test]
fn previous_and_next_follow_the_neighbours_and_disable_at_boundaries() {
    let app = app();
    with_library(&app, 3);
    let chosen = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&chosen);
    app.on_score_selected(move |key| recorded.borrow_mut().push(key.to_string()));

    open_preview(&app, 1, 3);
    assert_eq!(
        labelled(&app, "Previous score").and_then(|e| e.accessible_enabled()),
        Some(false)
    );
    assert_eq!(
        labelled(&app, "Next score").and_then(|e| e.accessible_enabled()),
        Some(true)
    );
    press(&app, "Previous score");
    assert!(chosen.borrow().is_empty(), "disabled at the first score");
    press(&app, "Next score");
    assert_eq!(*chosen.borrow(), [format!("{:064}", 2)]);

    open_preview(&app, 3, 3);
    assert_eq!(
        labelled(&app, "Next score").and_then(|e| e.accessible_enabled()),
        Some(false)
    );
    press(&app, "Next score");
    assert_eq!(chosen.borrow().len(), 1, "disabled at the last score");
    press(&app, "Previous score");
    assert_eq!(chosen.borrow()[1], format!("{:064}", 2));

    // A single score (or one outside the section) has neither neighbour.
    open_preview(&app, 1, 1);
    for label in ["Previous score", "Next score"] {
        assert_eq!(
            labelled(&app, label).and_then(|e| e.accessible_enabled()),
            Some(false)
        );
    }
    assert!(labelled(&app, "Score 1 of 1").is_some());
}

#[test]
fn left_and_right_keys_browse_the_preview() {
    use slint::platform::{Key, WindowEvent};
    let app = app();
    with_library(&app, 3);
    let chosen = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&chosen);
    app.on_score_selected(move |key| recorded.borrow_mut().push(key.to_string()));
    open_preview(&app, 2, 3);
    let send = |key: Key| {
        let text: SharedString = key.into();
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
    send(Key::LeftArrow);
    send(Key::RightArrow);
    assert_eq!(
        *chosen.borrow(),
        [format!("{:064}", 1), format!("{:064}", 3)]
    );

    open_preview(&app, 1, 3);
    send(Key::LeftArrow);
    assert_eq!(chosen.borrow().len(), 2, "no wrap before the first score");
    open_preview(&app, 3, 3);
    send(Key::RightArrow);
    assert_eq!(chosen.borrow().len(), 2, "no wrap after the last score");
}

#[test]
fn arrow_keys_do_nothing_while_a_sheet_is_open_or_no_preview_is_showing() {
    use slint::platform::{Key, WindowEvent};
    let app = app();
    with_library(&app, 3);
    let chosen = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&chosen);
    app.on_score_selected(move |_| *counter.borrow_mut() += 1);
    let send = |key: Key| {
        let text: SharedString = key.into();
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
    send(Key::RightArrow);
    assert_eq!(*chosen.borrow(), 0, "library without preview");
    open_preview(&app, 1, 3);
    app.set_show_metadata_editor(true);
    send(Key::RightArrow);
    assert_eq!(*chosen.borrow(), 0, "editor sheet owns the keys");
}

#[test]
fn preview_actions_open_and_edit_the_previewed_score() {
    let app = app();
    with_library(&app, 2);
    open_preview(&app, 2, 2);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let edited = Rc::new(RefCell::new(Vec::new()));
    let (o, e) = (Rc::clone(&opened), Rc::clone(&edited));
    app.on_open_pdf_requested(move |path, key, _item| {
        o.borrow_mut().push((path.to_string(), key.to_string()))
    });
    app.on_edit_metadata_requested(move |key| e.borrow_mut().push(key.to_string()));
    press(&app, "Open score");
    press(&app, "Edit details");
    assert_eq!(
        *opened.borrow(),
        [("score-2.pdf".to_owned(), format!("{:064}", 2))]
    );
    assert_eq!(*edited.borrow(), [format!("{:064}", 2)]);
}

#[test]
fn wide_preview_places_a_large_page_beside_the_details() {
    let app = app();
    with_library(&app, 1);
    open_preview(&app, 1, 1);
    let title_row = labelled(&app, "Open score").expect("Open score is exposed");
    let details = ElementHandle::find_by_element_type_name(&app, "ScoreDetails")
        .next()
        .expect("preview is shown");
    let preview = ElementHandle::find_by_element_type_name(&app, "PagePreview")
        .next()
        .expect("page preview is shown");
    assert!(
        preview.size().width >= 300.0 && preview.size().height >= 390.0,
        "{:?}",
        preview.size()
    );
    assert!(
        title_row.absolute_position().x > preview.absolute_position().x + preview.size().width,
        "actions sit beside the page, not under it"
    );
    assert!(
        preview.absolute_position().y + preview.size().height
            <= details.absolute_position().y + details.size().height
    );
}

#[derive(Default)]
struct Dispatches {
    opened: Vec<(String, String)>,
    previewed: Vec<String>,
    edited: Vec<String>,
}

fn record_dispatches(app: &AppWindow) -> Rc<RefCell<Dispatches>> {
    let seen = Rc::new(RefCell::new(Dispatches::default()));
    let opened = Rc::clone(&seen);
    app.on_open_pdf_requested(move |path, key, _item| {
        opened
            .borrow_mut()
            .opened
            .push((path.to_string(), key.to_string()));
    });
    let previewed = Rc::clone(&seen);
    app.on_score_selected(move |key| previewed.borrow_mut().previewed.push(key.to_string()));
    let edited = Rc::clone(&seen);
    app.on_edit_metadata_requested(move |key| edited.borrow_mut().edited.push(key.to_string()));
    seen
}

fn click(app: &AppWindow, label: &str, index: usize) {
    let _ = app;
    ElementHandle::find_by_accessible_label(app, label)
        .nth(index)
        .unwrap_or_else(|| panic!("`{label}` #{index} is exposed"))
        .mock_single_click(slint::platform::PointerEventButton::Left);
}

#[test]
fn tap_opens_the_pdf_in_every_section_and_layout_without_opening_the_preview() {
    for section in ["all", "recent", "composer:Composer 1"] {
        for list in [false, true] {
            let app = app();
            app.set_library_list_view(list);
            app.set_selected_section(section.into());
            with_library(&app, 3);
            let seen = record_dispatches(&app);

            click(&app, "Score 2", 0);
            {
                let seen = seen.borrow();
                assert_eq!(
                    seen.opened,
                    [("score-2.pdf".to_owned(), format!("{:064}", 2))],
                    "{section} list={list}"
                );
                assert!(
                    seen.previewed.is_empty(),
                    "one tap must not also open the preview"
                );
            }
            // Keyboard/accessibility activation takes the same single path.
            ElementHandle::find_by_accessible_label(&app, "Score 3")
                .next()
                .expect("row")
                .invoke_accessible_default_action();
            let seen = seen.borrow();
            assert_eq!(seen.opened.len(), 2, "{section} list={list}");
            assert!(seen.previewed.is_empty());
        }
    }
}

#[test]
fn items_show_no_missing_text_badge_or_text_pending_copy() {
    for list in [false, true] {
        let app = app();
        app.set_library_list_view(list);
        with_library(&app, 3);
        for label in ["No text yet", "Text pending"] {
            assert!(labelled(&app, label).is_none(), "{label} list={list}");
        }
        open_preview(&app, 2, 3);
    }
}

#[test]
fn sidebar_offers_only_the_sections_the_app_provides() {
    let app = app();
    with_library(&app, 3);
    for label in ["Needs details", "Text pending"] {
        assert!(labelled(&app, label).is_none(), "{label}");
    }
    assert!(labelled(&app, "Recent").is_some());
}

#[test]
fn editing_from_the_preview_still_works() {
    let app = app();
    with_library(&app, 2);
    open_preview(&app, 2, 2);
    let seen = record_dispatches(&app);
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Edit details").count(),
        1,
        "only the preview's action is on screen"
    );
    click(&app, "Edit details", 0);
    assert_eq!(seen.borrow().edited, [format!("{:064}", 2)]);
}

#[test]
fn details_star_has_accessible_label() {
    let app = app();
    with_library(&app, 2);
    open_preview(&app, 1, 2);
    assert!(labelled(&app, "Remove from favorites").is_none());
    let toggled = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&toggled);
    app.on_favorite_toggled(move |key| sink.borrow_mut().push(key.to_string()));
    press(&app, "Add to favorites");
    assert_eq!(*toggled.borrow(), [format!("{:064}", 1)]);

    let mut starred = score(1);
    starred.favorite = true;
    app.set_selected(starred);
    assert!(labelled(&app, "Add to favorites").is_none());
    assert_eq!(
        labelled(&app, "Remove from favorites").and_then(|star| star.accessible_role()),
        Some(i_slint_core::items::AccessibleRole::Button)
    );
}

fn set_piece(key: char, title: &str) -> notae_app_core::ui::search::SearchRow {
    notae_app_core::ui::search::SearchRow {
        id: None,
        title: title.to_owned(),
        composer_or_artist: None,
        tags: Vec::new(),
        status: "Ready".to_owned(),
        relative_path: format!("{title}.pdf"),
        note_key: key.to_string().repeat(64),
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
        item_id: None,
        position: None,
        missing: false,
    }
}

/// Pushes the model's selection to the details pane the way `sync_window` does.
fn show_selection(app: &AppWindow, model: &notae_app_core::ui::UiModel) {
    let nav = model.preview_neighbours();
    app.set_selected_previous_key(
        nav.as_ref()
            .and_then(|nav| nav.previous.clone())
            .unwrap_or_default()
            .into(),
    );
    app.set_selected_next_key(
        nav.as_ref()
            .and_then(|nav| nav.next.clone())
            .unwrap_or_default()
            .into(),
    );
    app.set_selected_position(
        nav.map_or_else(String::new, |nav| {
            format!("{} of {}", nav.position, nav.total)
        })
        .into(),
    );
    let selected = model.selected_row();
    app.set_details_open(selected.is_some());
    if let Some(row) = selected {
        app.set_selected(notae_android::rows::library_row(row, None, false));
    }
}

/// Plan Review Focus #1: a set holding the same piece twice (A, B, A). The ⋮ sheet, the details
/// pane and previous/next on the second copy act on that copy, never the first.
#[test]
fn selecting_second_copy_keeps_its_position() {
    use notae_app_core::favorites::PieceRef;
    use notae_app_core::sets::{SetFields, SetStore};
    use notae_app_core::ui::UiModel;
    use notae_app_core::ui::browse::LibrarySection;

    let dir = tempfile::tempdir().expect("temporary directory");
    let mut store = SetStore::default();
    let set = store.create(
        SetFields {
            name: "Msza".to_owned(),
            date: None,
            notes: None,
        },
        10,
    );
    let items: Vec<String> = [('a', "Ave"), ('b', "Bogurodzica"), ('a', "Ave")]
        .into_iter()
        .map(|(key, title)| {
            let piece = PieceRef {
                key: key.to_string().repeat(64),
                path: format!("{title}.pdf"),
            };
            store.add_piece(&set, piece, 10).expect("set exists")
        })
        .collect();
    let path = dir.path().join("sets.json");
    store.save(&path).expect("sets file is written");

    let model = Rc::new(RefCell::new(UiModel::new()));
    model.borrow_mut().set_sets_path(path);
    model
        .borrow_mut()
        .set_search_rows(vec![set_piece('a', "Ave"), set_piece('b', "Bogurodzica")]);
    model
        .borrow_mut()
        .select_section(LibrarySection::Set(set))
        .expect("no preferences file to save");

    let app = app();
    app.set_library_list_view(true);
    app.set_selected_section(model.borrow().section().key().into());
    let rows: Vec<LibraryRow> = model
        .borrow()
        .visible_rows()
        .into_iter()
        .map(|row| notae_android::rows::library_row(row, None, false))
        .collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
    settle();
    {
        let (model, weak) = (Rc::clone(&model), app.as_weak());
        app.on_score_selected(move |id| {
            model.borrow_mut().select_note(Some(id.to_string()));
            if let Some(app) = weak.upgrade() {
                show_selection(&app, &model.borrow());
            }
        });
    }
    let opened = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&opened);
    app.on_open_pdf_requested(move |path, key, item| {
        recorded
            .borrow_mut()
            .push((path.to_string(), key.to_string(), item.to_string()));
    });

    // ⋮ on the second Ave, then Score details.
    click(&app, "More actions for Ave", 1);
    settle();
    ElementHandle::find_by_accessible_label(&app, "Score details")
        .find(|element| element.type_name().as_deref() == Some("SheetAction"))
        .expect("the sheet offers Score details")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    settle();
    assert_eq!(app.get_selected().item_id, items[2].as_str());
    assert!(
        labelled(&app, "Score 3 of 3").is_some(),
        "the second copy is third"
    );
    assert_eq!(
        labelled(&app, "Next score").and_then(|e| e.accessible_enabled()),
        Some(false)
    );

    press(&app, "Open score");
    assert_eq!(
        *opened.borrow(),
        [("Ave.pdf".to_owned(), "a".repeat(64), items[2].clone())]
    );

    press(&app, "Previous score");
    assert!(labelled(&app, "Score 2 of 3").is_some());
    press(&app, "Next score");
    assert!(
        labelled(&app, "Score 3 of 3").is_some(),
        "next from Bogurodzica returns to the second Ave, not the first"
    );
    assert_eq!(app.get_selected().item_id, items[2].as_str());
}
