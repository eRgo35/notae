//! Polish text reaching Search through Slint's key and Android IME paths.
//!
//! These host tests drive the generated UI with the same event shapes the
//! resolved `i-slint-backend-android-activity` 1.18.1 builds: plain key
//! presses for hardware layouts and `InternalKeyEvent` composition events
//! for soft keyboards. They prove that nothing in the Slint `TextInput` or
//! our `SearchField` callbacks drops or rewrites non-ASCII text. They do not
//! prove that Samsung Keyboard or Gboard deliver the same events on a tablet.

mod support;

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
use notae_android::AppWindow;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};
use support::{app, app_sized};

const SEARCH_LABEL: &str = "Search titles, composers, tags or lyrics";
const POLISH_LOWER: &str = "ąćęłńóśźż";
const POLISH_UPPER: &str = "ĄĆĘŁŃÓŚŹŻ";

fn focused_search() -> (AppWindow, Rc<RefCell<Vec<String>>>) {
    focus_search(app())
}

fn focused_compact_search() -> (AppWindow, Rc<RefCell<Vec<String>>>) {
    focus_search(app_sized(360, 640))
}

fn focus_search(app: AppWindow) -> (AppWindow, Rc<RefCell<Vec<String>>>) {
    app.set_show_search(true);
    let queries = Rc::new(RefCell::new(Vec::new()));
    let queries_for_callback = Rc::clone(&queries);
    app.on_search_changed(move |query| queries_for_callback.borrow_mut().push(query.to_string()));
    slint::platform::update_timers_and_animations();

    let field = ElementHandle::find_by_accessible_label(&app, SEARCH_LABEL)
        .next()
        .expect("Search field is mounted");
    let position = field.absolute_position();
    let size = field.size();
    let center = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
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
    (app, queries)
}

fn field_value(app: &AppWindow) -> Option<SharedString> {
    ElementHandle::find_by_accessible_label(app, SEARCH_LABEL)
        .next()
        .and_then(|field| field.accessible_value())
}

/// The event `i-slint-backend-android-activity` dispatches when the IME
/// reports `text` with no composing region (`javahelper::callback_update_text`).
fn android_commit(text: &str) -> WindowEvent {
    let mut key_event = KeyEvent::default();
    key_event.text = text.into();
    WindowEvent::internal(InternalKeyEvent {
        event_type: KeyEventType::CommitComposition,
        replacement_range: Some(i32::MIN..i32::MAX),
        cursor_position: Some(text.len() as i32),
        anchor_position: Some(text.len() as i32),
        key_event,
        ..Default::default()
    })
}

/// The event the Android backend dispatches while `preedit` is still composing
/// after `committed` (the Java editable is `committed + preedit`).
fn android_compose(committed: &str, preedit: &str) -> WindowEvent {
    let mut key_event = KeyEvent::default();
    key_event.text = committed.into();
    WindowEvent::internal(InternalKeyEvent {
        event_type: KeyEventType::UpdateComposition,
        preedit_text: preedit.into(),
        preedit_selection: Some(0..preedit.len() as i32),
        replacement_range: Some(i32::MIN..i32::MAX),
        cursor_position: Some(committed.len() as i32),
        anchor_position: Some(committed.len() as i32),
        key_event,
    })
}

#[test]
fn polish_key_events_reach_the_search_field_and_query_callbacks_intact() {
    let (app, queries) = focused_search();
    let typed = format!("{POLISH_LOWER}{POLISH_UPPER}");
    for character in typed.chars() {
        let text = SharedString::from(character.to_string());
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    }
    slint::platform::update_timers_and_animations();

    assert_eq!(field_value(&app).as_deref(), Some(typed.as_str()));
    assert_eq!(app.get_search_text().as_str(), typed);
    let queries = queries.borrow();
    assert_eq!(
        queries.len(),
        typed.chars().count(),
        "one search per character"
    );
    assert_eq!(queries.last().map(String::as_str), Some(typed.as_str()));
    for (index, query) in queries.iter().enumerate() {
        let expected: String = typed.chars().take(index + 1).collect();
        assert_eq!(query, &expected, "query after character {}", index + 1);
    }
}

#[test]
fn android_ime_commits_of_polish_text_reach_search_intact() {
    for committed in [POLISH_LOWER, POLISH_UPPER, "Łódź — Pieśń żółta"] {
        let (app, queries) = focused_search();
        app.window().dispatch_event(android_commit(committed));
        slint::platform::update_timers_and_animations();

        assert_eq!(
            field_value(&app).as_deref(),
            Some(committed),
            "field shows {committed:?}"
        );
        assert_eq!(app.get_search_text().as_str(), committed);
        assert_eq!(
            queries.borrow().last().map(String::as_str),
            Some(committed),
            "search_changed receives {committed:?}"
        );
    }
}

#[test]
fn android_ime_composing_then_committing_a_polish_letter_keeps_the_letter() {
    let (app, queries) = focused_search();
    // Keyboards compose a long-press or dead-key alternate before committing.
    app.window().dispatch_event(android_compose("", "l"));
    app.window().dispatch_event(android_compose("", "ł"));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        field_value(&app).as_deref(),
        Some(""),
        "composing text is not committed text"
    );
    assert_eq!(
        app.get_search_text().as_str(),
        "",
        "composing does not search yet"
    );
    assert!(
        queries.borrow().iter().all(|query| query.is_empty()),
        "composing never reports a partial query: {:?}",
        queries.borrow()
    );
    app.window().dispatch_event(android_commit("ł"));
    app.window().dispatch_event(android_compose("ł", "ó"));
    app.window().dispatch_event(android_commit("łó"));
    app.window().dispatch_event(android_compose("łó", "ź"));
    app.window().dispatch_event(android_commit("łóź"));
    slint::platform::update_timers_and_animations();

    assert_eq!(field_value(&app).as_deref(), Some("łóź"));
    assert_eq!(app.get_search_text().as_str(), "łóź");
    assert_eq!(queries.borrow().last().map(String::as_str), Some("łóź"));
}

#[test]
fn compact_window_accepts_polish_commits_too() {
    let (app, queries) = focused_compact_search();
    app.window().dispatch_event(android_commit(POLISH_LOWER));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_search_text().as_str(), POLISH_LOWER);
    assert_eq!(
        queries.borrow().last().map(String::as_str),
        Some(POLISH_LOWER)
    );
}

#[test]
fn deleting_and_moving_the_caret_around_polish_letters_stays_on_char_boundaries() {
    let (app, queries) = focused_search();
    app.window().dispatch_event(android_commit("Łódź"));
    // Android reports the edited text back with a UTF-16 caret between letters.
    let mut key_event = KeyEvent::default();
    key_event.text = "Łódź".into();
    app.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::CommitComposition,
            replacement_range: Some(i32::MIN..i32::MAX),
            cursor_position: Some("Łó".len() as i32),
            anchor_position: Some("Łó".len() as i32),
            key_event,
            ..Default::default()
        }));
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: slint::platform::Key::Backspace.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: slint::platform::Key::Backspace.into(),
    });
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_search_text().as_str(), "Łdź");
    assert_eq!(queries.borrow().last().map(String::as_str), Some("Łdź"));
}
