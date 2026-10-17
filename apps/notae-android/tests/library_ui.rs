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
    app.on_open_pdf_requested(move |path, key| {
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
    app.on_open_pdf_requested(move |path, key| {
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
fn recent_tap_opens_the_pdf_without_opening_the_preview() {
    for list in [false, true] {
        let app = app();
        app.set_library_list_view(list);
        app.set_selected_section("recent".into());
        with_library(&app, 3);
        let seen = record_dispatches(&app);

        click(&app, "Score 2", 0);
        {
            let seen = seen.borrow();
            assert_eq!(
                seen.opened,
                [("score-2.pdf".to_owned(), format!("{:064}", 2))],
                "list={list}"
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
        assert_eq!(seen.opened.len(), 2);
        assert!(seen.previewed.is_empty());
    }
}

#[test]
fn recent_items_expose_score_details_as_a_48px_action_for_the_preview() {
    for list in [false, true] {
        let app = app();
        app.set_library_list_view(list);
        app.set_selected_section("recent".into());
        with_library(&app, 3);
        let seen = record_dispatches(&app);

        let details: Vec<_> =
            ElementHandle::find_by_accessible_label(&app, "Score details").collect();
        assert_eq!(
            details.len(),
            3,
            "every Recent item has the action, list={list}"
        );
        for action in &details {
            assert!(action.size().width >= 48.0 && action.size().height >= 48.0);
        }
        click(&app, "Score details", 1);
        let seen = seen.borrow();
        assert_eq!(seen.previewed, [format!("{:064}", 2)]);
        assert!(
            seen.opened.is_empty(),
            "details must not launch the PDF, list={list}"
        );
    }
}

#[test]
fn other_sections_tap_to_preview_and_have_no_separate_details_action() {
    for section in [
        "all",
        "needs-details",
        "text-pending",
        "composer:Composer 1",
    ] {
        for list in [false, true] {
            let app = app();
            app.set_library_list_view(list);
            app.set_selected_section(section.into());
            with_library(&app, 2);
            let seen = record_dispatches(&app);
            assert!(
                labelled(&app, "Score details").is_none(),
                "{section} list={list}"
            );
            click(&app, "Score 1", 0);
            let seen = seen.borrow();
            assert_eq!(
                seen.previewed,
                [format!("{:064}", 1)],
                "{section} list={list}"
            );
            assert!(seen.opened.is_empty());
        }
    }
}

fn center(element: &ElementHandle) -> slint::LogicalPosition {
    let position = element.absolute_position();
    let size = element.size();
    slint::LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    )
}

fn item_center(app: &AppWindow, title: &str) -> slint::LogicalPosition {
    // The item's own accessible element is its card or row, not the actions on it.
    center(&labelled(app, title).unwrap_or_else(|| panic!("`{title}` is exposed")))
}

fn pointer_down(app: &AppWindow, at: slint::LogicalPosition) {
    use slint::platform::{PointerEventButton, WindowEvent};
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: at });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
}

fn pointer_move(app: &AppWindow, to: slint::LogicalPosition) {
    app.window()
        .dispatch_event(slint::platform::WindowEvent::PointerMoved { position: to });
}

fn pointer_up(app: &AppWindow, at: slint::LogicalPosition) {
    use slint::platform::{PointerEventButton, WindowEvent};
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
}

fn wait(milliseconds: u64) {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(milliseconds));
    slint::platform::update_timers_and_animations();
}

fn menu_action(app: &AppWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(app, label)
        .find(|element| element.type_name().as_deref() == Some("PillButton"))
}

fn menu_is_open(app: &AppWindow) -> bool {
    menu_action(app, "Open score").is_some()
}

#[test]
fn every_item_has_a_48px_edit_action_that_edits_exactly_that_score() {
    for (list, section) in [
        (false, "all"),
        (true, "all"),
        (false, "recent"),
        (true, "recent"),
    ] {
        let app = app();
        app.set_library_list_view(list);
        app.set_selected_section(section.into());
        with_library(&app, 3);
        let seen = record_dispatches(&app);

        let edits: Vec<_> = ElementHandle::find_by_accessible_label(&app, "Edit details").collect();
        assert_eq!(
            edits.len(),
            3,
            "one per item, list={list} section={section}"
        );
        for edit in &edits {
            assert!(edit.size().width >= 48.0 && edit.size().height >= 48.0);
        }
        click(&app, "Edit details", 1);
        let seen = seen.borrow();
        assert_eq!(seen.edited, [format!("{:064}", 2)]);
        assert!(
            seen.opened.is_empty() && seen.previewed.is_empty(),
            "edit is not also a tap"
        );
    }
}

#[test]
fn edit_actions_do_not_overlap_the_other_item_action() {
    for list in [false, true] {
        let app = app();
        app.set_library_list_view(list);
        app.set_selected_section("recent".into());
        with_library(&app, 1);
        let edit = labelled(&app, "Edit details").expect("edit action");
        let details = labelled(&app, "Score details").expect("details action");
        let (e, d) = (edit.absolute_position(), details.absolute_position());
        let separated = d.x + details.size().width <= e.x
            || e.x + edit.size().width <= d.x
            || d.y + details.size().height <= e.y
            || e.y + edit.size().height <= d.y;
        assert!(
            separated,
            "list={list}: edit {e:?} and details {d:?} overlap"
        );
    }
}

#[test]
fn long_press_opens_actions_without_also_tapping() {
    for (list, section) in [
        (false, "all"),
        (true, "all"),
        (false, "recent"),
        (true, "recent"),
    ] {
        let app = app();
        app.set_library_list_view(list);
        app.set_selected_section(section.into());
        with_library(&app, 3);
        let seen = record_dispatches(&app);
        let at = item_center(&app, "Score 2");

        pointer_down(&app, at);
        wait(499);
        assert!(!menu_is_open(&app), "not before 500 ms");
        wait(2);
        assert!(menu_is_open(&app), "list={list} section={section}");
        pointer_up(&app, at);

        let seen = seen.borrow();
        assert!(
            seen.opened.is_empty() && seen.previewed.is_empty() && seen.edited.is_empty(),
            "the consumed press does not tap: list={list} section={section}"
        );
        assert!(
            labelled(&app, "Actions for Score 2").is_some(),
            "the menu names the pressed score"
        );
    }
}

#[test]
fn menu_actions_route_the_pressed_score_and_close_the_menu() {
    for (label, expect) in [
        ("Open score", "open"),
        ("Score details", "preview"),
        ("Edit details", "edit"),
    ] {
        let app = app();
        with_library(&app, 3);
        let seen = record_dispatches(&app);
        let at = item_center(&app, "Score 3");
        pointer_down(&app, at);
        wait(600);
        pointer_up(&app, at);
        menu_action(&app, label)
            .expect("menu action")
            .invoke_accessible_default_action();

        let seen = seen.borrow();
        let key = format!("{:064}", 3);
        match expect {
            "open" => assert_eq!(seen.opened, [("score-3.pdf".to_owned(), key)]),
            "preview" => assert_eq!(seen.previewed, [key]),
            _ => assert_eq!(seen.edited, [key]),
        }
        assert!(!menu_is_open(&app), "{label} closes the menu");
    }
}

#[test]
fn early_release_is_an_ordinary_tap() {
    let app = app();
    with_library(&app, 3);
    let seen = record_dispatches(&app);
    let at = item_center(&app, "Score 1");
    pointer_down(&app, at);
    wait(300);
    pointer_up(&app, at);
    wait(1_000);
    assert!(
        !menu_is_open(&app),
        "a released press never opens the menu later"
    );
    assert_eq!(seen.borrow().previewed, [format!("{:064}", 1)]);
}

#[test]
fn moving_beyond_touch_slop_cancels_the_long_press() {
    let app = app();
    with_library(&app, 3);
    let seen = record_dispatches(&app);
    let at = item_center(&app, "Score 1");
    pointer_down(&app, at);
    wait(200);
    pointer_move(&app, slint::LogicalPosition::new(at.x + 4.0, at.y + 3.0));
    wait(100);
    pointer_move(&app, slint::LogicalPosition::new(at.x + 40.0, at.y));
    wait(1_000);
    assert!(
        !menu_is_open(&app),
        "drifting a long way is a scroll or drag, not a long press"
    );
    pointer_up(&app, slint::LogicalPosition::new(at.x + 40.0, at.y));
    let seen = seen.borrow();
    assert!(seen.opened.is_empty() && seen.edited.is_empty());
}

#[test]
fn small_finger_drift_still_long_presses() {
    let app = app();
    with_library(&app, 3);
    let at = item_center(&app, "Score 1");
    pointer_down(&app, at);
    pointer_move(&app, slint::LogicalPosition::new(at.x + 4.0, at.y - 3.0));
    wait(600);
    assert!(menu_is_open(&app));
}

#[test]
fn a_sheet_opening_or_the_item_disappearing_cancels_the_pending_long_press() {
    let app = app();
    with_library(&app, 3);
    let at = item_center(&app, "Score 1");
    pointer_down(&app, at);
    wait(200);
    app.set_show_metadata_editor(true);
    wait(1); // let the app publish the modal state before time jumps ahead
    wait(1_000);
    assert!(!menu_is_open(&app), "a modal opening cancels");
    app.set_show_metadata_editor(false);
    pointer_up(&app, at);

    let app = self::app();
    with_library(&app, 3);
    let at = item_center(&app, "Score 1");
    pointer_down(&app, at);
    wait(200);
    app.set_library_rows(ModelRc::new(VecModel::from(Vec::<LibraryRow>::new())));
    // Rendering refreshes repeaters every frame; do it explicitly in the headless window.
    assert_eq!(count(&app, "ScoreCard"), 0);
    wait(1_000);
    assert!(!menu_is_open(&app), "a removed item cannot open a menu");
}

#[test]
fn dismissing_the_menu_never_activates_the_item_underneath() {
    use slint::platform::{Key, WindowEvent};
    let app = app();
    with_library(&app, 3);
    let seen = record_dispatches(&app);
    let at = item_center(&app, "Score 1");
    let open_menu = |app: &AppWindow| {
        pointer_down(app, at);
        wait(600);
        pointer_up(app, at);
        assert!(menu_is_open(app));
    };

    // Android Back / Escape.
    open_menu(&app);
    let text: SharedString = Key::Escape.into();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
    assert!(!menu_is_open(&app));
    assert!(!app.get_library_menu_open());

    // Tapping the scrim over the item.
    open_menu(&app);
    pointer_down(&app, at);
    pointer_up(&app, at);
    assert!(!menu_is_open(&app));

    // While open, the items beneath ignore accessibility activation too.
    open_menu(&app);
    labelled(&app, "Score 2")
        .expect("row")
        .invoke_accessible_default_action();
    click(&app, "Edit details", 0);
    let seen = seen.borrow();
    assert!(
        seen.opened.is_empty() && seen.previewed.is_empty() && seen.edited.is_empty(),
        "nothing dispatched: {:?} {:?} {:?}",
        seen.opened,
        seen.previewed,
        seen.edited
    );
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
