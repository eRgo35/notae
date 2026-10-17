mod support;

use i_slint_backend_testing::ElementHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
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
    assert!(
        change_size.height <= 56.0,
        "Change keeps its compact height instead of stretching: {change_size:?}"
    );
    let folder_card = ElementHandle::find_by_element_type_name(&app, "Card")
        .next()
        .expect("folder card exists");
    assert!(
        folder_card.size().height <= 128.0,
        "Library folder card is compact: {:?}",
        folder_card.size()
    );
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
        0,
        "Settings has no Scan action; the Library header owns Scan (#29)"
    );
}

#[test]
fn search_header_has_no_scan_and_keyboard_hint_is_contextual() {
    use std::{cell::Cell, rc::Rc};

    let app = app_sized(360, 600);
    let voice_requests = Rc::new(Cell::new(0));
    let voice_requests_for_callback = Rc::clone(&voice_requests);
    app.on_voice_search_requested(move || {
        voice_requests_for_callback.set(voice_requests_for_callback.get() + 1);
    });
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Scan library")
            .next()
            .is_some()
    );
    app.set_show_settings(true);
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Scan library").count(),
        0,
        "Settings has no Scan action"
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
    microphone.invoke_accessible_default_action();
    assert_eq!(voice_requests.get(), 1);
    assert_eq!(app.get_search_text().as_str(), "");

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
#[test]
fn search_keycap_tracks_typed_text_and_keyboard_presence_changes() {
    let app = app_sized(1280, 800);
    app.set_show_search(true);
    let keycap = || ElementHandle::find_by_accessible_label(&app, "Ctrl K").next();
    assert!(keycap().is_none(), "Search begins without a keyboard hint");
    app.set_hardware_keyboard_present(true);
    let changed_queries = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let changed_queries_callback = changed_queries.clone();
    app.on_search_changed(move |query| {
        changed_queries_callback
            .borrow_mut()
            .push(query.to_string());
    });
    slint::platform::update_timers_and_animations();

    assert!(
        keycap().is_some(),
        "an empty field with a keyboard shows the hint"
    );
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Press Ctrl K to open search from anywhere.")
            .next()
            .is_none(),
        "the empty Search message omits the keyboard shortcut sentence"
    );
    let field =
        ElementHandle::find_by_accessible_label(&app, "Search titles, composers, tags or lyrics")
            .next()
            .expect("Search field is visible");
    let field_position = field.absolute_position();
    let field_size = field.size();
    let hint = keycap().expect("keyboard hint is visible in the empty field");
    let hint_position = hint.absolute_position();
    let hint_size = hint.size();
    assert!(
        hint_position.x >= field_position.x
            && hint_position.y >= field_position.y
            && hint_position.x + hint_size.width <= field_position.x + field_size.width
            && hint_position.y + hint_size.height <= field_position.y + field_size.height,
        "keycap stays within SearchField bounds"
    );
    let center = LogicalPosition::new(
        field_position.x + field_size.width / 2.0,
        field_position.y + field_size.height / 2.0,
    );
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: center });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: center,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: center,
        button: PointerEventButton::Left,
    });
    for character in "Chopin".chars() {
        let text = slint::SharedString::from(character.to_string());
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    }
    slint::platform::update_timers_and_animations();

    assert_eq!(
        field.accessible_value(),
        Some(slint::SharedString::from("Chopin")),
        "key events reach the focused TextInput"
    );
    assert!(keycap().is_none(), "typing hides the shortcut hint");
    assert_eq!(app.get_search_text(), "Chopin");
    let clear = ElementHandle::find_by_accessible_label(&app, "Clear search")
        .next()
        .expect("typed text exposes the clear action");
    let clear_position = clear.absolute_position();
    let clear_size = clear.size();
    assert!(
        clear_position.x >= field_position.x
            && clear_position.x + clear_size.width <= field_position.x + field_size.width,
        "clear action remains within the field bounds"
    );
    assert_eq!(
        changed_queries.borrow().last().map(String::as_str),
        Some("Chopin"),
        "typed text reaches the Search query callback"
    );

    clear.invoke_accessible_default_action();
    assert_eq!(app.get_search_text(), "");
    assert_eq!(
        changed_queries.borrow().last().map(String::as_str),
        Some(""),
        "clearing the field sends an empty Search query"
    );
    assert!(
        keycap().is_some(),
        "clearing an empty field restores the hardware shortcut hint"
    );
    app.set_hardware_keyboard_present(false);
    assert!(
        keycap().is_none(),
        "detaching the keyboard hides the hint while Search remains open"
    );
    app.set_hardware_keyboard_present(true);
    assert!(
        keycap().is_some(),
        "attaching the keyboard restores the hint without reopening Search"
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

fn label_exists(app: &notae_android::AppWindow, label: &str) -> bool {
    ElementHandle::find_by_accessible_label(app, label)
        .next()
        .is_some()
}

#[test]
fn settings_without_scan_summary_states_absence_and_hides_result_rows() {
    for (width, height) in [(360, 600), (1280, 800)] {
        let app = app_sized(width, height);
        app.set_show_settings(true);
        app.set_settings_indexed_count("42".into());
        app.set_settings_no_text_count("3".into());
        for label in ["Indexed scores: 42", "Last scan: No scan this session"] {
            let row = ElementHandle::find_by_accessible_label(&app, label)
                .next()
                .expect("maintenance fact row exists");
            assert!(
                row.size().height <= 32.0,
                "{label} keeps its natural height at {width}x{height}: {:?}",
                row.size()
            );
        }
        assert!(label_exists(&app, "Last scan: No scan this session"));
        // A plain count of indexed PDFs without extractable text; no action is offered.
        assert!(label_exists(&app, "PDFs without extractable text: 3"));
        assert!(!label_exists(&app, "ID conflicts: 0"));
        assert!(!label_exists(&app, "Failures: 0"));
        for removed in ["Local storage and privacy", "Pending text"] {
            assert!(!label_exists(&app, removed), "{removed} is gone");
        }
    }
}

#[test]
fn settings_shows_conflict_and_failure_rows_without_a_scan_action() {
    for (width, height) in [(360, 600), (1280, 800)] {
        let app = app_sized(width, height);
        app.set_show_settings(true);
        app.set_settings_scan_summary("0 added, 2 updated, 0 moved, 0 deleted".into());
        app.set_settings_scan_conflicts("1".into());
        app.set_settings_scan_failures("3".into());
        assert!(label_exists(
            &app,
            "Last scan: 0 added, 2 updated, 0 moved, 0 deleted"
        ));
        assert!(label_exists(&app, "ID conflicts: 1"));
        assert!(label_exists(&app, "Failures: 3"));
        assert!(
            !label_exists(&app, "Scan library"),
            "Settings has no Scan action at {width}x{height}"
        );

        let change = ElementHandle::find_by_accessible_label(&app, "Change")
            .next()
            .expect("Change is reachable");
        assert_element_in_viewport(&change, width, height);
    }
}

#[test]
fn maintenance_card_keeps_the_density_of_the_other_settings_cards() {
    for (width, height) in [(360, 800), (1280, 800)] {
        let app = app_sized(width, height);
        app.set_show_settings(true);
        app.set_settings_indexed_count("70".into());
        app.set_settings_scan_summary("3 added, 2 updated, 1 moved, 4 deleted".into());
        app.set_settings_scan_conflicts("1".into());
        app.set_settings_scan_failures("3".into());
        let maintenance = card_containing(&app, "Indexed scores: 70");
        let folder = card_containing(&app, "Change");
        let about = card_containing(&app, "License: MIT License");
        let rows: f32 = [
            "Indexed scores: 70",
            "Last scan: 3 added, 2 updated, 1 moved, 4 deleted",
            "ID conflicts: 1",
            "Failures: 3",
        ]
        .iter()
        .map(|label| {
            ElementHandle::find_by_accessible_label(&app, label)
                .next()
                .unwrap_or_else(|| panic!("{label} exists"))
                .size()
                .height
        })
        .sum();
        // 16 px padding twice, one title line and 8 px between the five children: any more is a
        // stretched card (#29).
        let natural = 32.0 + 30.0 + rows + 8.0 * 4.0;
        assert!(
            maintenance.size().height <= natural,
            "{width}x{height}: maintenance card hugs its rows ({natural}): {:?}",
            maintenance.size()
        );
        assert!(folder.size().height <= 128.0);
        assert_eq!(maintenance.size().width, folder.size().width);
        assert_eq!(maintenance.size().width, about.size().width);
    }
}

/// The smallest `Card` whose bounds contain the element with this accessible label.
fn card_containing(app: &notae_android::AppWindow, label: &str) -> ElementHandle {
    let target = ElementHandle::find_by_accessible_label(app, label)
        .next()
        .unwrap_or_else(|| panic!("{label} exists"));
    let at = target.absolute_position();
    ElementHandle::find_by_element_type_name(app, "Card")
        .filter(|card| {
            let origin = card.absolute_position();
            let size = card.size();
            at.x >= origin.x
                && at.y >= origin.y
                && at.x < origin.x + size.width
                && at.y < origin.y + size.height
        })
        .min_by(|a, b| a.size().height.total_cmp(&b.size().height))
        .unwrap_or_else(|| panic!("a Card contains {label}"))
}

#[test]
fn settings_about_lists_build_facts_in_one_place() {
    let app = app_sized(360, 600);
    app.set_show_settings(true);
    app.set_app_version("9.8.7".into());
    app.set_app_build_identity("abc1234".into());
    app.set_sidecar_schema_version("2".into());
    for label in [
        "Version: 9.8.7",
        "Build: abc1234",
        "License: MIT License",
        "Repository: https://github.com/eRgo35/notae",
        "Sidecar schema: 2",
    ] {
        assert!(label_exists(&app, label), "About exposes {label}");
    }
    assert_eq!(
        ElementHandle::find_by_accessible_label(&app, "Indexed scores: 0").count(),
        1,
        "indexed count appears once, in maintenance"
    );
}

#[test]
fn welcome_groups_intro_card_and_action_without_oversized_gaps() {
    let intro = "Your sheet music, in good order. Notae reads your PDFs where they are and opens them in the PDF app you already use.";
    for (width, height) in [(360, 800), (1280, 2000)] {
        let app = app_sized(width, height);
        app.set_show_setup(true);
        let paragraph = ElementHandle::find_by_accessible_label(&app, intro)
            .next()
            .expect("first-run paragraph exists");
        let card = ElementHandle::find_by_element_type_name(&app, "Card")
            .next()
            .expect("feature card exists");
        let gap = card.absolute_position().y
            - (paragraph.absolute_position().y + paragraph.size().height);
        assert!(
            (gap - 12.0).abs() < 0.5,
            "paragraph to card gap is 12 px at {width}x{height}, got {gap}"
        );
        assert!(
            card.size().height <= 200.0,
            "feature card keeps its natural height at {width}x{height}: {:?}",
            card.size()
        );
        let choose = ElementHandle::find_by_accessible_label(&app, "Choose score folder")
            .next()
            .expect("setup action exists");
        let action_gap =
            choose.absolute_position().y - (card.absolute_position().y + card.size().height);
        assert!(
            action_gap <= 48.0,
            "action sits right below the card at {width}x{height}, gap {action_gap}"
        );
        let title = ElementHandle::find_by_accessible_label(&app, "Welcome to Notae")
            .next()
            .expect("title exists");
        let scroll = ElementHandle::find_by_element_type_name(&app, "ScrollView")
            .next()
            .expect("welcome scroll view exists");
        let card_bottom = card.absolute_position().y + card.size().height;
        let scroll_bottom = scroll.absolute_position().y + scroll.size().height;
        assert!(
            card_bottom <= scroll_bottom + 0.5,
            "tall window shows the whole card without scrolling at {width}x{height}: card bottom {card_bottom}, scroll bottom {scroll_bottom}"
        );
        let offset = title.absolute_position().y - scroll.absolute_position().y;
        assert!(
            offset <= 64.0 + 96.0 + 20.0 + 0.5,
            "tall-window top padding is capped at 64 px at {width}x{height}; title offset={offset}"
        );
    }
}

#[test]
fn settings_has_no_maintenance_scan_while_library_keeps_exactly_one() {
    let app = app();
    assert_eq!(
        ElementHandle::find_by_element_type_name(&app, "ScanButton").count(),
        1,
        "the Library header owns the only Scan action"
    );
    app.set_show_settings(true);
    assert_eq!(
        ElementHandle::find_by_element_type_name(&app, "ScanButton").count(),
        0
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
#[test]
fn search_scope_chip_bounds_stay_fixed_during_pointer_press() {
    let app = app_sized(1280, 800);
    app.set_show_search(true);
    let app_weak = app.as_weak();
    app.on_search_scope_changed(move |scope| {
        if let Some(app) = app_weak.upgrade() {
            app.set_search_scope(scope);
        }
    });
    slint::platform::update_timers_and_animations();

    let chips: Vec<_> = ["All", "Titles", "Composers", "Lyrics", "Tags"]
        .into_iter()
        .map(|label| {
            ElementHandle::find_by_accessible_label(&app, label)
                .next()
                .unwrap_or_else(|| panic!("{label} scope chip exists"))
        })
        .collect();
    let row = ElementHandle::find_by_element_type_name(&app, "Flickable")
        .next()
        .expect("Search scope row is a Flickable");
    let chip_bounds = || {
        chips
            .iter()
            .map(|chip| {
                let position = chip.absolute_position();
                let size = chip.size();
                (position.x, position.y, size.width, size.height)
            })
            .collect::<Vec<_>>()
    };
    let row_bounds = || {
        let position = row.absolute_position();
        let size = row.size();
        (position.x, position.y, size.width, size.height)
    };
    let before = chip_bounds();
    let row_before = row_bounds();
    let viewport_offset_before = chips[0].absolute_position().x - row_before.0;
    let titles = &chips[1];
    let position = titles.absolute_position();
    let size = titles.size();
    let press = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: press });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: press,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
    assert_eq!(
        chip_bounds(),
        before,
        "all chip bounds are stable while pressed"
    );
    assert_eq!(row_bounds(), row_before);
    assert_eq!(
        chips[0].absolute_position().x - row.absolute_position().x,
        viewport_offset_before
    );

    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: press,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
    assert_eq!(app.get_search_scope(), "titles");
    assert_eq!(chip_bounds(), before, "selection changes no chip geometry");
    assert_eq!(row_bounds(), row_before);
    assert_eq!(
        chips[0].absolute_position().x - row.absolute_position().x,
        viewport_offset_before
    );
}

#[test]
fn search_scope_chip_bounds_stay_fixed_during_small_pointer_drift() {
    let app = app_sized(1280, 800);
    app.set_show_search(true);
    let app_weak = app.as_weak();
    app.on_search_scope_changed(move |scope| {
        if let Some(app) = app_weak.upgrade() {
            app.set_search_scope(scope);
        }
    });
    slint::platform::update_timers_and_animations();

    let chips: Vec<_> = ["All", "Titles", "Composers", "Lyrics", "Tags"]
        .into_iter()
        .map(|label| {
            ElementHandle::find_by_accessible_label(&app, label)
                .next()
                .unwrap_or_else(|| panic!("{label} scope chip exists"))
        })
        .collect();
    let row = ElementHandle::find_by_element_type_name(&app, "Flickable")
        .next()
        .expect("Search scope row is a Flickable");
    let chip_bounds = || {
        chips
            .iter()
            .map(|chip| {
                let position = chip.absolute_position();
                let size = chip.size();
                (position.x, position.y, size.width, size.height)
            })
            .collect::<Vec<_>>()
    };
    let row_bounds = || {
        let position = row.absolute_position();
        let size = row.size();
        (position.x, position.y, size.width, size.height)
    };
    let before = chip_bounds();
    let row_before = row_bounds();
    let viewport_offset_before = chips[0].absolute_position().x - row_before.0;
    let titles = &chips[1];
    let position = titles.absolute_position();
    let size = titles.size();
    let press = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    let drift = LogicalPosition::new(press.x + 3.0, press.y);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: press });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: press,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: drift });
    slint::platform::update_timers_and_animations();
    assert_eq!(
        chip_bounds(),
        before,
        "a small pointer drift changes no chip bounds"
    );
    assert_eq!(row_bounds(), row_before);
    assert_eq!(
        chips[0].absolute_position().x - row.absolute_position().x,
        viewport_offset_before
    );

    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: drift,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
    assert_eq!(
        app.get_search_scope(),
        "titles",
        "small drift still selects the chip"
    );
    assert_eq!(chip_bounds(), before, "selection changes no chip geometry");
    assert_eq!(row_bounds(), row_before);
    assert_eq!(
        chips[0].absolute_position().x - row.absolute_position().x,
        viewport_offset_before
    );
}
