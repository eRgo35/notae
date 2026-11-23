# Viewer Layout, Zoom and Notes Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The viewer gets Pages/Scroll layouts, 1/2 pages, fit page/width/height and percentage scales (remembered per device), an animated page turn, pinch zoom anywhere, and a right-hand notes panel on wide windows.

**Architecture:** The core stores three new UI prefs and exposes them in `LayoutPrefs`; Kotlin resolves `auto` against the window into a concrete look, lays rows out with pure functions, and draws them with `HorizontalPager` (Pages) or `LazyColumn` (Scroll) under one zoom modifier that takes two-finger gestures before the pages and the drawing overlay.

**Tech Stack:** Rust (`notae-app-core`, UniFFI), Kotlin/Jetpack Compose (foundation pager, lazy list), Robolectric + Roborazzi.

**Spec:** `docs/superpowers/specs/2026-11-23-viewer-layout-and-zoom-design.md`

## Global Constraints

- No new dependencies (Rust or Gradle). `unsafe` stays forbidden; no unchecked `unwrap`/`expect` and no `!!` in production code.
- Prefs keys: `viewer_layout = auto | pages | scroll`, `viewer_pages = auto | one | two`, `viewer_scale = auto | fit_page | fit_width | fit_height | <percent>`; unknown values fall back to `auto`; a file without the lines reads as `auto`.
- `auto` = today: two pages when landscape, width EXPANDED and height REGULAR, else one; Scroll at fit width when height is COMPACT, else Pages at fit page.
- Percent presets: 50, 75, 100, 125, 150, 200. 100 % = PDF page width in points / 72 × display dpi (`DisplayMetrics.xdpi`).
- Pinch zoom is not stored; double tap returns to the chosen scale.
- An edge tap (outer fifth), arrow key, Page Up/Down or pedal turns exactly one spread (Pages) or to the next/previous row's top (Scroll), animated. The middle third toggles the bars. Escape is Back.
- Notes panel: 360 dp, right side, only at `WidthClass.EXPANDED`; its open state is not remembered.
- In drawing mode a pen pointer is never taken by zoom.
- Strings in `values/strings.xml` and `values-pl/strings.xml`. Copy: View / Widok; Layout / Układ; Pages / Strony; Scroll / Przewijanie; On screen / Na ekranie; 1 page / 1 strona; 2 pages / 2 strony; Scale / Skala; Fit page / Dopasuj stronę (two pages: Fit pages / Dopasuj strony); Fit width / Dopasuj szerokość; Fit height / Dopasuj wysokość; Close notes / Zamknij notatki.
- Conventional Commits; quality gates via `scripts/check.sh` (`fast` while working, `full` at the end).

## Review Focus

1. **Pinch near the screen edge or over a drawing in drawing mode** (the reported bug): zoom works; the edge zone does not also turn the page. → Task 4 `pinch_at_the_edge_zooms_without_turning`, `pinch_over_drawings_zooms_in_drawing_mode`.
2. **Two pages with an odd page count in Scroll** — the last row holds one page, centred; the counter reaches the last page. → Task 3 `rows_pair_pages_odd_last_alone_in_scroll`, Task 5 `edge_tap_reaches_the_last_row`.
3. **Switching layout or pages on screen mid-score**: the same first page stays on screen. → Task 6 `switching_layout_keeps_the_page`.
4. **A page wider than tall (landscape score) at fit height / percent in Pages**: the spread overflows horizontally, a swipe pans instead of turning, an edge tap still turns. → Task 4 `overflowing_spread_pans_on_swipe_and_turns_on_edge_tap`.
5. **Rotation with the notes panel open at EXPANDED, then to a narrower window**: the panel becomes the bottom sheet, not lost and not both. → Task 7 `panel_becomes_sheet_when_narrower`.

---

## File Structure

Core (`crates/notae-app-core/src/`):
- `ui_prefs.rs` — modify: `ViewerLayout`, `ViewerPages`, `ViewerScale` and three fields.
- `ui.rs`, `api.rs`, `api/action.rs`, `api/snapshot.rs` — modify: setters, one action, `LayoutPrefs` fields.

Kotlin (`apps/notae-android/app/src/main/kotlin/com/c2yz/notae/`):
- `platform/PageRenderer.kt` — modify: `PageSource.pageWidthPoints(index)`.
- `ui/viewer/PageLayout.kt` — create: `ViewLook`, `resolveLook`, `rowsFor`, `rowSize` (pure).
- `ui/viewer/Zoom.kt` — create: `PageZoom` (moved from PageArea) and `Modifier.pinchZoom`.
- `ui/viewer/PageArea.kt` — rewrite: `PageArea` dispatching to `PagedRows` (pager) and `ScrolledRows` (lazy column); remove `PageMode`, `pageModeFor`, `spreadsFor`.
- `ui/viewer/ViewMenu.kt` — create: the View menu.
- `ui/viewer/ViewerScreen.kt` — modify: look, menu, notes panel.
- `res/drawable/ic_book_open.xml` — create (Lucide "book-open", same style as the other icons).

---

### Task 1: View prefs in the core

**Files:**
- Modify: `crates/notae-app-core/src/ui_prefs.rs`, `ui.rs`, `api.rs`, `api/action.rs`, `api/snapshot.rs`
- Test: `crates/notae-app-core/tests/ui_prefs.rs`, `crates/notae-app-core/tests/api.rs`

**Interfaces:**
- Produces: `#[derive(uniffi::Enum, Default)] enum ViewerLayout { #[default] Auto, Pages, Scroll }`, `enum ViewerPages { #[default] Auto, One, Two }`, `enum ViewerScale { #[default] Auto, FitPage, FitWidth, FitHeight, Percent { value: u16 } }` (percent accepted 25..=400, else `Auto`); `UiPrefs { viewer_layout, viewer_pages, viewer_scale, .. }`; `Action::SetViewerLook { layout: ViewerLayout, pages: ViewerPages, scale: ViewerScale }` saved like `SetOpenScoresIn` (no write when unchanged); `LayoutPrefs { viewer_layout, viewer_pages, viewer_scale, .. }`. Kotlin sees `ViewerScale.Percent(value: UShort)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn viewer_look_defaults_to_auto_for_old_prefs() {
    let prefs = UiPrefs::from_text("section=all\n");
    assert_eq!((prefs.viewer_layout, prefs.viewer_pages, prefs.viewer_scale), (ViewerLayout::Auto, ViewerPages::Auto, ViewerScale::Auto));
}
#[test] fn viewer_look_round_trips() {
    let prefs = UiPrefs { viewer_layout: ViewerLayout::Scroll, viewer_pages: ViewerPages::Two, viewer_scale: ViewerScale::Percent { value: 125 }, ..UiPrefs::default() };
    assert_eq!(UiPrefs::from_text(&prefs.to_text()), prefs);
}
#[test] fn bad_viewer_values_fall_back_to_auto() {
    let prefs = UiPrefs::from_text("viewer_layout=sideways\nviewer_scale=9000\nviewer_pages=three\n");
    assert_eq!(prefs.viewer_scale, ViewerScale::Auto);
    assert_eq!(prefs.viewer_layout, ViewerLayout::Auto);
}
```

Plus api test `set_viewer_look_is_saved_and_shown`: dispatch `SetViewerLook { Scroll, One, FitHeight }` → `snapshot().layout.viewer_layout == Scroll`, `viewer_scale == FitHeight`, prefs file contains `viewer_layout=scroll` and `viewer_scale=fit_height`.

- [ ] **Step 2: Run** `cargo test -p notae-app-core viewer_look` — Expected: FAIL (no `ViewerLayout`).
- [ ] **Step 3: Implement** enums with `key()`/`from_key()` like `OpenScoresIn` (percent key is the bare number), fields, `to_text` lines, `UiModel::set_viewer_look`, the action, snapshot fields.
- [ ] **Step 4: Run** `cargo test -p notae-app-core` — Expected: PASS. Rebuild bindings as the app build does (`./gradlew :app:assembleDebug` regenerates UniFFI Kotlin).
- [ ] **Step 5: Commit** `feat(core): remember the viewer's layout, pages and scale`

### Task 2: Page size in points

**Files:** Modify `platform/PageRenderer.kt`; `test/.../ui/viewer/FakePages.kt` (add `pointsWidth: Float = 595f` constructor param); Test: existing `PageRenderer` test if present, else covered by Task 3.

**Interfaces:** Produces `fun pageWidthPoints(index: Int): Float` on `PageSource` (PdfRenderer `page.width`, read with the aspects at open; fallback 595f).

- [ ] **Step 1:** Add the member, read widths alongside aspects in `PdfPageSource.open`.
- [ ] **Step 2: Run** `./gradlew -q :app:testDebugUnitTest --tests '*viewer*'` — Expected: PASS (compiles, nothing else changed).
- [ ] **Step 3: Commit** `feat(viewer): know each page's printed width`

### Task 3: Look resolution and row layout (pure)

**Files:** Create `ui/viewer/PageLayout.kt`; Test: `test/.../ui/viewer/PageLayoutTest.kt` (plain JUnit) replacing `SpreadsTest.kt`.

**Interfaces:**
- Consumes: `ViewerLayout`, `ViewerPages`, `ViewerScale` (Task 1), `WidthClass`, `HeightClass`.
- Produces:
  - `data class ViewLook(val scroll: Boolean, val twoPages: Boolean, val scale: ViewerScale)` — `scale` is never `Auto`.
  - `fun resolveLook(layout: ViewerLayout, pages: ViewerPages, scale: ViewerScale, landscape: Boolean, width: WidthClass, height: HeightClass): ViewLook`
  - `fun rowsFor(pageCount: Int, twoPages: Boolean): List<IntRange>` (pairs 0..1, 2..3, odd last alone — same in both layouts)
  - `fun rowOf(page: Int, rows: List<IntRange>): Int`
  - `fun rowSize(aspects: List<Float>, pointWidths: List<Float>, area: Size, gap: Float, scale: ViewerScale, dpi: Float): Size` — pixel size of a row of pages sharing one height: FitPage = largest that fits both sides; FitWidth = row width equals area width; FitHeight = row height equals area height; Percent = widths from points/72·dpi·p/100 at a shared height (use the first page's height, others' widths follow aspect), plus gaps.
  - `fun pageBoxes(aspects: List<Float>, rowSize: Size, gap: Float): List<Rect>` — boxes inside the row, left to right, from (0,0).

- [ ] **Step 1: Write the failing tests:**

```kotlin
@Test fun auto_matches_today() {
    assertEquals(ViewLook(false, true, ViewerScale.FitPage), resolveLook(Auto, Auto, Auto, true, EXPANDED, REGULAR))
    assertEquals(ViewLook(false, false, ViewerScale.FitPage), resolveLook(Auto, Auto, Auto, false, MEDIUM, REGULAR))
    assertEquals(ViewLook(true, false, ViewerScale.FitWidth), resolveLook(Auto, Auto, Auto, true, MEDIUM, COMPACT))
}
@Test fun chosen_values_win_over_auto() =
    assertEquals(ViewLook(true, true, ViewerScale.FitHeight), resolveLook(Scroll, Two, FitHeight, false, COMPACT, REGULAR))
@Test fun rows_pair_pages_odd_last_alone_in_scroll() = assertEquals(listOf(0..1, 2..3, 4..4), rowsFor(5, twoPages = true))
@Test fun fit_page_fits_both_sides() // A4 (1.414) in 1000×800, one page → height 800, width 565.8
@Test fun fit_width_fills_the_width() // A4 in 1000×800 → 1000 × 1414
@Test fun fit_height_fills_the_height() // two A4 in 1000×800, gap 10 → height 800, width 2·565.8+10
@Test fun hundred_percent_is_printed_size() // 595 pt at 320 dpi, 100 % → width 2644.4; 50 % → 1322.2
@Test fun boxes_sit_side_by_side_with_the_gap()
```

- [ ] **Step 2: Run** `./gradlew -q :app:testDebugUnitTest --tests '*PageLayoutTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): resolve the view and lay out rows of pages`

### Task 4: Zoom modifier and the Pages layout

**Files:** Create `ui/viewer/Zoom.kt`; rewrite `ui/viewer/PageArea.kt`; Test: `PageAreaTest.kt` (update to the new signature, keep the existing cases), `DrawingTest.kt` (one new case).

**Interfaces:**
- Consumes: Task 3 functions, `PageSource.pageWidthPoints`.
- Produces:
  - `class PageZoom { var factor: Float (relative to the chosen scale, 1 = chosen); var offset: Offset; fun reset() }` — factor range 0.5..4.
  - `fun Modifier.pinchZoom(zoom: PageZoom, onChange: (centroid: Offset, factorChange: Float, pan: Offset) -> Unit): Modifier` — `awaitEachGesture` on `PointerEventPass.Initial`; acts only once ≥2 pressed pointers of type Touch are down; consumes those changes (so the pager, edge taps and drawing ignore them); ignores Stylus/Eraser pointers.
  - `@Composable fun PageArea(source: PageSource, look: ViewLook, page: Int, onPage: (Int) -> Unit, onToggleBars: () -> Unit, modifier: Modifier = Modifier, zoom: PageZoom = remember { PageZoom() }, overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit = { _, _ -> })` — `page` is the first page on screen; `onPage` reports the first page of the row now on screen.
- Pages layout (`PagedRows`, private): `HorizontalPager(rows.size)`, each page a row at `rowSize × zoom.factor`, centred when smaller than the area, translated by `zoom.offset` (clamped to its own edges) when larger. `userScrollEnabled = row width × factor ≤ area width`. One-finger pan of an overflowing row in the Main pass after children (so drawing still gets single touches before a pen is seen). Edge tap / keys → `animateScrollToPage(±1)`; any row change resets zoom; new row starts at top-left. Double tap → `zoom.reset()`. Render width = shown page width, capped at 2× the fit width, as today.

- [ ] **Step 1: Write the failing tests** (PageAreaTest, 1280×800, `ViewLook(false, true, FitPage)`): keep `edge_tap_turns_by_a_spread`, `middle_tap_toggles_the_bars`, `swipe_left_turns`, `double_tap_returns_to_fit`, `page_down_and_arrows_turn`; add
  - `turn_is_animated` — after an edge tap, advance the clock 50 ms: the pager's `currentPageOffsetFraction != 0`; after 1000 ms page = 2.
  - `pinch_at_the_edge_zooms_without_turning` — `pinch` with both fingers inside x < 15 % → `zoom.factor > 1`, page unchanged.
  - `overflowing_spread_pans_on_swipe_and_turns_on_edge_tap` — FakePages aspect 0.7 (landscape page), `ViewLook(false, false, FitHeight)`: `swipeLeft` → page unchanged, `zoom.offset.x < 0`; edge tap right → page 1.
  - DrawingTest `pinch_over_drawings_zooms_in_drawing_mode` — drawing session on, pinch over the page → factor > 1, no stroke added.
- [ ] **Step 2: Run** `./gradlew -q :app:testDebugUnitTest --tests '*PageAreaTest' --tests '*DrawingTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.** `ViewerScreen` call site is updated in Task 6; until then pass `resolveLook(Auto, Auto, Auto, …)` so the app compiles.
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): animated page turns and pinch zoom anywhere`

### Task 5: The Scroll layout

**Files:** Modify `ui/viewer/PageArea.kt`; Test: `test/.../ui/viewer/ScrollAreaTest.kt` (Robolectric, 1280×800).

**Interfaces:** `ScrolledRows` (private), selected when `look.scroll`. A `LazyColumn` of rows (`rowsFor`), spaced by `PAGE_GAP`, each `rowSize × zoom.factor` and centred; inside a `horizontalScroll` with the column's width = max(area width, widest row) when rows are wider than the area. Pinch (via `pinchZoom`) changes `zoom.factor` and keeps the point under the centroid in place (adjust both scroll states). Edge tap / keys → `animateScrollToItem(row ± 1)`. `onPage` gets the first page of `firstVisibleItemIndex`. Double tap → factor 1. The vertical scroll replaces today's `FIT_WIDTH` mode.

- [ ] **Step 1: Write the failing tests:** `rows_stack_under_each_other` (two rows' overlays reported with increasing top in root coordinates), `edge_tap_scrolls_to_the_next_row` (page 0 → 1, animated: mid-way offset between), `edge_tap_reaches_the_last_row` (5 pages, two per row: three taps → page 4, a fourth stays), `pinch_zooms_the_rows` (row width grows), `fling_reports_the_first_visible_page`.
- [ ] **Step 2: Run** `--tests '*ScrollAreaTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `--tests '*viewer*'` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): scroll pages one under another`

### Task 6: View menu and wiring

**Files:** Create `ui/viewer/ViewMenu.kt`, `res/drawable/ic_book_open.xml`; Modify `ViewerScreen.kt` (signature `ViewerScreen(viewer, onAction, openPages = null, layout: LayoutPrefs = default)` replacing `stylusSeen`; `layout.stylusSeen` used where `stylusSeen` was), `nav/NotaeNavDisplay.kt:346` (pass `snapshot.layout`), strings; Test: `ViewerScreenTest.kt`.

**Interfaces:**
- Consumes: `Action.SetViewerLook`, `LayoutPrefs.viewerLayout/viewerPages/viewerScale`, `resolveLook`.
- Produces: `@Composable fun ViewMenu(look: ViewLook, livePercent: Int?, expanded: Boolean, onDismiss: () -> Unit, onPick: (ViewerLayout, ViewerPages, ViewerScale) -> Unit)` — a `DropdownMenu` styled like `ShareSetMenu` (card colour, 20 dp corners, divider border), three labelled groups, the current choice marked with `ic_check`; the scale group shows "Fit pages" when two pages; picking an entry dismisses the menu. `livePercent` (shown as "137 %", unmarked presets) when the zoom factor ≠ 1. Picking dispatches `SetViewerLook` with the resolved values of the other two groups (so the menu never writes `auto` back except untouched groups keep their stored value).
- The View button (`ic_book_open`, "View") sits in the top bar before Draw; the page counter stays.

- [ ] **Step 1: Write the failing tests:** `view_menu_marks_the_current_look`, `picking_scroll_dispatches_the_look` (`SetViewerLook(Scroll, <stored pages>, <stored scale>)`), `switching_layout_keeps_the_page` (on page 3, layout prefs change Pages→Scroll → counter still "4 / n"), `pinched_view_shows_live_percent`.
- [ ] **Step 2: Run** `--tests '*ViewerScreenTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `--tests '*viewer*'` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): choose layout, pages on screen and scale`

### Task 7: Notes side panel

**Files:** Modify `ViewerScreen.kt` (extract `NotesContent(viewer, onEdit)` from `NotesSheet`; add `NotesPanel`), strings (Close notes); Test: `ViewerScreenTest.kt`.

**Interfaces:** At `windowWidthClass() == EXPANDED` the page area sits in a `Row` with `AnimatedVisibility(notes, enter = slideInHorizontally { it } + fadeIn(), exit = reverse)` holding a 360 dp `NotesPanel` (card background, title "Notes" + close button, `NotesContent`). The notes button toggles it (`selected = notes`); Back closes it (existing back handler). Otherwise the bottom sheet as today. Editing a note opens `NoteEditor` as today in both cases.

- [ ] **Step 1: Write the failing tests:** at `w1280dp-h800dp`: `notes_open_as_a_side_panel` (panel node shown, pages still shown, no sheet), `notes_button_closes_the_panel`; at `w600dp-h800dp` (MEDIUM width): `notes_open_as_a_sheet_when_narrower`; `panel_becomes_sheet_when_narrower` (open at 1280×800, switch `RuntimeEnvironment.setQualifiers("w600dp-h800dp")`, recreate → sheet shown, not panel).
- [ ] **Step 2: Run** `--tests '*ViewerScreenTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): notes beside the score on a tablet`

### Task 8: Screenshots, docs, gates and device test

**Files:** Modify `ui/LayoutMatrixScreenshotTest.kt` (states `viewer-scroll`, `viewer-two-scroll`, `viewer-notes-panel` at the four sizes), `README.md` (viewer feature lines), `CHANGELOG.md` (Unreleased → Added/Fixed), `docs/superpowers/plans/...` checklist below.

- [ ] **Step 1:** Add the states; `./gradlew :app:recordRoborazziDebug`; look at every new and changed PNG (bars not clipped, rows centred, panel 360 dp, menu not needed in matrix).
- [ ] **Step 2:** `./gradlew :app:verifyRoborazziDebug` — PASS; `scripts/check.sh full` — PASS.
- [ ] **Step 3:** Install the debug APK on the emulator/tablet and run the incremental checklist: tablet landscape, tablet portrait, phone — pinch at edges and centre, in and out of drawing mode; swipe and edge-tap animation; Scroll fling and edge tap; each scale; 1↔2 pages; notes panel open/close/rotate; prefs survive app restart; pen draws while a palm rests.
- [ ] **Step 4: Commit** `test(viewer): screenshot the new layouts and update README and changelog`

---

## Self-review notes

- Spec coverage: prefs/auto → 1, 3; percent printed size → 2, 3; Pages + animation + overflow → 4; pinch anywhere → 4 (`pinchZoom`), 5; Scroll → 5; menu + live percent → 6; notes panel → 7; tests/screenshots → 8.
- Ruling in the plan: `rowsFor` pairs the same way in both layouts; picking a menu entry dismisses the menu; `PageMode`/`pageModeFor`/`spreadsFor` are removed in favour of `ViewLook`/`rowsFor`.
