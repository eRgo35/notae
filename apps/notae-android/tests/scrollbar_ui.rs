//! The themed scrollbar, exercised through the real Library/preview/Search surfaces.

mod support;

use std::time::Duration;

use i_slint_backend_testing::ElementHandle;
use notae_android::{AppWindow, LibraryRow};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition, LogicalSize, ModelRc, SharedString, VecModel};
use support::{app, app_sized};

fn score(index: usize) -> LibraryRow {
    LibraryRow {
        id: SharedString::from(format!("{index}")),
        title: SharedString::from(format!("Score {index}")),
        composer_or_artist: SharedString::from("Composer"),
        note_key: SharedString::from(format!("{index:064}")),
        relative_path: SharedString::from(format!("score-{index}.pdf")),
        ..LibraryRow::default()
    }
}

fn library(app: &AppWindow, count: usize) {
    let rows: Vec<LibraryRow> = (1..=count).map(score).collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
}

fn wait(milliseconds: u64) {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(milliseconds));
    slint::platform::update_timers_and_animations();
}

fn thumb(app: &AppWindow) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, "Scroll position")
        .next()
        .expect("the scrollbar thumb is part of the surface")
}

fn thumb_shown(app: &AppWindow) -> bool {
    thumb(app).accessible_enabled() == Some(true)
}

fn only(app: &AppWindow, type_name: &str) -> ElementHandle {
    ElementHandle::find_by_element_type_name(app, type_name)
        .next()
        .unwrap_or_else(|| panic!("{type_name} is present"))
}

fn center(element: &ElementHandle) -> LogicalPosition {
    let position = element.absolute_position();
    let size = element.size();
    LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    )
}

fn wheel(app: &AppWindow, at: LogicalPosition, delta_y: f32) {
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: at });
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position: at,
        delta_x: 0.0,
        delta_y,
    });
    slint::platform::update_timers_and_animations();
}

fn press(app: &AppWindow, at: LogicalPosition) {
    use slint::platform::PointerEventButton;
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: at });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
}

fn move_to(app: &AppWindow, to: LogicalPosition) {
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: to });
}

fn release(app: &AppWindow, at: LogicalPosition) {
    use slint::platform::PointerEventButton;
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
}

/// Drags the thumb by `delta` logical pixels (the pointer may leave the track).
fn drag_thumb(app: &AppWindow, delta: f32) {
    if !thumb_shown(app) {
        // The thumb is only grabbable while it is visible, as for a person: nudge the content first.
        let surface = ElementHandle::find_by_element_type_name(app, "ScoreGrid")
            .next()
            .or_else(|| ElementHandle::find_by_element_type_name(app, "ListView").next())
            .expect("a scrolling surface");
        wheel(app, center(&surface), -1.0);
    }
    let start = center(&thumb(app));
    press(app, start);
    for step in 1..=10 {
        move_to(
            app,
            LogicalPosition::new(start.x, start.y + delta * step as f32 / 10.0),
        );
    }
    release(app, LogicalPosition::new(start.x, start.y + delta));
    slint::platform::update_timers_and_animations();
}

/// Scrolls to the very top or bottom with the wheel, then lets the bar fade.
fn scroll_to(app: &AppWindow, bottom: bool) {
    let grid = only(app, "ScoreGrid");
    for _ in 0..4 {
        wheel(
            app,
            center(&grid),
            if bottom { -100_000.0 } else { 100_000.0 },
        );
    }
}

fn card_y(app: &AppWindow, index: usize) -> f32 {
    ElementHandle::find_by_element_type_name(app, "ScoreCard")
        .nth(index)
        .expect("card exists")
        .absolute_position()
        .y
}

fn labelled(app: &AppWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, label)
        .next()
        .unwrap_or_else(|| panic!("`{label}` is exposed"))
}

fn track(app: &AppWindow) -> (f32, f32) {
    let grid = only(app, "ScoreGrid");
    (
        grid.absolute_position().y,
        grid.absolute_position().y + grid.size().height,
    )
}

#[test]
fn bar_is_hidden_when_the_content_fits_and_never_scrolls_anything() {
    let app = app();
    library(&app, 3);
    scroll_to(&app, true);
    assert!(!thumb_shown(&app), "nothing to scroll");
    let before = card_y(&app, 0);
    let at = center(&thumb(&app));
    press(&app, at);
    move_to(&app, LogicalPosition::new(at.x, at.y + 300.0));
    release(&app, LogicalPosition::new(at.x, at.y + 300.0));
    assert_eq!(card_y(&app, 0), before);
}

#[test]
fn bar_appears_while_scrolling_and_fades_after_1200_ms() {
    let app = app();
    library(&app, 80);
    assert!(!thumb_shown(&app), "idle at rest");
    wheel(&app, center(&only(&app, "ScoreGrid")), -300.0);
    assert!(thumb_shown(&app), "shown while scrolling");
    wait(1_150);
    assert!(thumb_shown(&app), "still shown before the idle timeout");
    wait(100);
    assert!(!thumb_shown(&app), "hidden once idle");

    // Scrolling again brings it back and restarts the countdown.
    wheel(&app, center(&only(&app, "ScoreGrid")), -300.0);
    wait(1_000);
    wheel(&app, center(&only(&app, "ScoreGrid")), -100.0);
    wait(1_000);
    assert!(thumb_shown(&app), "each movement restarts the idle timer");
    wait(400);
    assert!(!thumb_shown(&app));
}

#[test]
fn thumb_endpoints_match_the_viewport_and_the_target_is_at_least_48_px() {
    let app = app();
    library(&app, 80);
    let (track_top, track_bottom) = track(&app);

    scroll_to(&app, false);
    let thumb_at_top = thumb(&app);
    assert!(thumb_at_top.size().width >= 48.0 && thumb_at_top.size().height >= 48.0);
    assert!((thumb_at_top.absolute_position().y - track_top).abs() < 0.5);

    scroll_to(&app, true);
    let thumb_at_bottom = thumb(&app);
    let bottom = thumb_at_bottom.absolute_position().y + thumb_at_bottom.size().height;
    assert!(
        (bottom - track_bottom).abs() < 0.5,
        "thumb bottom {bottom} vs track {track_bottom}"
    );
    // The viewport really is at the end: the last card rests on the bottom edge.
    let last = ElementHandle::find_by_element_type_name(&app, "ScoreCard").count() - 1;
    let grid = only(&app, "ScoreGrid");
    let grid_bottom = grid.absolute_position().y + grid.size().height;
    let last_card = ElementHandle::find_by_element_type_name(&app, "ScoreCard")
        .nth(last)
        .expect("last card");
    assert!(
        (last_card.absolute_position().y + last_card.size().height - (grid_bottom - 20.0)).abs()
            < 1.0,
        "last card ends at the bottom with the trailing gap"
    );
}

#[test]
fn dragging_the_thumb_scrolls_to_the_exact_bounds() {
    let app = app();
    library(&app, 80);
    let (track_top, track_bottom) = track(&app);
    scroll_to(&app, false);
    let first_at_top = labelled(&app, "Score 1").absolute_position().y;

    drag_thumb(&app, 5_000.0);
    let t = thumb(&app);
    assert!(
        (t.absolute_position().y + t.size().height - track_bottom).abs() < 0.5,
        "thumb pinned at the end of the track"
    );
    let last = labelled(&app, "Score 80");
    assert!(
        (last.absolute_position().y + last.size().height - (track_bottom - 20.0)).abs() < 1.0,
        "bottom drag shows the very end of the content"
    );

    drag_thumb(&app, -5_000.0);
    assert!(
        (labelled(&app, "Score 1").absolute_position().y - first_at_top).abs() < 0.5,
        "top drag restores the first card exactly"
    );
    assert!((thumb(&app).absolute_position().y - track_top).abs() < 0.5);
}

#[test]
fn a_middle_thumb_position_is_reversible() {
    let app = app();
    library(&app, 80);
    let (track_top, track_bottom) = track(&app);
    scroll_to(&app, false);
    let range = track_bottom - track_top - thumb(&app).size().height;

    drag_thumb(&app, range / 2.0);
    let middle = thumb(&app).absolute_position().y - track_top;
    assert!(
        (middle - range / 2.0).abs() < 1.0,
        "thumb followed the pointer: {middle} vs {}",
        range / 2.0
    );
    let content_in_middle = card_y(&app, 0);

    drag_thumb(&app, -range / 2.0);
    assert!((thumb(&app).absolute_position().y - track_top).abs() < 1.0);
    drag_thumb(&app, range / 2.0);
    assert!(
        (card_y(&app, 0) - content_in_middle).abs() < 1.0,
        "the same thumb position maps to the same offset"
    );
}

#[test]
fn a_thin_thumb_on_a_huge_list_keeps_its_minimum_and_exact_endpoints() {
    let app = app();
    app.set_library_list_view(true);
    library(&app, 5_000);
    let list = only(&app, "ListView");
    let (top, bottom) = (
        list.absolute_position().y,
        list.absolute_position().y + list.size().height,
    );
    wheel(&app, center(&list), -1.0);
    let t = thumb(&app);
    assert_eq!(t.size().height, 48.0, "minimum thumb length");
    assert!((t.absolute_position().y - top).abs() < 0.5);

    drag_thumb(&app, 100_000.0);
    let t = thumb(&app);
    assert!(
        (t.absolute_position().y + t.size().height - bottom).abs() < 0.5,
        "still reaches the end"
    );
    drag_thumb(&app, -100_000.0);
    assert!((thumb(&app).absolute_position().y - top).abs() < 0.5);
}

#[test]
fn an_active_drag_never_fades_and_fades_after_release() {
    let app = app();
    library(&app, 80);
    wheel(&app, center(&only(&app, "ScoreGrid")), -300.0);
    let at = center(&thumb(&app));
    press(&app, at);
    wait(5_000);
    assert!(thumb_shown(&app), "held thumb stays visible");
    move_to(&app, LogicalPosition::new(at.x, at.y + 30.0));
    wait(5_000);
    assert!(thumb_shown(&app));
    release(&app, LogicalPosition::new(at.x, at.y + 30.0));
    // Leave the thumb: hovering it (a mouse) keeps it visible.
    move_to(&app, LogicalPosition::new(10.0, 10.0));
    wait(1_150);
    assert!(thumb_shown(&app), "the countdown starts at release");
    wait(100);
    assert!(!thumb_shown(&app));
}

#[test]
fn shrinking_content_or_resizing_clamps_the_scroll_offset() {
    let app = app();
    library(&app, 80);
    scroll_to(&app, true);

    // More vertical room: the content must not stay scrolled past its new end.
    app.window().set_size(LogicalSize::new(1280.0, 1000.0));
    wait(32);
    let grid = only(&app, "ScoreGrid");
    let grid_bottom = grid.absolute_position().y + grid.size().height;
    let last = labelled(&app, "Score 80");
    assert!(
        (last.absolute_position().y + last.size().height - (grid_bottom - 20.0)).abs() < 1.0,
        "no empty space below the last card after the resize"
    );

    // Fewer rows: back to the top.
    library(&app, 4);
    wait(32);
    let grid = only(&app, "ScoreGrid");
    assert!(
        (labelled(&app, "Score 1").absolute_position().y - grid.absolute_position().y).abs() < 1.0,
        "first card at the top again"
    );
}

#[test]
fn accessibility_page_actions_move_the_viewport_and_clamp() {
    let app = app();
    app.set_library_list_view(true);
    library(&app, 200);
    let list = only(&app, "ListView");
    let list_bottom = list.absolute_position().y + list.size().height;
    wheel(&app, center(&list), -10.0);

    // A row that is fully visible near the bottom of the viewport before the step.
    let (label, before) = ElementHandle::find_by_element_type_name(&app, "ScoreRow")
        .filter(|row| row.absolute_position().y + row.size().height <= list_bottom)
        .map(|row| {
            (
                row.accessible_label().expect("row label"),
                row.absolute_position().y,
            )
        })
        .last()
        .expect("a fully visible row");
    thumb(&app).invoke_accessible_increment_action();
    let after = labelled(&app, &label).absolute_position().y;
    // The std ScrollView insets its viewport slightly, so the step is 90% of that (just under the list height).
    let step = before - after;
    assert!(
        (0.85 * list.size().height..=0.9 * list.size().height).contains(&step),
        "a page step moves about 90% of the viewport: {before} -> {after}"
    );

    for _ in 0..200 {
        if thumb_shown(&app) {
            thumb(&app).invoke_accessible_decrement_action();
        }
    }
    assert!(
        (labelled(&app, "Score 1").absolute_position().y - list.absolute_position().y).abs() < 10.0,
        "decrement clamps at the top"
    );
}

#[test]
fn library_preview_and_settings_show_no_default_scrollbar() {
    let app = app_sized(1280, 360); // short: the preview body must scroll
    library(&app, 30);
    app.set_library_list_view(true);
    assert_eq!(
        count(&app, "ScrollBar"),
        0,
        "library list uses the themed bar only"
    );

    let row = score(1);
    app.set_selected(row);
    app.set_selected_position("1 of 30".into());
    app.set_details_open(true);
    assert_eq!(
        count(&app, "ScrollBar"),
        0,
        "no explicit default bar in the preview"
    );
    let body = only(&app, "ScrollView");
    wheel(&app, center(&body), -500.0);
    assert!(
        thumb_shown(&app),
        "the preview body scrolls and shows the themed thumb"
    );

    // Settings uses the themed bar too: no standard bar, and its thumb appears when it scrolls.
    app.set_details_open(false);
    app.set_show_settings(true);
    assert_eq!(count(&app, "ScrollBar"), 0, "Settings has no standard bar");
    let body = only(&app, "ScrollView");
    wheel(&app, center(&body), -500.0);
    assert!(
        thumb_shown(&app),
        "Settings shows the themed thumb when it scrolls"
    );

    // Control: the modal sheet has not adopted the themed bar, which proves the query works.
    app.set_show_settings(false);
    app.set_show_metadata_editor(true);
    wait(500);
    assert!(
        count(&app, "ScrollBar") > 0,
        "the modal sheet still uses the standard scrollbar"
    );
}

#[test]
fn welcome_scrolls_with_the_themed_bar_in_a_short_window() {
    let app = app_sized(1280, 300);
    app.set_show_setup(true);
    wait(0);
    assert_eq!(count(&app, "ScrollBar"), 0, "Welcome has no standard bar");
    let body = only(&app, "ScrollView");
    wheel(&app, center(&body), -300.0);
    assert!(
        thumb_shown(&app),
        "Welcome shows the themed thumb when it scrolls"
    );
}

fn count(app: &AppWindow, type_name: &str) -> usize {
    ElementHandle::find_by_element_type_name(app, type_name).count()
}

#[test]
fn search_results_use_the_same_bar() {
    let app = app();
    library(&app, 60);
    app.set_show_search(true);
    app.set_search_text("score".into());
    let list = only(&app, "ListView");
    wheel(&app, center(&list), -400.0);
    assert!(thumb_shown(&app));
    assert_eq!(count(&app, "ScrollBar"), 0);
}
