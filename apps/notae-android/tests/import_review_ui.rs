//! Layout of the "Review desktop import" sheet (#34): entries are separated cards and the
//! On device / From desktop blocks inside one do not touch, in light and dark themes.

mod support;

use i_slint_backend_testing::ElementHandle;
use i_slint_core::items::ColorScheme;
use i_slint_core::window::WindowInner;
use notae_android::{AppWindow, ImportRow, Theme};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::time::Duration;
use support::app_sized;

const SIZES: [(u32, u32); 2] = [(360, 600), (1280, 800)];
/// The app's stacked-card rhythm is 8 to 12 px (#34); entries sit 8 px apart.
const MIN_GAP: f32 = 8.0;
const MAX_GAP: f32 = 12.0;
/// Within an entry, `spacing` (8) + the 1px divider + `spacing` (8) separates the blocks.
const MIN_BLOCK_GAP: f32 = 16.0;
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

/// Switches the process colour scheme the Material palette reads (`Palette.color-scheme`),
/// the same value Android's system setting feeds through the backend.
fn set_dark_theme(app: &AppWindow, dark: bool) {
    let scheme = if dark {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    };
    WindowInner::from_pub(app.window())
        .context()
        .set_color_scheme(scheme);
    assert_eq!(
        app.global::<Theme>().get_dark(),
        dark,
        "Theme.dark follows the colour scheme"
    );
}

fn open_review(width: u32, height: u32, entries: usize, dark: bool) -> AppWindow {
    let app = app_sized(width, height);
    set_dark_theme(&app, dark);
    app.set_import_rows(ModelRc::new(VecModel::from(
        (1..=entries).map(entry).collect::<Vec<_>>(),
    )));
    app.set_import_preview_text("3 changed".into());
    app.set_show_import_preview(true);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(500));
    app
}

fn theme_name(dark: bool) -> &'static str {
    if dark { "dark" } else { "light" }
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
    for (width, height) in SIZES {
        for dark in [false, true] {
            let case = format!("{width}x{height} {}", theme_name(dark));
            let app = open_review(width, height, 4, dark);
            let cards: Vec<_> = ElementHandle::find_by_element_type_name(&app, CARD).collect();
            assert!(cards.len() >= 2, "{case}: {} cards visible", cards.len());
            for pair in cards.windows(2) {
                let (above, below) = (bounds(&pair[0]), bounds(&pair[1]));
                let gap = below.1 - above.3;
                assert!(
                    (MIN_GAP..=MAX_GAP).contains(&gap),
                    "{case}: entries are {gap} px apart, expected {MIN_GAP}..={MAX_GAP} \
                     ({above:?} then {below:?})"
                );
            }
        }
    }
}

#[test]
fn on_device_and_from_desktop_blocks_do_not_touch() {
    for (width, height) in SIZES {
        for dark in [false, true] {
            let case = format!("{width}x{height} {}", theme_name(dark));
            let app = open_review(width, height, 3, dark);
            let local = bounds(&by_label(
                &app,
                "On device: ID: 1; title: Score 1; composer: —; tags: —",
            ));
            let desktop = bounds(&by_label(
                &app,
                "From desktop: ID: 1; title: Score 1; composer: Composer 1; tags: —",
            ));
            let gap = desktop.1 - local.3;
            assert!(
                gap >= MIN_BLOCK_GAP,
                "{case}: the two blocks are {gap} px apart, expected at least {MIN_BLOCK_GAP} \
                 ({local:?}, {desktop:?})"
            );
        }
    }
}
