mod support;

use i_slint_backend_testing::ElementHandle;
use notae_android::{AppWindow, LibraryRow, SectionItem};
use slint::{ModelRc, VecModel};
use std::time::Duration;
use support::app;

const SEARCH_FIELD: &str = "Search titles, composers, tags or lyrics";
/// Shared Theme tokens: views enter over the medium token and leave over the short one.
const ENTER: u64 = 220;
const EXIT: u64 = 120;

fn tick(milliseconds: u64) {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(milliseconds));
    slint::platform::update_timers_and_animations();
}

fn label(app: &AppWindow, text: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(app, text).next()
}

fn element(app: &AppWindow, type_name: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_element_type_name(app, type_name).next()
}

fn x_of(element: &ElementHandle) -> f32 {
    element.absolute_position().x
}

/// A Library that has finished its startup fade-in.
fn settled() -> AppWindow {
    let app = app();
    tick(500);
    app
}

fn with_sections(app: &AppWindow) {
    let section = |key: &str, label: &str| SectionItem {
        key: key.into(),
        label: label.into(),
        count: 0,
        kind: "".into(),
    };
    app.set_sections(ModelRc::new(VecModel::from(vec![
        section("all", "All scores"),
        section("recent", "Recent"),
    ])));
}

#[test]
fn library_to_search_keeps_the_outgoing_screen_while_the_incoming_one_slides_and_fades_in() {
    let app = settled();
    let scan = element(&app, "ScanButton").expect("Library header Scan");
    assert_eq!(scan.computed_opacity(), 1.0);
    let rest = x_of(&scan);

    app.set_show_search(true);
    let field = label(&app, SEARCH_FIELD).expect("the logical destination mounts at once");
    assert_eq!(field.computed_opacity(), 0.0, "Search starts invisible");
    assert_eq!(
        scan.computed_opacity(),
        1.0,
        "Library has not started leaving yet"
    );

    tick(EXIT / 2);
    let leaving = element(&app, "ScanButton").expect("outgoing Library stays mounted mid-way");
    let opacity = leaving.computed_opacity();
    assert!(
        opacity > 0.0 && opacity < 1.0,
        "Library is half way out, got {opacity}"
    );
    assert!(
        x_of(&leaving) < rest - 0.5,
        "Library slides away from the right: {} vs rest {rest}",
        x_of(&leaving)
    );
    let arriving = field.computed_opacity();
    assert!(
        arriving > 0.0 && arriving < 1.0,
        "Search is partly visible, got {arriving}"
    );
    let arriving_x = x_of(&field);

    tick(EXIT / 2 + 10);
    assert!(
        element(&app, "ScanButton").is_none(),
        "Library unmounts once its short exit has finished"
    );
    assert!(field.computed_opacity() < 1.0, "Search is still arriving");

    tick(ENTER);
    assert_eq!(field.computed_opacity(), 1.0, "Search ends fully opaque");
    assert!(
        arriving_x > x_of(&field) + 0.5,
        "Search arrives from the right: {arriving_x} then {}",
        x_of(&field)
    );
}

#[test]
fn search_back_to_library_slides_the_other_way() {
    let app = settled();
    app.set_show_search(true);
    tick(500);
    let rest = x_of(&label(&app, SEARCH_FIELD).expect("Search is open"));

    app.set_show_search(false);
    tick(EXIT / 2);
    let leaving = label(&app, SEARCH_FIELD).expect("Search stays mounted while it leaves");
    assert!(leaving.computed_opacity() > 0.0 && leaving.computed_opacity() < 1.0);
    assert!(
        x_of(&leaving) > rest + 0.5,
        "Search leaves towards the right"
    );
    let scan = element(&app, "ScanButton").expect("Library is already back");
    assert!(scan.computed_opacity() < 1.0);
    let arriving_x = x_of(&scan);

    tick(500);
    assert!(label(&app, SEARCH_FIELD).is_none());
    assert_eq!(scan.computed_opacity(), 1.0);
    assert!(
        arriving_x < x_of(&scan) - 0.5,
        "Library arrives from the left"
    );
}

#[test]
fn settings_transition_uses_the_same_axis_and_tokens() {
    let app = settled();
    let scan_rest = x_of(&element(&app, "ScanButton").expect("Library Scan"));
    app.set_show_settings(true);
    tick(EXIT / 2);
    let library = element(&app, "ScanButton").expect("Library leaves, not vanishes");
    assert!(library.computed_opacity() < 1.0 && x_of(&library) < scan_rest - 0.5);
    let change = label(&app, "Change").expect("Settings is mounted");
    assert!(change.computed_opacity() > 0.0 && change.computed_opacity() < 1.0);
    let mid = x_of(&change);
    tick(500);
    assert_eq!(change.computed_opacity(), 1.0);
    assert!(mid > x_of(&change) + 0.5);
    assert!(element(&app, "ScanButton").is_none());
}

#[test]
fn a_leaving_screen_cannot_be_tapped_but_the_arriving_one_can() {
    use std::cell::Cell;
    use std::rc::Rc;

    let app = settled();
    app.set_rescan_enabled(true);
    let scans = Rc::new(Cell::new(0));
    app.on_rescan_clicked({
        let scans = Rc::clone(&scans);
        move || scans.set(scans.get() + 1)
    });
    let closed = Rc::new(Cell::new(0));
    app.on_search_closed({
        let closed = Rc::clone(&closed);
        move || closed.set(closed.get() + 1)
    });
    let scan = element(&app, "ScanButton").expect("Library Scan");

    app.set_show_search(true);
    tick(EXIT / 2);
    scan.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(scans.get(), 0, "the leaving Library ignores the tap");

    label(&app, "Back to library")
        .expect("Search header Back")
        .mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(
        closed.get(),
        1,
        "Search is interactive while it is arriving"
    );
}

#[test]
fn reversing_in_mid_flight_turns_around_from_the_current_state() {
    let app = settled();
    app.set_show_search(true);
    tick(ENTER / 2);
    let field = label(&app, SEARCH_FIELD).expect("Search is arriving");
    let half = field.computed_opacity();
    assert!(half > 0.0 && half < 1.0);

    app.set_show_search(false);
    tick(1);
    assert!(
        field.computed_opacity() <= half + 0.05,
        "Search never jumps back to opaque"
    );
    tick(500);
    assert!(label(&app, SEARCH_FIELD).is_none(), "Search ends unmounted");
    let scan = element(&app, "ScanButton").expect("Library is back");
    assert_eq!(scan.computed_opacity(), 1.0);
}

#[test]
fn disabled_motion_jumps_to_the_final_state_without_any_time() {
    let app = settled();
    app.set_animations_enabled(false);
    tick(0);
    app.set_show_search(true);
    let field = label(&app, SEARCH_FIELD).expect("Search is mounted");
    assert_eq!(field.computed_opacity(), 1.0);
    assert!(
        element(&app, "ScanButton").is_none(),
        "the outgoing Library is gone immediately"
    );

    app.set_show_search(false);
    assert!(label(&app, SEARCH_FIELD).is_none());
    assert_eq!(
        element(&app, "ScanButton")
            .expect("Library is back")
            .computed_opacity(),
        1.0
    );
}

fn preview_row() -> LibraryRow {
    LibraryRow {
        title: "Opus 1".into(),
        note_key: format!("{:064}", 1).into(),
        relative_path: "opus-1.pdf".into(),
        ..LibraryRow::default()
    }
}

#[test]
fn opening_and_closing_the_preview_cross_fades_it_with_the_library() {
    let app = settled();
    app.set_selected(preview_row());
    let scan = element(&app, "ScanButton").expect("Library Scan");
    let rest = x_of(&scan);

    app.set_details_open(true);
    tick(EXIT / 2);
    let chrome = element(&app, "ScanButton").expect("Library chrome stays while it leaves");
    assert!(chrome.computed_opacity() > 0.0 && chrome.computed_opacity() < 1.0);
    assert!(x_of(&chrome) < rest - 0.5);
    let preview = element(&app, "ScoreDetails").expect("preview is mounted");
    assert!(preview.computed_opacity() > 0.0 && preview.computed_opacity() < 1.0);
    let mid = x_of(&preview);

    tick(500);
    assert_eq!(preview.computed_opacity(), 1.0);
    assert!(mid > x_of(&preview) + 0.5, "preview arrives from the right");
    assert!(
        element(&app, "ScanButton").is_none(),
        "chrome has unmounted"
    );

    app.set_details_open(false);
    tick(EXIT / 2);
    assert!(
        element(&app, "ScoreDetails")
            .expect("preview stays visible while it leaves")
            .computed_opacity()
            < 1.0
    );
    tick(500);
    assert_eq!(
        element(&app, "ScanButton")
            .expect("chrome is back")
            .computed_opacity(),
        1.0
    );
    assert_eq!(x_of(&element(&app, "ScanButton").expect("Scan")), rest);
}

#[test]
fn sidebar_slides_and_fades_without_reflowing_the_content() {
    let app = settled();
    with_sections(&app);
    tick(1);
    let card = |app: &AppWindow| element(app, "Card").expect("content card");
    assert!(
        element(&app, "SidePanel").is_some(),
        "landscape starts with the sidebar"
    );
    let expanded_x = x_of(&card(&app));
    let width = card(&app).size().width;

    app.set_sidebar_collapsed_landscape(true);
    let final_width = card(&app).size().width;
    assert_eq!(
        final_width,
        width + 304.0,
        "the column is laid out once at its final width"
    );
    tick(EXIT / 2);
    let panel = element(&app, "SidePanel").expect("sidebar stays while it leaves");
    assert!(panel.computed_opacity() > 0.0 && panel.computed_opacity() < 1.0);
    let mid_x = x_of(&card(&app));
    assert!(
        mid_x < expanded_x - 1.0 && mid_x > 16.0 + 1.0,
        "content slides left: {mid_x} between 16 and {expanded_x}"
    );
    assert_eq!(
        card(&app).size().width,
        final_width,
        "no re-flow while the sidebar leaves"
    );
    tick(EXIT);
    assert!(
        element(&app, "SidePanel").is_none(),
        "sidebar has unmounted"
    );
    assert_eq!(
        x_of(&card(&app)),
        16.0,
        "content has reached its final place"
    );

    app.set_sidebar_collapsed_landscape(false);
    let arriving = element(&app, "SidePanel").expect("sidebar mounts at once");
    assert!(arriving.computed_opacity() < 1.0);
    assert_eq!(
        card(&app).size().width,
        width,
        "again one layout, no re-flow"
    );
    tick(ENTER / 2);
    let mid_x = x_of(&card(&app));
    assert!(mid_x > 16.0 + 1.0 && mid_x < expanded_x - 1.0);
    assert_eq!(card(&app).size().width, width);
    tick(500);
    assert_eq!(arriving.computed_opacity(), 1.0);
    assert_eq!(x_of(&card(&app)), expanded_x);
}

#[test]
fn disabled_motion_toggles_the_sidebar_instantly() {
    let app = settled();
    with_sections(&app);
    app.set_animations_enabled(false);
    tick(0);
    app.set_sidebar_collapsed_landscape(true);
    assert!(element(&app, "SidePanel").is_none());
    app.set_sidebar_collapsed_landscape(false);
    assert_eq!(
        element(&app, "SidePanel")
            .expect("sidebar is back")
            .computed_opacity(),
        1.0
    );
}

#[test]
fn dismissing_sheet_keeps_its_modal_surface_until_exit_finishes() {
    let app = app();
    app.set_show_metadata_editor(true);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    let sheet = ElementHandle::find_by_accessible_label(&app, "Score details dialog")
        .next()
        .expect("metadata sheet is open");
    assert_eq!(sheet.computed_opacity(), 1.0);

    app.set_show_metadata_editor(false);
    let exiting_sheet = ElementHandle::find_by_accessible_label(&app, "Score details dialog")
        .next()
        .expect("dismissed sheet stays mounted during its visual exit");
    assert!(exiting_sheet.computed_opacity() > 0.0);

    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Score details dialog")
            .next()
            .is_none()
    );
}

#[test]
fn zero_android_animator_scale_disables_motion_but_nonzero_scale_keeps_it() {
    assert!(!notae_android::motion::animations_enabled_for_scale(0.0));
    assert!(notae_android::motion::animations_enabled_for_scale(1.0));
    assert!(notae_android::motion::animations_enabled_for_scale(0.5));
}
