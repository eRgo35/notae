mod support;

use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;
use support::{app, app_sized};

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
    assert_eq!(back_action.accessible_role(), Some(i_slint_backend_testing::AccessibleRole::Button));
}

#[test]
fn independently_initialized_threads_own_their_generated_ui() {
    let handles = [800, 900].map(|width| {
        std::thread::spawn(move || {
            let app = app_sized(width, 800);
            assert!(ElementHandle::find_by_accessible_label(&app, "Settings").next().is_some());
        })
    });

    for handle in handles {
        handle.join().expect("each test thread initializes its own Slint backend");
    }
}
