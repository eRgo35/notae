mod support;

use i_slint_backend_testing::ElementHandle;
use std::time::Duration;
use support::app;

#[test]
fn newly_opened_search_fades_in_with_the_shared_medium_duration() {
    let app = app();
    app.set_show_search(true);
    let field =
        ElementHandle::find_by_accessible_label(&app, "Search titles, composers, tags or lyrics")
            .next()
            .expect("Search field is mounted");

    assert!(
        field.computed_opacity() < 1.0,
        "new screen starts its fade-in"
    );
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    assert_eq!(field.computed_opacity(), 1.0, "screen reaches full opacity");
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
fn disabling_motion_reaches_screen_and_sheet_final_states_without_time_advance() {
    let app = app();
    app.set_animations_enabled(false);
    app.set_show_search(true);
    let field =
        ElementHandle::find_by_accessible_label(&app, "Search titles, composers, tags or lyrics")
            .next()
            .expect("Search field is mounted");
    assert_eq!(field.computed_opacity(), 1.0);

    app.set_show_metadata_editor(true);
    i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Score details dialog")
            .next()
            .is_some()
    );
    app.set_show_metadata_editor(false);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Score details dialog")
            .next()
            .is_none()
    );
}

#[test]
fn rapid_screen_changes_keep_the_latest_logical_destination_visible() {
    let app = app();
    app.set_show_search(true);
    app.set_show_search(false);
    app.set_show_settings(true);
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Indexed scores: 0").count(),
        1,
        "Settings is the logical destination"
    );

    app.set_show_settings(false);
    app.set_show_search(true);
    let field =
        ElementHandle::find_by_accessible_label(&app, "Search titles, composers, tags or lyrics")
            .next()
            .expect("the latest Search destination is mounted");
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    assert_eq!(
        field.size().height,
        52.0,
        "SearchField has settled geometry after mounting"
    );
    assert_eq!(
        field.computed_opacity(),
        1.0,
        "the latest screen finishes its fade-in"
    );
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Scan library").count(),
        0,
        "the fading Settings screen is not logically active"
    );
}

#[test]
fn dismissing_import_preview_keeps_its_contents_mounted_until_exit_finishes() {
    let app = app();
    app.set_show_import_preview(true);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    app.set_show_import_preview(false);

    assert!(
        ElementHandle::find_by_accessible_label(&app, "Review desktop import dialog")
            .next()
            .is_some(),
        "the import preview remains mounted during its exit"
    );
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Review desktop import dialog")
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
