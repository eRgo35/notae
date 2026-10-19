//! Library grid geometry: phones get a two-column Drive-style grid, tablets keep their layout.

mod support;

use i_slint_backend_testing::ElementHandle;
use notae_android::{AppWindow, LibraryRow, Theme};
use slint::{Global, ModelRc, SharedString, VecModel};
use support::app_sized;

#[derive(Clone, Copy, Debug)]
struct Tile {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

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

/// A Library with `count` scores whose startup transition has finished.
fn library(width: u32, height: u32, count: usize) -> AppWindow {
    let app = app_sized(width, height);
    let rows: Vec<LibraryRow> = (1..=count).map(score).collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
    app
}

/// The tiles that are on screen, in model order.
fn tiles(app: &AppWindow, count: usize) -> Vec<Tile> {
    (1..=count)
        .filter_map(|index| {
            let tile =
                ElementHandle::find_by_accessible_label(app, &format!("Score {index}")).next()?;
            let (position, size) = (tile.absolute_position(), tile.size());
            Some(Tile {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            })
        })
        .collect()
}

/// `(x, y, width, height)`.
type Bounds = (f32, f32, f32, f32);

fn columns(tiles: &[Tile]) -> usize {
    let mut xs: Vec<i32> = tiles
        .iter()
        .map(|tile| (tile.x * 10.0).round() as i32)
        .collect();
    xs.sort_unstable();
    xs.dedup();
    xs.len()
}

fn near(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 0.01
}

/// Tile bounds measured before the phone layout existed: `(x, y, width, height)` of the first tile
/// and of the first tile of the second row. The tablet and wide layouts must not move. `y` is
/// relative to where the first tile sits on the host, because the header height depends on fonts.
#[test]
fn tablet_and_wide_tile_geometry_is_unchanged() {
    let baseline: [(u32, u32, Bounds, Bounds); 8] = [
        (
            1280,
            800,
            (340.0, 136.0, 214.0, 342.2),
            (340.0, 498.2, 214.0, 342.2),
        ),
        (
            900,
            1400,
            (36.0, 196.0, 195.0, 317.5),
            (36.0, 533.5, 195.0, 317.5),
        ),
        (
            800,
            1280,
            (36.0, 182.0, 232.0, 365.6),
            (36.0, 567.6, 232.0, 365.6),
        ),
        (
            700,
            600,
            (36.0, 182.0, 198.66667, 322.26666),
            (36.0, 524.26666, 198.66667, 322.26666),
        ),
        (
            1920,
            1080,
            (340.0, 136.0, 179.0, 296.7),
            (340.0, 452.7, 179.0, 296.7),
        ),
        (
            840,
            700,
            (340.0, 136.0, 224.0, 355.2),
            (340.0, 511.2, 224.0, 355.2),
        ),
        (
            839,
            700,
            (36.0, 182.0, 179.75, 297.675),
            (36.0, 499.675, 179.75, 297.675),
        ),
        (
            600,
            900,
            (36.0, 182.0, 256.0, 396.8),
            (36.0, 598.8, 256.0, 396.8),
        ),
    ];
    for (width, height, first, second_row) in baseline {
        let app = library(width, height, 9);
        let found = tiles(&app, 9);
        // The grid starts below a header whose text height comes from the system fonts, so the
        // absolute `y` differs between hosts (Noto Sans on Fedora, DejaVu Sans on the CI image).
        // Everything else, including the distance between rows, is font independent.
        let top = found[0].y - first.1;
        let matches = |tile: &Tile, (x, y, w, h): Bounds| {
            near(tile.x, x) && near(tile.y, top + y) && near(tile.width, w) && near(tile.height, h)
        };
        assert!(
            matches(&found[0], first),
            "{width}x{height}: first tile {:?}, expected {first:?} below a header offset of {top}",
            found[0]
        );
        assert!(
            found.iter().any(|tile| matches(tile, second_row)),
            "{width}x{height}: no tile at {second_row:?} (header offset {top}) in {found:?}"
        );
    }
}

fn assert_phone_grid(width: u32, height: u32, dark: bool) {
    let app = library(width, height, 12);
    Theme::get(&app).set_dark(dark);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(50));
    let (w, h) = (width as f32, height as f32);
    let found = tiles(&app, 12);
    let label = format!("{width}x{height} dark={dark}");
    assert!(found.len() >= 4, "{label}: tiles on screen, got {found:?}");

    let columns = columns(&found);
    assert!(
        (2..=3).contains(&columns),
        "{label}: {columns} columns, tiles {found:?}"
    );
    if width < 500 {
        assert_eq!(columns, 2, "{label}");
    }
    for tile in &found {
        assert!(
            tile.x >= 0.0 && tile.x + tile.width <= w,
            "{label}: {tile:?}"
        );
        assert!(
            tile.width >= 140.0 && tile.width <= 260.0 && tile.width < w * 0.55,
            "{label}: tile width bounded, got {tile:?}"
        );
        assert!(
            near(tile.height, tile.width * 1.2 + 64.0),
            "{label}: one aspect ratio, got {tile:?}"
        );
        assert!(
            tile.height < h * 0.45,
            "{label}: a tile is a fraction of the view, got {tile:?}"
        );
    }
    assert!(
        found[0].y + found[0].height <= h && found[1].y + found[1].height <= h,
        "{label}: the first row is entirely inside the window"
    );

    // Neighbours: 12 px apart on both axes, and never overlapping.
    for (index, a) in found.iter().enumerate() {
        for b in &found[index + 1..] {
            let separated_x = a.x + a.width <= b.x + 0.01 || b.x + b.width <= a.x + 0.01;
            let separated_y = a.y + a.height <= b.y + 0.01 || b.y + b.height <= a.y + 0.01;
            assert!(separated_x || separated_y, "{label}: {a:?} overlaps {b:?}");
        }
    }
    let gap_x = found[1].x - (found[0].x + found[0].width);
    assert!(near(gap_x, 12.0), "{label}: horizontal gap {gap_x}");
    let below = found
        .iter()
        .find(|tile| near(tile.x, found[0].x) && tile.y > found[0].y)
        .expect("a second row is on screen");
    let gap_y = below.y - (found[0].y + found[0].height);
    assert!(near(gap_y, 12.0), "{label}: vertical gap {gap_y}");
}

#[test]
fn phones_show_two_bounded_cards_per_row_instead_of_one_view_sized_tile() {
    // 360x640 and 411x891 used to get ONE tile 288x438 / 339x505 logical px wide and tall.
    for dark in [false, true] {
        assert_phone_grid(360, 640, dark);
        assert_phone_grid(411, 891, dark);
    }
}

#[test]
fn phone_tile_sizes_are_stable() {
    let app = library(360, 640, 4);
    let found = tiles(&app, 4);
    assert!(
        near(found[0].width, 146.0) && near(found[0].height, 239.2),
        "{found:?}"
    );
    let app = library(411, 891, 4);
    let found = tiles(&app, 4);
    assert!(
        near(found[0].width, 171.5) && near(found[0].height, 269.8),
        "{found:?}"
    );
}

#[test]
fn larger_phone_widths_may_show_three_columns_and_stay_bounded() {
    let app = library(599, 900, 12);
    let found = tiles(&app, 12);
    assert_eq!(columns(&found), 3, "{found:?}");
    assert!(found.iter().all(|tile| tile.width <= 260.0), "{found:?}");
}

#[test]
fn a_landscape_phone_is_compact_by_its_height() {
    // 891x411: wide enough for the sidebar, but a 342 px high tile would fill the whole view.
    let app = library(891, 411, 12);
    let found = tiles(&app, 12);
    assert!(columns(&found) >= 2, "{found:?}");
    assert!(
        found
            .iter()
            .all(|tile| tile.height < 411.0 * 0.7 && tile.width <= 260.0),
        "{found:?}"
    );
}

#[test]
fn phone_cards_keep_order_and_the_scroll_range_covers_every_row() {
    let app = library(360, 640, 12);
    let found = tiles(&app, 12);
    for pair in found.windows(2) {
        assert!(
            (pair[1].y, pair[1].x) > (pair[0].y, pair[0].x),
            "model order: {pair:?}"
        );
    }
    let grid = ElementHandle::find_by_element_type_name(&app, "ScoreGrid")
        .next()
        .expect("grid is shown");
    // Twelve scores in two columns are six rows of 239.2 px plus gaps.
    let rows_height = 6.0 * (239.2 + 12.0);
    assert!(
        grid.size().height < rows_height,
        "the grid scrolls: viewport {} content {rows_height}",
        grid.size().height
    );
}

#[test]
fn list_rows_are_not_affected_by_the_phone_grid() {
    for (width, height, row_width) in [(360, 640, 300.0), (411, 891, 351.0), (1280, 800, 916.0)] {
        let app = app_sized(width, height);
        app.set_library_list_view(true);
        app.set_library_rows(ModelRc::new(VecModel::from(vec![score(1), score(2)])));
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
        let found = tiles(&app, 2);
        assert_eq!(found.len(), 2);
        for row in &found {
            assert!(near(row.height, 80.0), "{width}x{height}: {row:?}");
            assert!(
                near(row.width, row_width),
                "{width}x{height}: full-width row {row:?}"
            );
        }
        assert!(
            near(found[1].y - found[0].y, 80.0),
            "{width}x{height}: {found:?}"
        );
    }
}
