//! The Android accessibility snapshot: what TalkBack would be told about the real UI.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use notae_android::accessibility::{
    self, Action, FLAG_CLICK, FLAG_EDITABLE, FLAG_ENABLED, FLAG_HAS_SELECTION, HOST_PARENT, Node,
    Refusal, Registry, Role, Snapshot, WIRE_HEADER_INTS, WIRE_NODE_INTS, WIRE_NODE_STRINGS,
};
use notae_android::{AppWindow, LibraryRow, SectionItem};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use support::app;

const SEARCH_LABEL: &str = "Search titles, composers, tags or lyrics";

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

fn with_library(app: &AppWindow, count: usize) {
    let rows: Vec<LibraryRow> = (1..=count).map(score).collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
    app.set_sections(ModelRc::new(VecModel::from(vec![SectionItem {
        key: "all".into(),
        label: "All scores".into(),
        count: count as i32,
        kind: "".into(),
    }])));
}

fn snapshot(app: &AppWindow) -> (Snapshot, Registry) {
    let mut registry = Registry::default();
    let snapshot = accessibility::build(app.window(), &mut registry);
    (snapshot, registry)
}

fn labelled<'a>(snapshot: &'a Snapshot, label: &str) -> &'a Node {
    snapshot
        .nodes
        .iter()
        .find(|node| node.label == label)
        .unwrap_or_else(|| panic!("`{label}` is exposed; have {:?}", labels(snapshot)))
}

fn labels(snapshot: &Snapshot) -> Vec<&str> {
    snapshot
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .collect()
}

#[test]
fn library_cards_become_labelled_clickable_buttons_inside_the_window() {
    let app = app();
    with_library(&app, 3);
    let (snapshot, _) = snapshot(&app);

    for index in 1..=3 {
        let node = labelled(&snapshot, &format!("Score {index}"));
        assert_eq!(node.role, Role::Button);
        assert!(node.has(FLAG_CLICK), "a card has a default action");
        assert!(node.has(FLAG_ENABLED));
        assert!(node.description.contains(&format!("Composer {index}")));
        let [left, top, right, bottom] = node.bounds;
        assert!(right > left && bottom > top, "{node:?} has an area");
        assert!(
            left >= 0 && top >= 0 && right <= 1280 && bottom <= 800,
            "{node:?} is on screen"
        );
        assert_eq!(node.parent, snapshot_parent_of(&snapshot, node));
    }
}

/// Every node's parent is the host or an earlier node, so Java can build child lists in order.
fn snapshot_parent_of(snapshot: &Snapshot, node: &Node) -> i32 {
    assert!(
        node.parent == HOST_PARENT
            || snapshot
                .nodes
                .iter()
                .take_while(|n| n.id != node.id)
                .any(|n| n.id == node.parent),
        "parent of {node:?} precedes it"
    );
    node.parent
}

#[test]
fn ids_are_unique_and_survive_a_rebuild() {
    let app = app();
    with_library(&app, 3);
    let mut registry = Registry::default();
    let first = accessibility::build(app.window(), &mut registry);
    let second = accessibility::build(app.window(), &mut registry);

    let mut ids: Vec<i32> = first.nodes.iter().map(|node| node.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), first.nodes.len(), "ids are unique");
    assert_eq!(
        first, second,
        "an unchanged UI rebuilds to an identical snapshot"
    );
}

#[test]
fn text_only_groups_are_not_nodes_and_order_follows_the_tree() {
    let app = app();
    with_library(&app, 3);
    let (snapshot, _) = snapshot(&app);
    let positions: Vec<usize> = (1..=3)
        .map(|index| {
            snapshot
                .nodes
                .iter()
                .position(|node| node.label == format!("Score {index}"))
                .expect("card exposed")
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "model order {positions:?}"
    );
    assert!(
        snapshot.nodes.iter().all(|node| {
            node.role != Role::Generic || !node.label.is_empty() || node.has(FLAG_CLICK)
        }),
        "role-less nodes without text or actions are pruned"
    );
}

#[test]
fn click_runs_the_slint_default_action_of_exactly_that_card_and_its_overflow_sheet() {
    let app = app();
    with_library(&app, 3);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let previewed = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&opened);
    app.on_open_pdf_requested(move |path, key| {
        recorded
            .borrow_mut()
            .push((path.to_string(), key.to_string()));
    });
    let recorded = Rc::clone(&previewed);
    app.on_score_selected(move |key| recorded.borrow_mut().push(key.to_string()));
    let (snapshot, registry) = snapshot(&app);

    // A card opens the PDF (#26); it does not open the preview.
    let id = labelled(&snapshot, "Score 2").id;
    accessibility::perform(app.window(), &registry, id, &Action::Click).expect("card clicks");
    assert_eq!(
        *opened.borrow(),
        [("score-2.pdf".to_owned(), format!("{:064}", 2))]
    );
    assert!(previewed.borrow().is_empty());

    // Its overflow button (the TalkBack route to the actions) opens the sheet for exactly that
    // score; the sheet's Score details then previews it without launching the PDF.
    let more = labelled(&snapshot, "More actions for Score 2").id;
    accessibility::perform(app.window(), &registry, more, &Action::Click)
        .expect("overflow button clicks");
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
    assert!(
        previewed.borrow().is_empty(),
        "opening the sheet previews nothing"
    );
    let mut registry = Registry::default();
    let open_snapshot = accessibility::build(app.window(), &mut registry);
    let sheet = labelled(&open_snapshot, "Score actions dialog");
    assert_eq!(sheet.role, Role::Pane);
    let details = open_snapshot
        .nodes
        .iter()
        .find(|node| node.label == "Score details")
        .expect("the sheet offers Score details")
        .id;
    accessibility::perform(app.window(), &registry, details, &Action::Click)
        .expect("sheet action clicks");
    assert_eq!(*previewed.borrow(), [format!("{:064}", 2)]);
    assert_eq!(opened.borrow().len(), 1);
}

#[test]
fn actions_on_vanished_or_unsupported_nodes_are_refused_not_ignored() {
    let app = app();
    with_library(&app, 2);
    let (snapshot, registry) = snapshot(&app);
    assert_eq!(
        accessibility::perform(app.window(), &registry, 99_999, &Action::Click),
        Err(Refusal::StaleNode)
    );
    let card = labelled(&snapshot, "Score 1").id;
    assert_eq!(
        accessibility::perform(app.window(), &registry, card, &Action::SetText("x".into())),
        Err(Refusal::Unsupported)
    );
}

#[test]
fn removed_items_leave_the_snapshot_and_their_ids_go_stale() {
    let app = app();
    with_library(&app, 3);
    let mut registry = Registry::default();
    let before = accessibility::build(app.window(), &mut registry);
    let gone = labelled(&before, "Score 3").id;

    with_library(&app, 2);
    let after = accessibility::build(app.window(), &mut registry);
    assert!(after.nodes.iter().all(|node| node.label != "Score 3"));
    assert_eq!(
        accessibility::perform(app.window(), &registry, gone, &Action::Click),
        Err(Refusal::StaleNode)
    );
}

fn open_search(app: &AppWindow) {
    app.set_show_search(true);
    slint::platform::update_timers_and_animations();
}

#[test]
fn search_field_is_an_editable_text_input_that_accepts_set_text() {
    let app = app();
    open_search(&app);
    let queries = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&queries);
    app.on_search_changed(move |query| recorded.borrow_mut().push(query.to_string()));
    let (snapshot, registry) = snapshot(&app);

    let field = labelled(&snapshot, SEARCH_LABEL);
    assert_eq!(field.role, Role::TextInput);
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| node.role == Role::TextInput)
            .count(),
        1,
        "the wrapper and its built-in TextInput are one editable node"
    );
    assert!(field.has(FLAG_EDITABLE) && field.has(FLAG_HAS_SELECTION));
    assert_eq!(field.hint, SEARCH_LABEL);

    accessibility::perform(
        app.window(),
        &registry,
        field.id,
        &Action::SetText("łódź".into()),
    )
    .expect("set text");
    assert_eq!(*queries.borrow(), ["łódź"]);
    assert_eq!(app.get_search_text().as_str(), "łódź");

    let (after, registry) = {
        let mut registry = registry;
        let snapshot = accessibility::build(app.window(), &mut registry);
        (snapshot, registry)
    };
    let field = labelled(&after, SEARCH_LABEL);
    assert_eq!(field.value, "łódź");
    accessibility::perform(
        app.window(),
        &registry,
        field.id,
        &Action::SetSelection(1, 3),
    )
    .expect("select");
    let mut registry = registry;
    let selected = accessibility::build(app.window(), &mut registry);
    assert_eq!(
        labelled(&selected, SEARCH_LABEL).selection,
        Some((1, 3)),
        "UTF-16 offsets 1..3 of \"łódź\" select \"ód\""
    );
}

#[test]
fn clicking_a_text_field_moves_input_focus_to_it() {
    let app = app();
    open_search(&app);
    let (snapshot, registry) = snapshot(&app);
    let field = labelled(&snapshot, SEARCH_LABEL);
    assert!(!field.has(accessibility::FLAG_FOCUSED) || snapshot.focused == Some(field.id));

    accessibility::perform(app.window(), &registry, field.id, &Action::Click).expect("focus");
    let mut registry = registry;
    let after = accessibility::build(app.window(), &mut registry);
    let field = labelled(&after, SEARCH_LABEL);
    assert!(field.has(accessibility::FLAG_FOCUSED), "{field:?}");
    assert_eq!(after.focused, Some(field.id));
}

#[test]
fn dangling_description_separators_are_trimmed() {
    let app = app();
    let mut row = score(1);
    row.composer_or_artist = "".into();
    app.set_library_rows(ModelRc::new(VecModel::from(vec![row])));
    let (snapshot, _) = snapshot(&app);
    let card = labelled(&snapshot, "Score 1");
    assert_eq!(
        card.description, "Opens the score",
        "no leading separator when the composer is empty"
    );
}

/// The per-item overflow button is a labelled, clickable button of its own, there is one per
/// score, and neither the Info icon nor an Edit action is on an item.
#[test]
fn every_card_has_a_labelled_overflow_button_and_no_info_or_edit_button() {
    let app = app();
    with_library(&app, 3);
    let (snapshot, _) = snapshot(&app);

    for index in 1..=3 {
        let node = labelled(&snapshot, &format!("More actions for Score {index}"));
        assert_eq!(node.role, Role::Button);
        assert!(node.has(FLAG_CLICK) && node.has(FLAG_ENABLED));
        let [left, top, right, bottom] = node.bounds;
        assert!(right > left && bottom > top, "{node:?} has an area");
    }
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.label.contains("Edit details") || node.label == "Score details"),
        "items expose neither an Info nor an Edit action"
    );
}

#[test]
fn encoding_is_one_fixed_stride_record_per_node() {
    let app = app();
    with_library(&app, 2);
    let (snapshot, _) = snapshot(&app);
    let wire = snapshot.encode();

    assert_eq!(wire.ints[0], accessibility::WIRE_VERSION);
    assert_eq!(wire.ints[1] as usize, snapshot.nodes.len());
    assert_eq!(
        wire.ints.len(),
        WIRE_HEADER_INTS + snapshot.nodes.len() * WIRE_NODE_INTS
    );
    assert_eq!(wire.strings.len(), snapshot.nodes.len() * WIRE_NODE_STRINGS);

    let index = snapshot
        .nodes
        .iter()
        .position(|n| n.label == "Score 2")
        .expect("card");
    let record = &wire.ints[WIRE_HEADER_INTS + index * WIRE_NODE_INTS..][..WIRE_NODE_INTS];
    let node = &snapshot.nodes[index];
    assert_eq!(record[0], node.id);
    assert_eq!(record[1], node.parent);
    assert_eq!(record[2], Role::Button as i32);
    assert_eq!(record[3], node.flags);
    assert_eq!(&record[4..8], &node.bounds);
    assert_eq!(wire.strings[index * WIRE_NODE_STRINGS], "Score 2");
}

#[test]
fn utf16_and_utf8_offsets_round_trip_over_multibyte_text() {
    let text = "a łódź 🎼 b";
    for (byte, _) in text.char_indices() {
        let utf16 = accessibility::utf16_index(text, byte);
        assert_eq!(
            accessibility::utf8_offset(text, utf16),
            byte,
            "offset {byte}"
        );
    }
    assert_eq!(accessibility::utf8_offset(text, 9_999), text.len());
    assert_eq!(accessibility::utf8_offset(text, -3), 0);
    assert_eq!(
        accessibility::utf16_index(text, 9_999),
        text.encode_utf16().count() as i32
    );
    // A byte offset inside a code point clamps back to its start.
    assert_eq!(accessibility::utf16_index("łx", 1), 0);
}
