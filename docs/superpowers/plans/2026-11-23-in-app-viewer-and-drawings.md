# In-app Viewer and Drawings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scores open in Notae's own `PdfRenderer` viewer (with Previous/Next inside a set and a setting to keep the system app), and inside a set the S Pen marks up a score on an overlay stored per set item, carried by backups and shared sets.

**Architecture:** The Rust core owns the viewer's state (which score, which set item, its neighbours, the hint) and the overlays (one JSON file per set item, in backups and shared sets); Kotlin renders pages with `PdfRenderer`, handles gestures and pen input, and keeps the drawing session (tool, undo/redo) in memory, sending the finished overlay to the core. The viewer sits above whatever screen opened it, as the last back-stack entry.

**Tech Stack:** Rust (`notae-app-core`, serde, UniFFI), Kotlin/Jetpack Compose, Navigation 3, `android.graphics.pdf.PdfRenderer`, Robolectric + Roborazzi.

**Spec:** `docs/superpowers/specs/2026-11-22-in-app-viewer-and-drawings-design.md` (ADR 0003)

## Global Constraints

- No new dependencies (Rust or Gradle). `unsafe` stays forbidden; no unchecked `unwrap`/`expect` and no `!!` in production code.
- The library PDF is never written. Overlays live in Notae's private storage: `<data dir>/overlays/<set id>/<item id>.json`, written atomically (temp file + rename, as `favorites::write_atomically`).
- All overlay coordinates and widths are fractions (0–1) of the page's width and height.
- *System default app* behaves exactly as 0.7.1: `Effect::OpenPdf`, `pdf_opened`, the launch error.
- `open_scores_in` defaults to *Notae*, also for 0.7.x prefs files without the line.
- Hint copy: "Scores now open in Notae. Change it in Settings." (Polish: "Nuty otwierają się teraz w Notae. Możesz to zmienić w Ustawieniach.") — only on the first viewer open after updating from before 0.8.0, never on a new install.
- Content-changed copy: "This score's PDF changed; drawings may not line up." (pl: "PDF tego utworu się zmienił; rysunki mogą nie pasować.")
- Two pages side by side only when the window is landscape, width class EXPANDED and height class REGULAR; pages pair 1–2, 3–4, …; height class COMPACT fits to width and scrolls vertically; otherwise one page fit to the screen.
- Pinch zoom 1×–4×; double-tap returns to fit; the outer fifth of either edge turns (by a spread in two-page mode); the middle third toggles the bars; arrows and Page Up/Down turn; Escape is Back.
- Pen: black and red; highlighter: yellow, translucent; eraser removes whole strokes it touches.
- The pen draws and a finger moves the page once a stylus has been seen (`stylus_seen` in UI prefs); before that a single finger draws in drawing mode and two fingers pan/zoom.
- Backup schema 3 carries overlays; schema 1 and 2 files still import. A `.notae-set` without drawings keeps version 2 (readable by 0.7.x); with drawings it is version 3. Score notes are never shared.
- Imports reuse the set-note pairing rule: per piece the file's drawings win; local drawings stay where the file has none; "keep both" gives the file's drawings to the copy; a file without drawings keeps local drawings.
- Removing a piece asks first only when it has drawings.
- Strings in `values/strings.xml` and `values-pl/strings.xml`; Conventional Commits; quality gates via `scripts/check.sh` (`fast` while working, `full` at the end).

## Review Focus

1. **The PDF can't be opened in the viewer** (moved, permission lost, damaged): the viewer shows the existing PDF error and Back works; Recent is not corrupted. → Task 8 `unreadable_pdf_shows_the_error_with_back`.
2. **A set item removed or the set deleted while its overlay file exists, then a new item gets a reused id**: no old drawings appear on the new piece. → Task 3 `removing_an_item_deletes_its_overlay` and `deleting_a_set_deletes_its_overlays`.
3. **Previous/Next across a missing piece** (shared set piece not in the library): the viewer skips it like a text note instead of opening an unreadable file. → Task 2 `next_skips_notes_and_missing_pieces`.
4. **A resting palm in drawing mode on a tablet that has seen the pen**: a finger touch never adds a stroke. → Task 11 `finger_never_draws_once_a_stylus_was_seen`.
5. **Rotation or one/two-page switch with drawings**: strokes stay on the same spot of the page. → Task 11 `overlay_points_map_to_the_page_box` (fraction → pixel mapping at two page boxes).

---

## File Structure

Core (`crates/notae-app-core/src/`):
- `ui_prefs.rs` — modify: `OpenScoresIn`, `open_scores_in`, `viewer_hint_shown`, `stylus_seen`.
- `viewer.rs` — create: `ViewerState` and neighbour lookup in a set.
- `overlays.rs` — create: overlay records (serde + UniFFI), `OverlayStore` (load/save/remove per item, remove per set).
- `ui.rs`, `api.rs`, `api/action.rs`, `api/snapshot.rs` — modify: actions, back order, snapshot fields.
- `backup.rs`, `share.rs` — modify: schema 3, overlays in files, pairing rule.

Kotlin (`apps/notae-android/app/src/main/kotlin/com/c2yz/notae/`):
- `nav/BackStack.kt`, `nav/NotaeNavDisplay.kt` — modify: `Route.Viewer`.
- `platform/PageRenderer.kt` — create: `PageSource` interface and its `PdfRenderer` implementation.
- `ui/viewer/ViewerScreen.kt` — create: bars, notes sheet, hint, warning, error.
- `ui/viewer/PageArea.kt` — create: spreads, zoom/pan, turning.
- `ui/viewer/Drawing.kt` — create: `DrawSession` (pure Kotlin), overlay canvas, input routing, tool bar.
- `ui/settings/SettingsScreen.kt` — modify: **Open scores in** row.
- `ui/sets/…` share sheet — modify: **Include drawings**.

---

### Task 1: Viewer preferences in the core

**Files:**
- Modify: `crates/notae-app-core/src/ui_prefs.rs`, `api/action.rs`, `api.rs`, `api/snapshot.rs`, `ui.rs`
- Test: `ui_prefs.rs` tests module, `crates/notae-app-core/tests/` (follow the existing api test file for actions)

**Interfaces:**
- Produces: `#[derive(uniffi::Enum)] pub enum OpenScoresIn { Notae, SystemApp }` (prefs keys `notae` / `system`); `UiPrefs { open_scores_in: OpenScoresIn, viewer_hint_shown: bool, stylus_seen: bool, .. }`; `UiPrefs::load(path)` returns `viewer_hint_shown: true` when the file is missing (new install) and `false` when the file exists without the line (an update); `Action::SetOpenScoresIn { choice: OpenScoresIn }`, `Action::StylusSeen`; `LayoutPrefs { open_scores_in: OpenScoresIn, stylus_seen: bool, .. }` in the snapshot.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn open_scores_in_defaults_to_notae_for_old_prefs() {
    let prefs = UiPrefs::from_text("library_view=list\nsection=all\n");
    assert_eq!(prefs.open_scores_in, OpenScoresIn::Notae);
}
#[test] fn prefs_round_trip_viewer_fields() {
    let prefs = UiPrefs { open_scores_in: OpenScoresIn::SystemApp, viewer_hint_shown: true, stylus_seen: true, ..UiPrefs::default() };
    assert_eq!(UiPrefs::from_text(&prefs.to_text()), prefs);
}
#[test] fn hint_is_due_after_an_update_not_on_a_new_install() {
    let dir = tempfile_dir(); // the module's existing temp-dir helper
    assert!(UiPrefs::load(&dir.join("missing")).viewer_hint_shown);
    std::fs::write(dir.join("old"), "section=all\n").unwrap();
    assert!(!UiPrefs::load(&dir.join("old")).viewer_hint_shown);
}
```

Plus an api test `set_open_scores_in_is_saved_and_shown`: dispatch `SetOpenScoresIn { SystemApp }` → `snapshot().layout.open_scores_in == SystemApp` and the prefs file contains `open_scores_in=system`.

- [ ] **Step 2: Run** `cargo test -p notae-app-core prefs` — Expected: FAIL (no `OpenScoresIn`).
- [ ] **Step 3: Implement** the enum, the three fields with `key=value` lines `open_scores_in`, `viewer_hint_shown`, `stylus_seen`, the load rule, the two actions (saved like `SetLibraryView`), and the snapshot fields.
- [ ] **Step 4: Run** `cargo test -p notae-app-core` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): remember where scores open and whether a pen was seen`

### Task 2: The viewer in the core

**Files:**
- Create: `crates/notae-app-core/src/viewer.rs`
- Modify: `ui.rs`, `api.rs`, `api/action.rs`, `api/snapshot.rs`, `lib.rs`
- Test: `viewer.rs` tests, api tests

**Ruling recorded in the plan:** the spec names `Screen::Viewer`; the core keeps the viewer as an overlay field (`UiModel.viewer: Option<ViewerState>`) closed first by `back()`, so the screen, section, selection and search below stay as they were and Back returns to the row it came from. The snapshot exposes it as `viewer: Option<Viewer>`; `Snapshot.screen` is unchanged. The page to open at is always the first page (no page memory is specified beyond that).

**Interfaces:**
- Consumes: `OpenScoresIn`, `viewer_hint_shown` (Task 1).
- Produces:
  - `pub struct ViewerState { note_key: String, relative_path: String, set: Option<(String /*set id*/, String /*item id*/)> }`.
  - `pub fn neighbours(set: &Set, item_id: &str, present: impl Fn(&PieceRef) -> bool) -> (Option<String>, Option<String>)` — previous/next **piece** item ids, skipping text notes and pieces `present` rejects (not in the library).
  - Snapshot record `Viewer { uri: String, relative_path: String, note_key: String, title: String, composer: Option<String>, score_note: Option<String>, set_note: Option<String>, set_id: Option<String>, item_id: Option<String>, has_previous: bool, has_next: bool, show_hint: bool }`.
  - Actions `ViewerPrevious`, `ViewerNext`, `ViewerHintSeen`.
  - `Action::OpenPdf` with `open_scores_in == Notae`: marks the set item opened, records Recent (`record_opened`), sets `viewer`, returns no effect. With `SystemApp`: today's code path, unchanged.

- [ ] **Step 1: Write the failing tests** (api tests, using the existing test library fixture with a set holding piece, note, piece, missing piece, piece):

```rust
#[test] fn open_pdf_opens_the_viewer_and_counts_as_opened() // snapshot.viewer is Some with the note_key; recent list starts with it; no Effect returned
#[test] fn system_app_setting_keeps_the_0_7_effect()        // SetOpenScoresIn(SystemApp) then OpenPdf → vec![Effect::OpenPdf{..}], viewer None
#[test] fn back_closes_the_viewer_first()                   // open details, OpenPdf, back() → viewer None, selection unchanged
#[test] fn next_skips_notes_and_missing_pieces()            // from item 1, ViewerNext → item 3 (note 2 skipped), ViewerNext → item 5 (missing 4 skipped); has_next false at the end
#[test] fn previous_and_next_do_not_grow_the_back_stack()   // after two ViewerNext, one back() → viewer None
#[test] fn hint_shows_once_after_an_update()                // prefs file without viewer_hint_shown: first viewer show_hint true; ViewerHintSeen → false and saved; new install → false
#[test] fn notes_show_score_note_then_set_note()            // viewer.score_note and viewer.set_note from the item
```

- [ ] **Step 2: Run** `cargo test -p notae-app-core viewer` — Expected: FAIL.
- [ ] **Step 3: Implement.** `back()` closes the viewer before the add-to-set sheet check (it is drawn over everything). `ViewerNext/Previous` replace `viewer` with the neighbour item (marking it opened and recording Recent). The viewer also closes when the library closes or its set is deleted.
- [ ] **Step 4: Run** `cargo test -p notae-app-core` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): open scores in Notae's viewer, with Previous and Next in a set`

### Task 3: Overlay store in the core

**Files:**
- Create: `crates/notae-app-core/src/overlays.rs`
- Modify: `ui.rs` (removal hooks), `api.rs` (`data_dir.join("overlays")`), `api/action.rs`, `api/snapshot.rs`
- Test: `overlays.rs` tests, api tests

**Interfaces:**
- Produces (serde + `uniffi::Record`/`Enum`, snake_case JSON):
  - `enum Tool { Pen, Highlighter }`; `struct Stroke { tool: Tool, color: u32 /*ARGB*/, width: f32, points: Vec<f32> /*x0,y0,x1,y1… fractions*/ }`; `struct TextBox { x: f32, y: f32, text: String, size: f32 }`; `struct PageOverlay { page: u32, strokes: Vec<Stroke>, texts: Vec<TextBox> }`; `struct Overlay { content_key: String, pages: Vec<PageOverlay> }` (file: `{"schema_version":1,"content_key":…,"pages":[…]}`).
  - `OverlayStore::new(dir: PathBuf)`, `get(&self, set_id, item_id) -> Option<Overlay>`, `put(&self, set_id, item_id, overlay: &Overlay) -> io::Result<()>` (an overlay with no strokes and no texts deletes the file), `remove_item(set_id, item_id)`, `remove_set(set_id)`, `has(set_id, item_id) -> bool`, `all_for(set_id) -> Vec<(String, Overlay)>`.
  - `Action::SaveOverlay { set_id: String, item_id: String, overlay: Overlay }`.
  - Snapshot `Viewer` gains `overlay: Option<Overlay>` and `content_changed: bool` (overlay's `content_key` ≠ the item's current key).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn overlay_round_trips_through_its_file()
#[test] fn an_empty_overlay_deletes_the_file()
#[test] fn removing_an_item_deletes_its_overlay()     // api: RemoveSetItem on an item without drawings → overlay file gone (with drawings: see Task 4)
#[test] fn deleting_a_set_deletes_its_overlays()      // ConfirmDeleteSet → overlays/<set id>/ gone
#[test] fn a_changed_pdf_keeps_the_overlay_and_warns() // overlay content_key "aa…", item key "bb…" → viewer.overlay Some, content_changed true
#[test] fn a_damaged_overlay_file_reads_as_none()
```

- [ ] **Step 2: Run** `cargo test -p notae-app-core overlay` — Expected: FAIL.
- [ ] **Step 3: Implement**; `SaveOverlay` writes via `put`; failures log with `eprintln!` like the other stores.
- [ ] **Step 4: Run** `cargo test -p notae-app-core` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): store drawings per set item`

### Task 4: Ask before removing a piece with drawings

**Files:** Modify `ui.rs`, `api.rs`, `api/action.rs`, `api/snapshot.rs`; Kotlin `ui/sets/` dialogs (reuse the set-delete confirmation dialog's component); strings.
**Test:** api tests; Kotlin `SetsScreenTest`.

**Interfaces:**
- Produces: `RemoveSetItem` on an item with an overlay sets `remove_item_confirmation: Option<RemoveItemConfirmation { item_id: String, title: String }>` instead of removing; `Action::ConfirmRemoveSetItem`, `Action::CancelRemoveSetItem`; `back()` cancels it. Unchecking a piece in Add to set whose occurrences have drawings asks the same way (one confirmation per uncheck). Copy: title "Remove “%1$s”?", body "Its drawings in this set are deleted too.", buttons "Cancel" / "Remove" (pl: "Usunąć „%1$s”?", "Jego rysunki w tym zestawie też zostaną usunięte.", "Anuluj" / "Usuń").

- [ ] **Step 1: Write the failing tests:** `removing_a_piece_with_drawings_asks_first` (overlay present → item still there, confirmation Some; confirm → item and overlay gone), `removing_a_piece_without_drawings_is_immediate`, `back_cancels_the_removal`; Kotlin `remove_confirmation_shows_and_confirms`.
- [ ] **Step 2: Run** `cargo test -p notae-app-core remov` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p notae-app-core` and the Kotlin test — Expected: PASS.
- [ ] **Step 5: Commit** `feat(sets): ask before removing a piece that has drawings`

### Task 5: Backups carry drawings (schema 3)

**Files:** Modify `backup.rs`, `share.rs` (extract the pairing), `ui.rs` (`apply_import` call); Test: `backup.rs` tests, api tests.

**Interfaces:**
- Produces: `pub fn pair_items(set: &Set, local: &Set) -> Vec<(String /*item id in set*/, String /*local item id*/)>` in `share.rs` — the matching `keep_set_notes` uses (key, else path, repeats in order); `keep_set_notes` is rewritten on top of it. Export JSON `"overlays":[{"set_id","item_id","overlay":{…}}]`, `schema_version: 3`; `parse_import` accepts 1, 2 and 3; `Backup.overlays: Vec<(String, String, Overlay)>`.
- Import rule, per replaced set: for each `(new, local)` pair, the file's overlay for `new` wins; else the local overlay of `local` moves to `new`; local overlays of unpaired local items are removed with their items. New sets take the file's overlays.

- [ ] **Step 1: Write the failing tests:** `export_includes_overlays_as_schema_3`, `schema_2_backup_still_imports_and_keeps_local_drawings`, `imported_drawings_win_and_local_ones_fill_gaps`, `keep_set_notes_behaves_as_before` (the existing tests keep passing after the refactor).
- [ ] **Step 2: Run** `cargo test -p notae-app-core backup` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p notae-app-core` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(backup): carry drawings in exports (schema 3)`

### Task 6: Share a set with its drawings

**Files:** Modify `share.rs`, `api/action.rs` (`ShareSet { set_id, with_notes, with_drawings: bool }`), `api.rs`, `ui.rs` (`import_shared_set`), snapshot (`SetCard`/share sheet gets `has_drawings: bool`); Kotlin share sheet; strings "Include drawings" / "Dołącz rysunki".
**Test:** `share.rs` tests, api tests, Kotlin share sheet test.

**Interfaces:**
- `share_json(set, rows, with_notes, overlays: Option<&[(String, Overlay)]>)` — `None` writes version 2 exactly as today; `Some` writes version 3 with `"overlays"`. `parse_shared` accepts 2 and 3. Importing: replace → pairing rule of Task 5; keep both → the file's overlays go to the copy's set id.
- Kotlin: the share sheet shows an **Include drawings** switch row only when the set has drawings (default off); the two share choices pass it.

- [ ] **Step 1: Write the failing tests:** `share_without_drawings_stays_version_2` (byte-equal to today's output for the fixture), `share_with_drawings_is_version_3_and_round_trips`, `keep_both_gives_the_drawings_to_the_copy`, `score_notes_are_never_shared`; Kotlin `include_drawings_shows_only_when_the_set_has_drawings`.
- [ ] **Step 2: Run** `cargo test -p notae-app-core share` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p notae-app-core` and the Kotlin test — Expected: PASS.
- [ ] **Step 5: Commit** `feat(share): include drawings when sharing a set`

### Task 7: The viewer route, skeleton screen and the setting

**Files:** Modify `nav/BackStack.kt`, `nav/NotaeNavDisplay.kt`, `ui/settings/SettingsScreen.kt`; Create `ui/viewer/ViewerScreen.kt`; strings.
**Test:** `nav/BackStackTest.kt`, `ui/viewer/ViewerScreenTest.kt`, `ui/settings/SettingsScreenTest.kt`.

**Interfaces:**
- Consumes: `Snapshot.viewer: Viewer?` (Tasks 2–3), `LayoutPrefs.openScoresIn` (Task 1).
- Produces: `Route.Viewer(val noteKey: String, val itemId: String?)` appended last by `backStackFor` when `snapshot.viewer != null`; `@Composable fun ViewerScreen(viewer: Viewer, onAction: (Action) -> Unit, pages: PageSource?)` with a top bar (Back, title, page "3 / 5"), the body left to Task 9. Back uses the existing slide (`ScreenMotion` forward/back). Settings row **Open scores in** (pl **Otwieraj nuty w**) with choices *Notae* / *System default app* (pl *Domyślna aplikacja systemowa*) as the Language row's radio list.

- [ ] **Step 1: Write the failing tests:** `viewer_sits_above_the_screen_it_opened_from` (BackStackTest: details + viewer → `[Library, Details, Viewer]`; search + viewer → `[…, Search, Viewer]`), `viewer_back_button_dispatches_back`, `open_scores_in_row_dispatches_the_choice`.
- [ ] **Step 2: Run** `./gradlew -q :app:testDebugUnitTest --tests '*BackStackTest' --tests '*ViewerScreenTest' --tests '*SettingsScreenTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): open scores on a viewer screen and add the Open scores in setting`

### Task 8: Page rendering

**Files:** Create `platform/PageRenderer.kt`; Test: `ui/viewer/FakePages.kt` (test source) and `ui/viewer/ViewerScreenTest.kt`.

**Interfaces:**
- Produces: `interface PageSource : Closeable { val pageCount: Int; fun pageAspect(index: Int): Float /*height ÷ width*/; suspend fun render(index: Int, widthPx: Int): ImageBitmap }`; `class PdfPageSource private constructor(...) : PageSource` with `companion suspend fun open(resolver: ContentResolver, document: Uri): PdfPageSource` (via `openFileDescriptor(document, "r")`, as `SafLibrary.withRenderer`). One page open at a time: a `Mutex` serialises `openPage`; renders on `Dispatchers.IO`; an LRU of 6 bitmaps keyed `(index, widthPx)`. `PdfOpener.find` resolves the URI; failures map through `pdfOpenFailure`.
- `ViewerScreen` opens the source in a `LaunchedEffect(viewer.noteKey)`, closes it on leave, and on failure shows the existing PDF error text (`PdfError` composable) with Back.

- [ ] **Step 1: Write the failing test** `unreadable_pdf_shows_the_error_with_back` (a `PageSource` factory that throws `FileNotFoundException` → the "file not found" PDF error text is shown, Back dispatches `back`), and `pages_render_from_the_source` (FakePages with 3 white pages → page indicator "1 / 3").
- [ ] **Step 2: Run** `./gradlew -q :app:testDebugUnitTest --tests '*ViewerScreenTest'` — Expected: FAIL.
- [ ] **Step 3: Implement** (inject the factory as a parameter defaulting to `PdfPageSource::open`).
- [ ] **Step 4: Run** the same — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): render score pages with PdfRenderer`

### Task 9: Page area: spreads, zoom and turning

**Files:** Create `ui/viewer/PageArea.kt`; Modify `ViewerScreen.kt`, `MainActivity.kt` (keep screen on); Test: `ui/viewer/PageAreaTest.kt`, `ui/viewer/SpreadsTest.kt`.

**Interfaces:**
- Produces: `enum class PageMode { ONE, TWO, FIT_WIDTH }`; `fun pageModeFor(landscape: Boolean, width: WidthClass, height: HeightClass): PageMode`; `fun spreadsFor(pageCount: Int, mode: PageMode): List<IntRange>` (TWO → `0..1, 2..3, …`, odd last alone; else one page each); `@Composable fun PageArea(source: PageSource, mode: PageMode, spread: Int, onSpread: (Int) -> Unit, onToggleBars: () -> Unit, overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit = { _, _ -> })`. Zoom state (scale 1–4, offset) resets on spread change and on double-tap. `FIT_WIDTH` is a vertical scroll of full-width pages, its "spread" the topmost visible page. Keep-screen-on: `DisposableEffect` in `ViewerScreen` adds/clears `WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON` on the activity window.

- [ ] **Step 1: Write the failing tests:**

```kotlin
@Test fun two_pages_only_on_a_landscape_tablet() {
    assertEquals(PageMode.TWO, pageModeFor(true, WidthClass.EXPANDED, HeightClass.REGULAR))
    assertEquals(PageMode.ONE, pageModeFor(false, WidthClass.MEDIUM, HeightClass.REGULAR))
    assertEquals(PageMode.FIT_WIDTH, pageModeFor(true, WidthClass.MEDIUM, HeightClass.COMPACT))
}
@Test fun spreads_pair_one_two_three_four() = assertEquals(listOf(0..1, 2..3, 4..4), spreadsFor(5, PageMode.TWO))
```

PageAreaTest (FakePages, 1280×800): `edge_tap_turns_by_a_spread` (tap at x = 95 % → spread 1; at 5 % → back to 0), `middle_tap_toggles_the_bars`, `swipe_left_turns`, `double_tap_returns_to_fit` (pinch via `performTouchInput { pinch(...) }` then double-tap → scale 1), `page_down_and_arrows_turn`, `screen_stays_on_while_the_viewer_shows` (window flags contain KEEP_SCREEN_ON; cleared after the viewer leaves).

- [ ] **Step 2: Run** `./gradlew -q :app:testDebugUnitTest --tests '*PageAreaTest' --tests '*SpreadsTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): turn, zoom and pair pages`

### Task 10: Set bar, notes sheet, hint and warning

**Files:** Modify `ViewerScreen.kt`; strings ("Previous piece"/"Next piece", pl "Poprzedni utwór"/"Następny utwór"; "Notes"/"Notatki"; the hint and warning copy from Global Constraints). Test: `ViewerScreenTest.kt`.

**Interfaces:** Previous/Next piece buttons only with `set_id`, enabled by `has_previous`/`has_next`, dispatch `ViewerPrevious`/`ViewerNext`; the notes button opens a sheet with the score note then the set note, editable with the existing `NoteEditor` and `SaveScoreNote`/`SaveSetNote` actions; the hint is a dismissible card that dispatches `ViewerHintSeen`; `content_changed` shows the warning as a banner line under the bar.

- [ ] **Step 1: Write the failing tests:** `set_bar_moves_between_pieces`, `no_set_bar_outside_a_set`, `notes_sheet_lists_score_note_then_set_note`, `hint_shows_and_is_dismissed`, `changed_pdf_shows_the_warning`.
- [ ] **Step 2: Run** `--tests '*ViewerScreenTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `feat(viewer): move through a set, show notes, the hint and the changed-PDF warning`

### Task 11: Drawing session model

**Files:** Create `ui/viewer/Drawing.kt` (model part); Test: `ui/viewer/DrawSessionTest.kt` (plain JUnit, no Robolectric).

**Interfaces:**
- Produces: `enum class DrawTool { PEN_BLACK, PEN_RED, HIGHLIGHTER, ERASER, TEXT }`; `class DrawSession(initial: Overlay, contentKey: String)` with `tool`, `fun begin(page: Int, x: Float, y: Float)`, `fun extend(x: Float, y: Float)`, `fun end()`, `fun addText(page, x, y, text)`, `fun moveText(page, index, x, y)`, `fun deleteText(page, index)`, `fun clearPage(page)`, `fun undo()`, `fun redo()`, `val canUndo/canRedo`, `fun overlay(): Overlay`. Eraser removes every stroke whose polyline comes within its radius (0.01 of the page width) of the eraser path. Pen width 0.003, highlighter 0.02 with colour `0x66FFEB3B`, pens `0xFF000000` and `0xFFD32F2F`. Text size default 0.025. `fun Overlay.toPixels(page: Int, box: Rect): List<Offset>`-style mapping helper `fun pagePoint(x: Float, y: Float, box: Rect): Offset` and its inverse.
- `fun routeTouch(toolType: Int, stylusSeen: Boolean, pointers: Int, drawing: Boolean): TouchRoute` with `enum class TouchRoute { DRAW, ERASE, MOVE_PAGE }` — stylus → DRAW (eraser tool type → ERASE); finger → MOVE_PAGE once `stylusSeen`, else DRAW for one pointer in drawing mode and MOVE_PAGE for two.

- [ ] **Step 1: Write the failing tests:** `pen_stroke_is_recorded_in_fractions`, `eraser_removes_whole_strokes_it_touches`, `undo_and_redo_walk_the_session`, `clear_page_is_undoable`, `text_box_can_be_moved_and_deleted`, `finger_never_draws_once_a_stylus_was_seen`, `finger_draws_before_a_stylus_was_seen`, `overlay_points_map_to_the_page_box` (point (0.5, 0.25) → centre-x, quarter-height in two different boxes, and back).
- [ ] **Step 2: Run** `--tests '*DrawSessionTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `feat(drawings): drawing session with tools, eraser, text and undo`

### Task 12: Drawing in the viewer

**Files:** Modify `ui/viewer/Drawing.kt` (UI part), `ViewerScreen.kt`, `PageArea.kt` (overlay slot input); strings for the tool bar (Draw, Pen, Red pen, Highlighter, Eraser, Text, Undo, Redo, Clear page, Done, Show drawings/Hide drawings; clear confirmation "Clear this page's drawings?" / "Wyczyścić rysunki na tej stronie?").
**Test:** `ui/viewer/DrawingTest.kt`.

**Interfaces:**
- Consumes: `DrawSession`, `routeTouch`, `pagePoint` (Task 11), `PageArea.overlay` slot (Task 9), `Viewer.overlay`, `Action.SaveOverlay`, `Action.StylusSeen`, `LayoutPrefs.stylusSeen`.
- The **Draw** toggle shows only with `set_id`; drawing mode shows the tool bar (bottom on phones, top bar row on tablets); outside it the overlay draws read-only with a show/hide toggle. Input is read with `pointerInteropFilter`/`PointerEvent.type` + `PointerType.Stylus`/`Eraser`; the first stylus event dispatches `StylusSeen` once. Leaving drawing mode, Previous/Next, and leaving the viewer dispatch `SaveOverlay` when the session changed.

- [ ] **Step 1: Write the failing tests:** `draw_toggle_only_in_a_set`, `stylus_stroke_is_saved_on_done` (inject stylus events with `performTouchInput` + `MotionEvent` tool type via `pointerInput` test helpers or `InputDeviceCompat`; assert `SaveOverlay` with one stroke), `finger_moves_the_page_after_a_stylus` , `clear_page_asks_first`, `hidden_drawings_are_not_drawn` (screenshot-free: overlay node absent).
- [ ] **Step 2: Run** `--tests '*DrawingTest'` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `feat(drawings): mark up scores in a set with the pen`

### Task 13: Screenshots, README and changelog

**Files:** Modify `ui/LayoutMatrixScreenshotTest.kt` (states `viewer`, `viewer-set-drawing`, `viewer-notes` at 360×640, 640×360, 1280×800, 800×1280), `README.md` (replace the "hands every PDF to another viewer" and "No in-app PDF viewing or annotation" lines), `CHANGELOG.md` (Unreleased → Added: viewer, drawings, setting, Include drawings).

- [ ] **Step 1:** Add the states (FakePages with three pages, an overlay with a pen stroke and a highlight).
- [ ] **Step 2:** `./gradlew :app:recordRoborazziDebug`; look at every new PNG: bars not clipped, one page vs two pages per window class, tool bar fits 360 dp.
- [ ] **Step 3:** `./gradlew :app:verifyRoborazziDebug` — PASS; `scripts/check.sh full` — PASS.
- [ ] **Step 4: Commit** `test(viewer): screenshot the viewer and update README and changelog`

---

## Self-review notes

- Spec coverage: viewer core → 2; setting/hint → 1, 2, 7, 10; PdfRenderer/cache → 8; layout/gestures/keyboard/keep-on → 9; notes/prev-next → 10; drawings input → 11–12; storage/removal/warning → 3, 4; backup → 5; share → 6; README → 13. Device checks (palm, rotation alignment, file hash unchanged) stay on the 0.8.0 RC checklist.
- Ruling in the plan: viewer as a core overlay field instead of a `Screen` variant (Task 2), first page always.
