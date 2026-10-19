//! The "More actions" overflow button and the Drive-style actions sheet that replaced the
//! long-press menu and the per-item Info icon.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use notae_android::{AppWindow, LibraryRow, SectionItem, Theme};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Global, LogicalPosition, ModelRc, SharedString, VecModel};
use support::app_sized;

fn score(index: usize) -> LibraryRow {
    LibraryRow {
        id: SharedString::from(format!("{index}")),
        title: SharedString::from(format!("Score {index}")),
        composer_or_artist: SharedString::from(format!("Composer {index}")),
        note_key: SharedString::from(format!("{index:064}")),
        relative_path: SharedString::from(format!("score-{index}.pdf")),
        ..LibraryRow::default()
    }
}

fn wait(milliseconds: u64) {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(milliseconds));
    slint::platform::update_timers_and_animations();
}

/// A settled Library of three scores in the given layout, section and colour scheme.
fn library(width: u32, height: u32, list: bool, section: &str, dark: bool) -> AppWindow {
    let app = app_sized(width, height);
    app.set_library_list_view(list);
    app.set_selected_section(section.into());
    app.set_library_rows(ModelRc::new(VecModel::from(
        (1..=3).map(score).collect::<Vec<_>>(),
    )));
    app.set_sections(ModelRc::new(VecModel::from(vec![SectionItem {
        key: "all".into(),
        label: "All scores".into(),
        count: 3,
        kind: "".into(),
    }])));
    Theme::get(&app).set_dark(dark);
    wait(500);
    app
}

#[derive(Default)]
struct Dispatches {
    opened: Vec<(String, String)>,
    previewed: Vec<String>,
    edited: Vec<String>,
}

impl Dispatches {
    fn is_empty(&self) -> bool {
        self.opened.is_empty() && self.previewed.is_empty() && self.edited.is_empty()
    }
}

fn record(app: &AppWindow) -> Rc<RefCell<Dispatches>> {
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

fn labelled(app: &AppWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(app, label).next()
}

fn overflow(app: &AppWindow, title: &str) -> ElementHandle {
    labelled(app, &format!("More actions for {title}"))
        .unwrap_or_else(|| panic!("`More actions for {title}` is exposed"))
}

/// The sheet's own action (the preview has an "Open score" and an "Edit details" of its own).
fn sheet_action(app: &AppWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(app, label)
        .find(|element| element.type_name().as_deref() == Some("SheetAction"))
}

fn sheet_is_open(app: &AppWindow) -> bool {
    labelled(app, "Score actions dialog").is_some()
}

fn click(element: &ElementHandle) {
    element.mock_single_click(PointerEventButton::Left);
}

fn tap_at(app: &AppWindow, at: LogicalPosition) {
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: at });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
}

fn key(app: &AppWindow, key: Key) {
    let text: SharedString = key.into();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

/// Every combination the overflow button must work in: grid and list, All and Recent, a phone
/// and a tablet window, light and dark.
fn each_surface(check: impl Fn(&AppWindow, &str)) {
    for (width, height) in [(360, 640), (411, 891), (1280, 800)] {
        for list in [false, true] {
            for section in ["all", "recent"] {
                for dark in [false, true] {
                    let app = library(width, height, list, section, dark);
                    check(
                        &app,
                        &format!("{width}x{height} list={list} {section} dark={dark}"),
                    );
                }
            }
        }
    }
}

#[test]
fn every_item_has_a_labelled_48px_overflow_button_inside_it_and_no_info_icon() {
    each_surface(|app, context| {
        assert!(
            labelled(app, "Score details").is_none(),
            "{context}: the per-item Info icon is gone"
        );
        assert!(labelled(app, "Edit details").is_none(), "{context}");
        let count = ElementHandle::find_by_element_type_name(app, "MoreActions").count();
        assert_eq!(count, 3, "{context}: one per item");
        for title in ["Score 1", "Score 2", "Score 3"] {
            let item = labelled(app, title).expect("item is exposed");
            let button = overflow(app, title);
            let (b, i) = (button.absolute_position(), item.absolute_position());
            assert!(
                button.size().width >= 48.0 && button.size().height >= 48.0,
                "{context}: {title} target {:?}",
                button.size()
            );
            assert!(
                b.x >= i.x
                    && b.y >= i.y
                    && b.x + button.size().width <= i.x + item.size().width + 0.01
                    && b.y + button.size().height <= i.y + item.size().height + 0.01,
                "{context}: {title} button {b:?} outside item {i:?}"
            );
            assert_eq!(
                button.accessible_role(),
                Some(i_slint_core::items::AccessibleRole::Button)
            );
        }
    });
}

#[test]
fn tapping_the_overflow_button_opens_the_sheet_for_that_score_only() {
    each_surface(|app, context| {
        let seen = record(app);
        assert!(!sheet_is_open(app), "{context}");
        click(&overflow(app, "Score 2"));
        wait(500);
        assert!(sheet_is_open(app), "{context}");
        assert!(app.get_library_menu_open(), "{context}");
        assert!(
            seen.borrow().is_empty(),
            "{context}: opening the sheet dispatches nothing"
        );
        for label in ["Open score", "Score details", "Edit details"] {
            let action = sheet_action(app, label)
                .unwrap_or_else(|| panic!("{context}: sheet offers {label}"));
            assert!(action.size().height >= 48.0, "{context}: {label}");
        }
        // Header: the title of the tapped score, inside the sheet.
        let sheet = labelled(app, "Score actions dialog").expect("sheet");
        let header = ElementHandle::find_by_accessible_label(app, "Score 2")
            .find(|element| {
                element.absolute_position().y >= sheet.absolute_position().y
                    && element.absolute_position().y
                        < sheet.absolute_position().y + sheet.size().height
            })
            .unwrap_or_else(|| panic!("{context}: the sheet is headed by the score title"));
        assert!(header.size().width > 0.0);
    });
}

#[test]
fn the_sheet_sits_at_the_bottom_and_phones_get_it_full_width() {
    for (width, height) in [(360u32, 640u32), (1280, 800)] {
        let app = library(width, height, false, "all", false);
        click(&overflow(&app, "Score 1"));
        wait(500);
        let sheet = labelled(&app, "Score actions dialog").expect("sheet");
        let (position, size) = (sheet.absolute_position(), sheet.size());
        assert!(
            (position.y + size.height - height as f32).abs() < 1.0,
            "{width}x{height}: bottom anchored, {position:?} {size:?}"
        );
        if width < 600 {
            assert!((size.width - width as f32).abs() < 1.0, "{size:?}");
        } else {
            assert!(size.width <= 640.0);
        }
    }
}

#[test]
fn sheet_actions_route_the_exact_stable_key_and_close_the_sheet() {
    each_surface(|app, context| {
        for (label, which) in [("Open score", 0), ("Score details", 1), ("Edit details", 2)] {
            let seen = record(app);
            click(&overflow(app, "Score 2"));
            wait(500);
            click(&sheet_action(app, label).unwrap_or_else(|| {
                panic!(
                    "{context}: {label} missing, sheet open={}",
                    sheet_is_open(app)
                )
            }));
            wait(500);
            let seen = seen.borrow();
            let key = format!("{:064}", 2);
            match which {
                0 => assert_eq!(seen.opened, [("score-2.pdf".to_owned(), key)], "{context}"),
                1 => assert_eq!(seen.previewed, [key], "{context}"),
                _ => assert_eq!(seen.edited, [key], "{context}"),
            }
            let total = seen.opened.len() + seen.previewed.len() + seen.edited.len();
            assert_eq!(total, 1, "{context}: {label} dispatches exactly once");
            assert!(!sheet_is_open(app), "{context}: {label} closes the sheet");
            assert!(!app.get_library_menu_open(), "{context}");
        }
    });
}

#[test]
fn accessibility_default_action_of_a_sheet_action_activates_it() {
    let app = library(411, 891, false, "all", false);
    let seen = record(&app);
    overflow(&app, "Score 1").invoke_accessible_default_action();
    wait(500);
    assert!(
        sheet_is_open(&app),
        "the overflow button is operable without touch"
    );
    sheet_action(&app, "Edit details")
        .expect("action")
        .invoke_accessible_default_action();
    assert_eq!(seen.borrow().edited, [format!("{:064}", 1)]);
}

#[test]
fn keyboard_reaches_the_sheet_and_enter_activates_the_first_action() {
    let app = library(1280, 800, false, "all", false);
    let seen = record(&app);
    click(&overflow(&app, "Score 2"));
    wait(500);
    key(&app, Key::Return);
    assert_eq!(
        seen.borrow().opened,
        [("score-2.pdf".to_owned(), format!("{:064}", 2))],
        "the sheet takes focus on its first action"
    );
}

#[test]
fn dismissing_never_activates_anything() {
    each_surface(|app, context| {
        let seen = record(app);
        let item = labelled(app, "Score 1").expect("item");
        // Near the top-left corner of the card: visible above the sheet on every size.
        let corner = LogicalPosition::new(
            item.absolute_position().x + 6.0,
            item.absolute_position().y + 6.0,
        );
        for dismiss in ["scrim", "escape", "back"] {
            click(&overflow(app, "Score 2"));
            wait(500);
            assert!(sheet_is_open(app), "{context} {dismiss}");
            match dismiss {
                "scrim" => tap_at(app, corner),
                "escape" => key(app, Key::Escape),
                _ => key(app, Key::Back),
            }
            wait(500);
            assert!(!sheet_is_open(app), "{context}: {dismiss} dismisses");
            assert!(!app.get_library_menu_open(), "{context}: {dismiss}");
            assert!(
                seen.borrow().is_empty(),
                "{context}: {dismiss} activated something"
            );
        }
    });
}

#[test]
fn items_beneath_an_open_sheet_ignore_input() {
    let app = library(1280, 800, false, "all", false);
    let seen = record(&app);
    click(&overflow(&app, "Score 2"));
    wait(500);
    labelled(&app, "Score 1")
        .expect("item")
        .invoke_accessible_default_action();
    overflow(&app, "Score 3").invoke_accessible_default_action();
    assert!(
        seen.borrow().is_empty(),
        "the cards stay inert while the sheet is up"
    );
    assert!(sheet_is_open(&app));
}

#[test]
fn holding_an_item_no_longer_opens_anything_and_release_is_an_ordinary_tap() {
    for list in [false, true] {
        let app = library(411, 891, list, "all", false);
        let seen = record(&app);
        let item = labelled(&app, "Score 1").expect("item");
        let at = LogicalPosition::new(
            item.absolute_position().x + 30.0,
            item.absolute_position().y + 30.0,
        );
        app.window()
            .dispatch_event(WindowEvent::PointerMoved { position: at });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: at,
            button: PointerEventButton::Left,
        });
        wait(900);
        assert!(!sheet_is_open(&app), "list={list}: no long-press menu");
        assert!(
            seen.borrow().is_empty(),
            "list={list}: nothing before release"
        );
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: at,
            button: PointerEventButton::Left,
        });
        assert_eq!(
            seen.borrow().opened,
            [("score-1.pdf".to_owned(), format!("{:064}", 1))],
            "list={list}"
        );
    }
}
