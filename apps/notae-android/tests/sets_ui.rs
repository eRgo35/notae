//! The Sets section: folder cards and rows, the new/edit set dialog and the delete confirmation.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use notae_android::sets::{SetFieldError, SetFields, SetStore};
use notae_android::ui::UiModel;
use notae_android::ui::browse::LibrarySection;
use notae_android::{AppWindow, LibraryRow, SetCard, Theme};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Global, ModelRc, SharedString, VecModel};
use support::app_sized;

fn wait(milliseconds: u64) {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(milliseconds));
    slint::platform::update_timers_and_animations();
}

fn card(id: &str, name: &str) -> SetCard {
    SetCard {
        id: id.into(),
        name: name.into(),
        date: "1 Nov 2026".into(),
        count: "7 items".into(),
        edited: "Edited today".into(),
    }
}

/// A settled Sets section with two sets, at a tablet size.
fn sets_screen(width: u32, height: u32, list: bool, dark: bool) -> AppWindow {
    let app = app_sized(width, height);
    app.set_library_list_view(list);
    app.set_selected_section("sets".into());
    app.set_library_title("Sets".into());
    app.set_set_cards(ModelRc::new(VecModel::from(vec![
        card("aaaaaaaaaaaaaaaa", "Wszystkich Świętych"),
        card("bbbbbbbbbbbbbbbb", "Dzień Zaduszny"),
    ])));
    Theme::get(&app).set_dark(dark);
    wait(500);
    app
}

fn labelled(app: &AppWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(app, label).next()
}

fn click(element: &ElementHandle) {
    element.mock_single_click(PointerEventButton::Left);
}

/// The element labelled `label` inside the element labelled `container`.
fn inside(app: &AppWindow, container: &str, label: &str) -> ElementHandle {
    let outer = labelled(app, container).unwrap_or_else(|| panic!("{container} is shown"));
    let (o, size) = (outer.absolute_position(), outer.size());
    ElementHandle::find_by_accessible_label(app, label)
        .find(|element| {
            let p = element.absolute_position();
            p.x >= o.x && p.y >= o.y && p.x <= o.x + size.width && p.y <= o.y + size.height
        })
        .unwrap_or_else(|| panic!("{label} inside {container}"))
}

fn count_of(app: &AppWindow, type_name: &str) -> usize {
    ElementHandle::find_by_element_type_name(app, type_name).count()
}

/// Tablet sizes in both orientations, light and dark.
fn each_tablet(check: impl Fn(u32, u32, bool, &str)) {
    for (width, height) in [(1280, 800), (800, 1280)] {
        for dark in [false, true] {
            check(
                width,
                height,
                dark,
                &format!("{width}x{height} dark={dark}"),
            );
        }
    }
}

#[test]
fn sets_grid_shows_folder_cards() {
    each_tablet(|width, height, dark, context| {
        let app = sets_screen(width, height, false, dark);
        assert_eq!(count_of(&app, "SetFolderCard"), 2, "{context}");
        assert_eq!(count_of(&app, "SetRow"), 0, "{context}");
        assert_eq!(count_of(&app, "ScoreCard"), 0, "{context}");
        assert!(
            labelled(&app, "New set").is_some(),
            "{context}: header action"
        );
        assert!(
            labelled(&app, "Search").is_some(),
            "{context}: one global search"
        );
        for name in ["Wszystkich Świętych", "Dzień Zaduszny"] {
            let overflow = labelled(&app, &format!("More actions for {name}"))
                .unwrap_or_else(|| panic!("{context}: ⋮ for {name}"));
            assert!(overflow.size().width >= 48.0, "{context}");
        }
        assert!(
            labelled(&app, "No scores here").is_none(),
            "{context}: no empty state over the cards"
        );

        let opened = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&opened);
        app.on_set_opened(move |id| seen.borrow_mut().push(id.to_string()));
        let folder = ElementHandle::find_by_element_type_name(&app, "SetFolderCard")
            .nth(1)
            .expect("second card");
        click(&folder);
        assert_eq!(*opened.borrow(), ["bbbbbbbbbbbbbbbb"], "{context}");
    });
}

#[test]
fn sets_list_shows_set_rows() {
    each_tablet(|width, height, dark, context| {
        let app = sets_screen(width, height, true, dark);
        assert_eq!(count_of(&app, "SetRow"), 2, "{context}");
        assert_eq!(count_of(&app, "SetFolderCard"), 0, "{context}");
    });
}

#[test]
fn an_empty_sets_section_invites_a_new_set() {
    let app = app_sized(1280, 800);
    app.set_selected_section("sets".into());
    wait(500);
    assert!(labelled(&app, "No sets yet").is_some());
    let requested = Rc::new(RefCell::new(0));
    let seen = Rc::clone(&requested);
    app.on_new_set_requested(move || *seen.borrow_mut() += 1);
    click(&labelled(&app, "New set").expect("New set button"));
    assert_eq!(*requested.borrow(), 1);
}

#[test]
fn an_open_set_without_items_shows_its_own_empty_state() {
    let app = app_sized(1280, 800);
    app.set_selected_section("set:aaaaaaaaaaaaaaaa".into());
    app.set_library_title("Wszystkich Świętych".into());
    wait(500);
    assert!(labelled(&app, "This set is empty").is_some());
    assert!(labelled(&app, "No scores by this composer are in the library.").is_none());
}

fn open_dialog(app: &AppWindow, name: &str) {
    app.set_set_dialog_id("".into());
    app.set_set_dialog_name(name.into());
    app.set_set_dialog_date("".into());
    app.set_set_dialog_notes("".into());
    app.set_set_dialog_error("".into());
    app.set_show_set_dialog(true);
    wait(500);
}

#[test]
fn new_set_dialog_requires_name() {
    each_tablet(|width, height, dark, context| {
        let app = sets_screen(width, height, false, dark);
        let saved = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&saved);
        app.on_set_dialog_saved(move |id, name, date, notes| {
            seen.borrow_mut().push((
                id.to_string(),
                name.to_string(),
                date.to_string(),
                notes.to_string(),
            ));
        });
        open_dialog(&app, "");
        assert!(labelled(&app, "Set details dialog").is_some(), "{context}");
        let save = inside(&app, "Set details dialog", "Save set");
        click(&save);
        assert!(saved.borrow().is_empty(), "{context}: nothing saved");
        assert_eq!(app.get_set_dialog_error(), "Name is required", "{context}");
        assert!(labelled(&app, "Name is required").is_some(), "{context}");

        app.set_set_dialog_name("Dzień Zaduszny".into());
        app.set_set_dialog_date("2026-11-02".into());
        click(&save);
        assert_eq!(
            *saved.borrow(),
            [(
                String::new(),
                "Dzień Zaduszny".to_owned(),
                "2026-11-02".to_owned(),
                String::new()
            )],
            "{context}"
        );
    });
}

#[test]
fn the_date_field_has_a_clear_button() {
    let app = sets_screen(1280, 800, false, false);
    open_dialog(&app, "Mass");
    app.set_set_dialog_date("2026-11-02".into());
    wait(100);
    click(&labelled(&app, "Clear date").expect("Clear date button"));
    assert_eq!(app.get_set_dialog_date(), "");
}

#[test]
fn delete_asks_for_confirmation_naming_the_set() {
    each_tablet(|width, height, dark, context| {
        let app = sets_screen(width, height, true, dark);
        let requested = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&requested);
        app.on_delete_set_requested(move |id| seen.borrow_mut().push(id.to_string()));
        let confirmed = Rc::new(RefCell::new(0));
        let seen = Rc::clone(&confirmed);
        app.on_delete_set_confirmed(move || *seen.borrow_mut() += 1);
        let cancelled = Rc::new(RefCell::new(0));
        let seen = Rc::clone(&cancelled);
        app.on_delete_set_cancelled(move || *seen.borrow_mut() += 1);

        click(&labelled(&app, "More actions for Dzień Zaduszny").expect("⋮"));
        wait(500);
        assert!(labelled(&app, "Edit set").is_some(), "{context}");
        click(&labelled(&app, "Delete set").expect("Delete set action"));
        wait(500);
        assert_eq!(*requested.borrow(), ["bbbbbbbbbbbbbbbb"], "{context}");
        assert_eq!(*confirmed.borrow(), 0, "{context}: asking deletes nothing");

        // Rust names the set it is about to delete.
        app.set_confirm_delete_set_name("Dzień Zaduszny".into());
        wait(500);
        assert!(
            labelled(&app, "Delete “Dzień Zaduszny”?").is_some(),
            "{context}: the question names the set"
        );
        click(&inside(&app, "Delete set dialog", "Cancel"));
        assert_eq!(
            (*cancelled.borrow(), *confirmed.borrow()),
            (1, 0),
            "{context}"
        );
        click(&inside(&app, "Delete set dialog", "Delete"));
        assert_eq!(*confirmed.borrow(), 1, "{context}");
    });
}

#[test]
fn edit_set_routes_the_set_id() {
    let app = sets_screen(1280, 800, false, false);
    let edited = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&edited);
    app.on_edit_set_requested(move |id| seen.borrow_mut().push(id.to_string()));
    click(&labelled(&app, "More actions for Wszystkich Świętych").expect("⋮"));
    wait(500);
    click(&labelled(&app, "Edit set").expect("Edit set action"));
    wait(500);
    assert_eq!(*edited.borrow(), ["aaaaaaaaaaaaaaaa"]);
    assert!(!app.get_library_menu_open(), "the sheet closes");
}

// --- Inside a set ---

fn key(app: &AppWindow, key: Key) {
    let text: SharedString = key.into();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn set_row(position: &str, title: &str, missing: bool) -> LibraryRow {
    LibraryRow {
        title: title.into(),
        note_key: title.repeat(8).into(),
        relative_path: format!("{title}.pdf").into(),
        item_id: format!("item-{position}").into(),
        position: position.into(),
        missing,
        status: if missing { "Missing from library" } else { "" }.into(),
        ..LibraryRow::default()
    }
}

/// A settled open set with pieces at positions 1, 3 and 4 (4 is missing from the library).
fn set_screen(width: u32, height: u32, list: bool, dark: bool, notes: &str) -> AppWindow {
    let app = app_sized(width, height);
    app.set_library_list_view(list);
    app.set_selected_section("set:aaaaaaaaaaaaaaaa".into());
    app.set_library_title("Wszystkich Świętych".into());
    app.set_library_subtitle("1 Nov 2026 · 3 pieces · 1 note".into());
    app.set_library_notes(notes.into());
    app.set_library_rows(ModelRc::new(VecModel::from(vec![
        set_row("1", "Kyrie", false),
        set_row("3", "Gloria", false),
        set_row("4", "Psalm 23", true),
    ])));
    Theme::get(&app).set_dark(dark);
    wait(500);
    app
}

#[test]
fn set_header_back_returns_to_sets() {
    each_tablet(|width, height, dark, context| {
        let app = set_screen(width, height, false, dark, "Uroczystość · 10:00");
        let selected = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&selected);
        app.on_section_selected(move |key| seen.borrow_mut().push(key.to_string()));

        assert!(
            labelled(&app, "Wszystkich Świętych").is_some(),
            "{context}: title"
        );
        assert!(
            labelled(&app, "1 Nov 2026 · 3 pieces · 1 note").is_some(),
            "{context}: subtitle"
        );
        assert!(
            labelled(&app, "Uroczystość · 10:00").is_some(),
            "{context}: notes line"
        );
        for action in ["Add", "Reorder", "Search", "Scan library"] {
            let button = labelled(&app, action).unwrap_or_else(|| panic!("{context}: {action}"));
            let right = button.absolute_position().x + button.size().width;
            assert!(right <= width as f32, "{context}: {action} fits the header");
        }
        assert!(
            labelled(&app, "New set").is_none(),
            "{context}: not the Sets list"
        );

        let back = labelled(&app, "Back to sets").expect("back arrow");
        assert!(
            back.size().width >= 48.0,
            "{context}: back is a full target"
        );
        click(&back);
        assert_eq!(*selected.borrow(), ["sets"], "{context}");

        // Android Back (and Escape) inside a set goes to the list of sets.
        key(&app, Key::Back);
        key(&app, Key::Escape);
        assert_eq!(*selected.borrow(), ["sets", "sets", "sets"], "{context}");
    });

    // No notes, no notes line; outside a set, no back arrow.
    let app = set_screen(1280, 800, false, false, "");
    assert_eq!(count_of(&app, "SetNotesLine"), 0);
    app.set_selected_section("all".into());
    wait(500);
    assert!(labelled(&app, "Back to sets").is_none());
    assert!(labelled(&app, "Reorder").is_none());
}

#[test]
fn set_cards_show_position_numbers() {
    each_tablet(|width, height, dark, context| {
        for list in [false, true] {
            let context = format!("{context} list={list}");
            let app = set_screen(width, height, list, dark, "");
            assert_eq!(count_of(&app, "PositionBadge"), 3, "{context}");
            for label in ["1. Kyrie", "3. Gloria", "4. Psalm 23"] {
                assert!(labelled(&app, label).is_some(), "{context}: {label}");
            }

            // A missing piece shows why and opens nothing; its ⋮ has nothing to offer yet.
            assert!(
                labelled(&app, "Missing from library").is_some(),
                "{context}"
            );
            assert!(
                labelled(&app, "More actions for Psalm 23").is_none(),
                "{context}: no ⋮ on a missing row"
            );
            assert!(
                labelled(&app, "More actions for Gloria").is_some(),
                "{context}"
            );
            let opened = Rc::new(RefCell::new(Vec::new()));
            let seen = Rc::clone(&opened);
            app.on_open_pdf_requested(move |path, _| seen.borrow_mut().push(path.to_string()));
            click(&labelled(&app, "4. Psalm 23").expect("missing row"));
            assert!(
                opened.borrow().is_empty(),
                "{context}: a missing row opens nothing"
            );
            click(&labelled(&app, "3. Gloria").expect("row"));
            assert_eq!(*opened.borrow(), ["Gloria.pdf"], "{context}");
        }
    });

    // Outside a set, rows carry no position.
    let app = app_sized(1280, 800);
    app.set_library_rows(ModelRc::new(VecModel::from(vec![set_row(
        "", "Kyrie", false,
    )])));
    wait(500);
    assert_eq!(count_of(&app, "PositionBadge"), 0);
    assert!(labelled(&app, "Kyrie").is_some());
}

// --- The model behind the dialog ---

fn fields(name: &str, date: Option<&str>) -> SetFields {
    SetFields {
        name: name.to_owned(),
        date: date.map(str::to_owned),
        notes: None,
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("notae-sets-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temporary directory is created");
    dir
}

#[test]
fn saving_a_new_set_opens_it_and_writes_the_file() {
    let dir = temp_dir("create");
    let path = dir.join("sets.json");
    let mut model = UiModel::new();
    model.set_sets_path(path.clone());
    let id = model
        .create_set(fields("  Dzień Zaduszny ", Some("2026-11-02")))
        .expect("valid fields");
    assert_eq!(model.section(), &LibrarySection::Set(id.clone()));
    assert_eq!(model.section_title(), "Dzień Zaduszny");
    let stored = SetStore::load(&path);
    let set = stored.get(&id).expect("saved to sets.json");
    assert_eq!(set.date.as_deref(), Some("2026-11-02"));

    model
        .update_set(&id, fields("Zaduszki", None))
        .expect("valid fields");
    assert_eq!(
        SetStore::load(&path).get(&id).expect("set").name,
        "Zaduszki"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn invalid_fields_create_nothing() {
    let mut model = UiModel::new();
    assert_eq!(
        model.create_set(fields("   ", None)),
        Err(SetFieldError::EmptyName)
    );
    assert_eq!(
        model.create_set(fields("Mass", Some("2026-02-30"))),
        Err(SetFieldError::InvalidDate)
    );
    assert!(model.sets().sets_by_modified().is_empty());
    assert_eq!(model.section(), &LibrarySection::All);
}

#[test]
fn deleting_waits_for_confirmation_and_leaves_an_open_set() {
    let mut model = UiModel::new();
    let id = model.create_set(fields("Mass", None)).expect("valid");
    model.request_delete_set(&id);
    assert_eq!(model.pending_delete_set_name(), Some("Mass"));
    assert!(model.sets().get(&id).is_some(), "asking deletes nothing");
    model.cancel_delete_set();
    assert_eq!(model.pending_delete_set_name(), None);
    assert!(model.sets().get(&id).is_some());

    model.request_delete_set(&id);
    model.confirm_delete_set();
    assert!(model.sets().get(&id).is_none());
    assert_eq!(model.pending_delete_set_name(), None);
    assert_eq!(
        model.section(),
        &LibrarySection::Sets,
        "the deleted set was open"
    );
}
