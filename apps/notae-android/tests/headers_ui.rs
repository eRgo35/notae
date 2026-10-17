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
        let choose = ElementHandle::find_by_accessible_label(&app, "Choose score folder")
            .next()
            .expect("setup action exists");
        assert_element_in_viewport(&choose, width, height);

        app.set_show_recovery(true);
        app.set_recovery_folder_name("Orchestra".into());
        assert!(
            ElementHandle::find_by_accessible_label(&app, "Reconnect your library")
                .next()
                .is_some()
        );
        let reconnect = ElementHandle::find_by_accessible_label(&app, "Reconnect folder")
            .next()
            .expect("recovery action exists");
        assert_element_in_viewport(&reconnect, width, height);
    }
}

#[test]
fn short_landscape_welcome_scrolls_its_content_without_clipping_the_action() {
    let app = app_sized(800, 360);
    app.set_show_setup(true);
    let scroll = ElementHandle::find_by_element_type_name(&app, "ScrollView")
        .next()
        .expect("welcome content scroll view exists");
    let title = ElementHandle::find_by_accessible_label(&app, "Welcome to Notae")
        .next()
        .expect("welcome title exists");
    let before = title.absolute_position();
    scroll.scroll(0.0, -1_000.0);
    let after = title.absolute_position();
    assert!(
        after.y < before.y,
        "scrolling the short landscape welcome content upward moves its title: before={before:?}, after={after:?}, scroll={:?} {:?}",
        scroll.absolute_position(),
        scroll.size()
    );
    let choose = ElementHandle::find_by_accessible_label(&app, "Choose score folder")
        .next()
        .expect("setup action exists");
    assert_element_in_viewport(&choose, 800, 360);
}

#[test]
fn settings_actions_stay_in_the_screen_and_expose_folder_change() {
    let app = app_sized(360, 600);
    app.set_show_settings(true);

    app.set_library_display_name("A very long shared family sheet music folder name".into());
    let folder = ElementHandle::find_by_accessible_label(
        &app,
        "A very long shared family sheet music folder name",
    )
    .next()
    .expect("long folder name is present");
    let change = ElementHandle::find_by_accessible_label(&app, "Change")
        .next()
        .expect("Change action is present");
    let folder_pos = folder.absolute_position();
    let folder_size = folder.size();
    let change_pos = change.absolute_position();
    let change_size = change.size();
    assert!(
        change_pos.y < folder_pos.y + folder_size.height
            && folder_pos.y < change_pos.y + change_size.height,
        "folder name and Change remain on the same wrapped row: folder={folder_pos:?} {folder_size:?}, Change={change_pos:?} {change_size:?}"
    );
    assert_element_in_viewport(&change, 360, 600);
    let folder_card = ElementHandle::find_by_element_type_name(&app, "Card")
        .next()
        .expect("folder card exists");
    let card_pos = folder_card.absolute_position();
    let card_size = folder_card.size();
    assert!(
        (folder_pos.x - (card_pos.x + 16.0)).abs() < 0.1
            && (change_pos.x + change_size.width - (card_pos.x + card_size.width - 16.0)).abs()
                < 0.1,
        "folder card content keeps its 16 px horizontal padding: card={card_pos:?} {card_size:?}, folder={folder_pos:?}, Change={change_pos:?} {change_size:?}"
    );
    assert!(
        folder_pos.x >= 0.0 && folder_pos.x + folder_size.width <= 360.0,
        "long folder name stays within the horizontal viewport: {folder_pos:?} {folder_size:?}"
    );
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Scan library").count(),
        1,
        "Settings has one maintenance Scan action and no header Scan action"
    );
}

#[test]
fn search_header_has_no_scan_and_keyboard_hint_is_contextual() {
    let app = app_sized(360, 600);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Scan library")
            .next()
            .is_some()
    );
    app.set_show_settings(true);
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Scan library").count(),
        1
    );
    app.set_show_settings(false);
    app.set_show_search(true);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Search by voice")
            .next()
            .is_none()
    );
    app.set_voice_search_available(true);
    let microphone = ElementHandle::find_by_accessible_label(&app, "Search by voice")
        .next()
        .expect("available recognizer exposes voice search");
    assert!(microphone.size().width >= 48.0 && microphone.size().height >= 48.0);

    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Scan library").count(),
        0
    );
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Ctrl K")
            .next()
            .is_none()
    );

    app.set_hardware_keyboard_present(true);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Ctrl K")
            .next()
            .is_some()
    );
    app.set_search_text("Chopin".into());
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Ctrl K")
            .next()
            .is_none()
    );
}

fn assert_element_in_viewport(element: &ElementHandle, width: u32, height: u32) {
    let position = element.absolute_position();
    let size = element.size();
    assert!(
        position.x >= 0.0
            && position.y >= 0.0
            && position.x + size.width <= width as f32
            && position.y + size.height <= height as f32,
        "element bounds must stay inside {width}x{height}: position={position:?}, size={size:?}"
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
#[test]
fn changing_search_scope_keeps_following_chip_bounds_stable() {
    let app = app_sized(1280, 800);
    app.set_show_search(true);

    let titles = ElementHandle::find_by_accessible_label(&app, "Titles")
        .next()
        .expect("Titles scope chip exists");
    let before = titles.absolute_position();
    app.set_search_scope("titles".into());
    slint::platform::update_timers_and_animations();
    let after = titles.absolute_position();

    assert_eq!(after.x, before.x);
}

#[test]
fn search_scope_chips_scroll_only_when_the_row_overflows() {
    for (width, expect_scroll) in [(1280, false), (320, true)] {
        let app = app_sized(width, 600);
        app.set_show_search(true);
        let titles = ElementHandle::find_by_accessible_label(&app, "Titles")
            .next()
            .expect("Titles scope chip exists");
        let before = titles.absolute_position();
        let flickable = ElementHandle::find_by_element_type_name(&app, "Flickable")
            .next()
            .expect("Search scope row is a Flickable");

        flickable.scroll(-500.0, 0.0);
        slint::platform::update_timers_and_animations();
        let after = titles.absolute_position();

        if expect_scroll {
            assert!(
                after.x < before.x,
                "narrow overflowing scope row scrolls horizontally: before={before:?}, after={after:?}"
            );
        } else {
            assert_eq!(
                after.x, before.x,
                "scope row that fits does not scroll: before={before:?}, after={after:?}"
            );
        }
    }
}

#[test]
fn startup_hides_setup_until_restoration_is_resolved() {
    let app = app();
    app.set_show_setup(true);
    app.set_startup_ready(false);

    assert!(
        ElementHandle::find_by_accessible_label(&app, "Loading library")
            .next()
            .is_some()
    );
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Welcome to Notae")
            .next()
            .is_none()
    );

    app.set_startup_ready(true);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Loading library")
            .next()
            .is_none()
    );
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Welcome to Notae")
            .next()
            .is_some(),
        "the resolved no-saved-root path presents Welcome"
    );
}
