mod support;

use std::cell::Cell;
use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use support::app_sized;

fn settings() -> notae_android::AppWindow {
    // Tall enough that the Language card at the end of Settings is on screen.
    let app = app_sized(1280, 1600);
    app.set_show_settings(true);
    app
}

fn row(app: &notae_android::AppWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, label)
        .next()
        .unwrap_or_else(|| panic!("Settings lists {label}"))
}

#[test]
fn settings_offers_system_english_and_polish_with_one_selected() {
    let app = settings();
    app.set_language_choice(2);
    let description = |label| {
        row(&app, label)
            .accessible_description()
            .unwrap_or_default()
    };
    assert_eq!(description("System default"), "");
    assert_eq!(description("English"), "");
    assert_eq!(description("Polski"), "Selected");
}

#[test]
fn choosing_a_language_reports_its_settings_index() {
    let app = settings();
    let chosen = Rc::new(Cell::new(-1));
    {
        let chosen = Rc::clone(&chosen);
        app.on_language_chosen(move |index| chosen.set(index));
    }
    for (label, index) in [("Polski", 2), ("English", 1), ("System default", 0)] {
        row(&app, label).invoke_accessible_default_action();
        assert_eq!(chosen.get(), index, "{label}");
    }
}
