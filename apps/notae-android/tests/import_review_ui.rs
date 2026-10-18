//! Layout of the "Review desktop import" sheet (#34): entries are separated cards and the
//! On device / From desktop blocks inside one do not touch.

mod support;

use i_slint_backend_testing::ElementHandle;
use notae_android::{AppWindow, ImportRow};
use slint::{ModelRc, VecModel};
use std::time::Duration;
use support::app_sized;

/// The vertical rhythm the rest of the app uses between stacked cards is 8 to 16 px.
const MIN_GAP: f32 = 8.0;
const CARD: &str = "ImportCard";

fn entry(index: usize) -> ImportRow {
    ImportRow {
        path: format!("{index:02} - Score {index}.pdf").into(),
        change: "Changed".into(),
        local_metadata: format!("ID: {index}; title: Score {index}; composer: —; tags: —").into(),
        proposed_metadata: format!(
            "ID: {index}; title: Score {index}; composer: Composer {index}; tags: —"
        )
        .into(),
        fields: "Imported: composer; preserved: —. ".into(),
        ocr: "OCR available".into(),
        conflict: "".into(),
    }
}

fn open_review(width: u32, height: u32, entries: usize) -> AppWindow {
    let app = app_sized(width, height);
    app.set_import_rows(ModelRc::new(VecModel::from(
        (1..=entries).map(entry).collect::<Vec<_>>(),
    )));
    app.set_import_preview_text("3 changed".into());
    app.set_show_import_preview(true);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(500));
    app
}

fn bounds(element: &ElementHandle) -> (f32, f32, f32, f32) {
    let at = element.absolute_position();
    let size = element.size();
    (at.x, at.y, at.x + size.width, at.y + size.height)
}

fn by_label(app: &AppWindow, text: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, text)
        .next()
        .unwrap_or_else(|| panic!("`{text}` is exposed"))
}

#[test]
fn import_entries_are_separated_cards_with_a_gap() {
    for (width, height) in [(360, 600), (1280, 800)] {
        let app = open_review(width, height, 4);
        let cards: Vec<_> = ElementHandle::find_by_element_type_name(&app, CARD).collect();
        assert!(
            cards.len() >= 2,
            "{width}x{height}: {} cards visible",
            cards.len()
        );
        for pair in cards.windows(2) {
            let (above, below) = (bounds(&pair[0]), bounds(&pair[1]));
            let gap = below.1 - above.3;
            assert!(
                gap >= MIN_GAP,
                "{width}x{height}: entries are {gap} px apart ({above:?} then {below:?})"
            );
        }
    }
}

#[test]
fn on_device_and_from_desktop_blocks_do_not_touch() {
    for (width, height) in [(360, 600), (1280, 800)] {
        let app = open_review(width, height, 3);
        let local = bounds(&by_label(
            &app,
            "On device: ID: 1; title: Score 1; composer: —; tags: —",
        ));
        let desktop = bounds(&by_label(
            &app,
            "From desktop: ID: 1; title: Score 1; composer: Composer 1; tags: —",
        ));
        assert!(
            desktop.1 - local.3 >= MIN_GAP,
            "{width}x{height}: the two blocks are {} px apart ({local:?}, {desktop:?})",
            desktop.1 - local.3
        );
    }
}
