mod support;

use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;
use support::{app, app_sized};

#[test]
fn welcome_and_recovery_actions_remain_reachable_in_compact_and_short_landscape() {
    for (width, height) in [(360, 640), (800, 360)] {
        let app = app_sized(width, height);
        app.set_show_setup(true);
        assert!(
            ElementHandle::find_by_accessible_label(&app, "Welcome to Notae")
                .next()
                .is_some()
        );
        assert!(
            ElementHandle::find_by_accessible_label(&app, "Choose score folder")
                .next()
                .is_some()
        );

        app.set_show_recovery(true);
        app.set_recovery_folder_name("Orchestra".into());
        assert!(
            ElementHandle::find_by_accessible_label(&app, "Reconnect your library")
                .next()
                .is_some()
        );
        assert!(
            ElementHandle::find_by_accessible_label(&app, "Reconnect folder")
                .next()
                .is_some()
        );
    }
}
#[test]
fn settings_actions_stay_in_the_screen_and_expose_folder_change() {
    let app = app_sized(360, 600);
    app.set_show_settings(true);

    assert!(
        ElementHandle::find_by_accessible_label(&app, "Change")
            .next()
            .is_some()
    );
    app.set_library_display_name("A very long shared family sheet music folder name".into());
    assert!(
        ElementHandle::find_by_accessible_label(
            &app,
            "A very long shared family sheet music folder name"
        )
        .next()
        .is_some()
    );
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Scan library").count(),
        1,
        "Settings has one maintenance Scan action and no header Scan action"
    );
}

#[test]
fn settings_shows_absent_and_real_scan_results_and_runs_maintenance_scan() {
    use std::cell::Cell;
    use std::rc::Rc;

    let app = app_sized(360, 600);
    app.set_show_settings(true);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "No scan this session")
            .next()
            .is_some()
    );

    app.set_settings_scan_summary(
        "Latest scan: 0 added, 2 updated, 0 moved, 0 deleted, 1 conflicts, 1 failures".into(),
    );
    assert!(
        ElementHandle::find_by_accessible_label(
            &app,
            "Latest scan: 0 added, 2 updated, 0 moved, 0 deleted, 1 conflicts, 1 failures"
        )
        .next()
        .is_some()
    );

    let scanned = Rc::new(Cell::new(false));
    app.on_rescan_clicked({
        let scanned = scanned.clone();
        move || scanned.set(true)
    });
    app.set_rescan_enabled(true);
    ElementHandle::find_by_accessible_label(&app, "Scan library")
        .next()
        .expect("maintenance scan is reachable")
        .invoke_accessible_default_action();
    assert!(
        scanned.get(),
        "maintenance Scan invokes the existing rescan operation"
    );
}

#[test]
fn maintenance_scan_is_available_when_settings_is_open() {
    let app = app();
    app.set_show_settings(true);

    assert!(
        ElementHandle::find_by_accessible_label(&app, "Scan library")
            .next()
            .is_some()
    );
}

#[test]
fn settings_is_reachable_and_exposes_an_accessible_header_action() {
    let app = app();
    let app_weak = app.as_weak();
    app.on_settings_clicked(move || {
        if let Some(app) = app_weak.upgrade() {
            app.set_show_settings(true);
        }
    });
    let app_weak = app.as_weak();
    app.on_settings_closed(move || {
        if let Some(app) = app_weak.upgrade() {
            app.set_show_settings(false);
        }
    });

    let settings_action = ElementHandle::find_by_accessible_label(&app, "Settings")
        .next()
        .expect("configured Library exposes its Settings action");
    settings_action.invoke_accessible_default_action();

    let back_action = ElementHandle::find_by_accessible_label(&app, "Back to library")
        .next()
        .expect("Settings exposes an accessible way back to Library");
    assert_eq!(
        back_action.accessible_role(),
        Some(i_slint_backend_testing::AccessibleRole::Button)
    );
}

#[test]
fn independently_initialized_threads_own_their_generated_ui() {
    let handles = [800, 900].map(|width| {
        std::thread::spawn(move || {
            let app = app_sized(width, 800);
            assert!(
                ElementHandle::find_by_accessible_label(&app, "Settings")
                    .next()
                    .is_some()
            );
        })
    });

    for handle in handles {
        handle
            .join()
            .expect("each test thread initializes its own Slint backend");
    }
}
