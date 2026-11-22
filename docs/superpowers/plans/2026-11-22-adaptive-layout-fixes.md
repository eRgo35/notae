# Adaptive Layout Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every Notae screen keeps its toolbars, titles and main actions in place, on one line where they belong, at every phone and tablet size in portrait and landscape.

**Architecture:** One window class (width + height) in `ui/components/WindowWidth.kt` decides the chrome: bottom bar, a new slim rail for short windows, the rail, or the side panel. The library header moves actions that do not fit into ⋮ instead of squeezing the title. Grids, details, dialogs and the welcome screen size themselves from height as well as width. A matrix screenshot test pins all of it.

**Tech Stack:** Kotlin, Jetpack Compose (Material 3), Navigation 3, Robolectric + Compose UI test, Roborazzi.

**Spec:** `docs/superpowers/specs/2026-11-22-adaptive-layout-fixes-design.md`

## Global Constraints

- No new dependencies; specifically not `androidx.compose.material3.adaptive`.
- Height class: `COMPACT` under **480 dp**, otherwise `REGULAR`. Width thresholds stay `COMPACT_WIDTH = 600.dp`, `EXPANDED_WIDTH = 840.dp` (`ui/components/NotaeDialog.kt:86,89`).
- Chrome rule: width < 600 → bottom bar; height COMPACT and width ≥ 600 → slim rail, never the side panel; else width < 840 → rail; else side panel.
- The slim rail fits **360 dp** of height.
- No library-beside-details when height is COMPACT (`NotaeNavDisplay.kt:96`).
- Button labels, titles and subtitles: one line, ellipsis, never wrap mid-word.
- Nothing moved into ⋮ or behind a filter button becomes unreachable or unlabelled; the existing a11y checks (`assertAllClickablesLabelled`, `assertClickTargetsAtLeast48dp`) pass at every size.
- No visual redesign: One UI look (blue accent, rounded cards, large titles where there is room). Tablet 1280×800 and 800×1280 must not get worse; their baselines may change and are re-recorded for review in the PR.
- Conventional Commits; `scripts/check.sh fast` while working, `full` before merging. Production Kotlin has no unchecked `!!`.
- Size matrix (dp): phone portrait 360×640, 384×832, 412×915; phone landscape 640×360, 832×384, 915×412; foldable/small tablet 600×960, 700×500, 840×600; tablet 800×1280, 1280×800, 1440×900. Polish at 360×640, 832×384, 840×600, 1280×800, 800×1280; font scale 1.3 at 384×832 and 1280×800.

Test commands below run from `apps/notae-android` with `ANDROID_HOME=~/Android/Sdk` set:
- unit/UI: `./gradlew :app:testDebugUnitTest --tests '<FQN>'`
- record screenshots: `./gradlew :app:recordRoborazziDebug --tests '<FQN>'`
- verify screenshots: `./gradlew :app:verifyRoborazziDebug`

## Review Focus

1. **Rotating a phone with a sheet or dialog open** (composer sheet, metadata editor, add picker): the overlay stays open and usable in the new chrome. → Task 3 adds `composer_sheet_survives_rotation_to_landscape`.
2. **Polish strings in the slim header and rail** (longest labels, e.g. "Wszystkie utwory"): the title still keeps its minimum width and the rail item is labelled for TalkBack even though the rail shows icons only. → Task 3 and Task 5 run their header/rail tests with `pl` too.
3. **A set with set notes and a reorder hint at 915×412**: three header text lines must not push the content off-screen. → Task 5 adds `set_header_with_notes_fits_a_short_window`.
4. **Font scale 1.3 on the slim rail at 640×360**: icons stay inside 360 dp and Settings is reachable. → Task 3 adds `slim_rail_fits_360dp_at_font_scale_1_3`.
5. **Soft keyboard over the metadata editor in phone landscape**: Save stays visible above the IME. → Task 9 uses `imePadding()` and asserts buttons are outside the scroll container.

---

## File Structure

- `ui/components/WindowWidth.kt` — modify: `HeightClass`, `Chrome`, `chromeFor()`, `windowChrome()`. Single source of the chrome rule.
- `ui/components/PillButton.kt` — modify: one-line label.
- `ui/components/ScreenTitle.kt` — create: shared title style for Settings, Search, Details, Licenses (`screenTitleSize()`).
- `ui/library/PhoneChrome.kt` — modify: `SlimLibrary`, `NavRail(slim = true)`, make `ComposerSheet` internal.
- `ui/library/LibraryScreen.kt` — modify: chrome switch, `Header` overflow.
- `ui/library/HeaderActions.kt` — create: `HeaderAction`, `fitActions()` (pure, unit-testable) and the overflow ⋮.
- `ui/library/ScoreItems.kt`, `ui/sets/SetsPane.kt` — modify: `gridColumns()` from width and height.
- `nav/NotaeNavDisplay.kt` — modify: two-pane needs height REGULAR.
- `ui/details/DetailsScreen.kt`, `ui/details/MetadataEditor.kt`, `ui/sets/AddToSet.kt`, `ui/setup/WelcomeScreen.kt`, `ui/settings/SettingsScreen.kt`, `ui/settings/LicensesScreen.kt`, `ui/search/SearchScreen.kt`, `ui/library/ReorderList.kt`, `ui/sets/SetsPane.kt` — modify: R4/R5 fixes.
- Tests (all under `app/src/test/kotlin/com/c2yz/notae/`): `ui/Windows.kt` (create: the size matrix and `assertInsideWindow()`), `ui/components/WindowClassTest.kt`, `ui/library/SlimLayoutTest.kt`, `ui/library/HeaderActionsTest.kt`, `ui/library/GridColumnsTest.kt`, `ui/ShortWindowTest.kt`, `ui/LayoutMatrixScreenshotTest.kt`; extend `nav/ListDetailSceneTest.kt`, `ui/settings/SettingsScreenTest.kt`.

All main paths are under `apps/notae-android/app/src/main/kotlin/com/c2yz/notae/`.

---

### Task 1: Window class with height and the chrome rule

**Files:**
- Modify: `ui/components/WindowWidth.kt`
- Create test: `ui/components/WindowClassTest.kt`
- Create test helper: `ui/Windows.kt`

**Interfaces:**
- Produces:
  - `enum class HeightClass { COMPACT, REGULAR }`
  - `enum class Chrome { BOTTOM_BAR, SLIM_RAIL, RAIL, SIDE_PANEL }`
  - `fun chromeFor(widthDp: Int, heightDp: Int): Chrome` (pure)
  - `fun heightClassFor(heightDp: Int): HeightClass` (pure); `internal val COMPACT_HEIGHT = 480.dp`
  - `@Composable fun windowChrome(): Chrome` and `@Composable fun windowHeightClass(): HeightClass` (from `LocalConfiguration.screenWidthDp/screenHeightDp`)
  - `windowWidthClass()` stays as is.
  - Test helper `object Windows { val MATRIX: Map<String, String> }` (name like `"w640dp-h360dp"` → qualifiers `"+w640dp-h360dp-xhdpi"`) and `fun SemanticsNodeInteraction.assertInsideWindow(compose: ComposeContentTestRule)` asserting the node's bounds lie within the root's bounds.

- [ ] **Step 1: Write the failing test** `WindowClassTest` (plain JUnit, no Robolectric):

```kotlin
@Test fun chrome_follows_width_and_height() {
    assertEquals(Chrome.BOTTOM_BAR, chromeFor(360, 640))
    assertEquals(Chrome.BOTTOM_BAR, chromeFor(412, 915))
    assertEquals(Chrome.SLIM_RAIL, chromeFor(640, 360))
    assertEquals(Chrome.SLIM_RAIL, chromeFor(832, 384))
    assertEquals(Chrome.SLIM_RAIL, chromeFor(915, 412))   // was the side panel
    assertEquals(Chrome.RAIL, chromeFor(700, 500))
    assertEquals(Chrome.RAIL, chromeFor(600, 960))
    assertEquals(Chrome.RAIL, chromeFor(800, 1280))
    assertEquals(Chrome.SIDE_PANEL, chromeFor(840, 600))
    assertEquals(Chrome.SIDE_PANEL, chromeFor(1280, 800))
}
@Test fun height_class_boundary_is_480() {
    assertEquals(HeightClass.COMPACT, heightClassFor(479))
    assertEquals(HeightClass.REGULAR, heightClassFor(480))
}
```

- [ ] **Step 2: Run** `./gradlew :app:testDebugUnitTest --tests 'com.c2yz.notae.ui.components.WindowClassTest'` — expect FAIL (unresolved `chromeFor`).
- [ ] **Step 3: Implement** the Produces list in `WindowWidth.kt`; update the file's KDoc to state the rule.
- [ ] **Step 4: Run** the same command — expect PASS.
- [ ] **Step 5: Commit** `feat(layout): choose chrome from window width and height`

### Task 2: Two panes only in tall windows

**Files:**
- Modify: `nav/NotaeNavDisplay.kt:96` (and the R37 comment)
- Test: `nav/ListDetailSceneTest.kt`

**Interfaces:**
- Consumes: `COMPACT_HEIGHT` from Task 1.

- [ ] **Step 1: Write the failing test** in `ListDetailSceneTest`:

```kotlin
@Test @Config(qualifiers = "w915dp-h412dp-xhdpi")
fun short_wide_window_opens_details_alone() {
    show(selecting(0))
    compose.onNodeWithTag(DETAILS_BODY_TAG).assertIsDisplayed()
    compose.onNodeWithTag(GRID_TAG).assertDoesNotExist()
}
```

- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.nav.ListDetailSceneTest'` — expect the new test FAIL, others PASS.
- [ ] **Step 3: Implement**: `val twoPane = maxWidth >= TWO_PANE_MIN_WIDTH && maxHeight >= COMPACT_HEIGHT`.
- [ ] **Step 4: Run** — all PASS.
- [ ] **Step 5: Commit** `fix(nav): open details alone in short windows`

### Task 3: Slim rail layout for short windows

**Files:**
- Modify: `ui/library/PhoneChrome.kt` (add `SlimLibrary`; `NavRail` gains `slim: Boolean`; `ComposerSheet` becomes `internal`)
- Modify: `ui/library/LibraryScreen.kt:225-230` (switch on `windowChrome()` instead of `windowWidthClass()`)
- Test: `ui/library/SlimLayoutTest.kt` (Robolectric, `@GraphicsMode(NATIVE)`, `@Config(qualifiers = "w640dp-h360dp-xhdpi")`)

**Interfaces:**
- Consumes: `Chrome`, `windowChrome()` (Task 1).
- Produces: `internal fun SlimLibrary(snapshot: Snapshot, onAction: (Action) -> Unit, header: @Composable () -> Unit, inSet: Boolean, content: @Composable ColumnScope.() -> Unit)`; test tag `const val SLIM_RAIL_TAG = "slimRail"`, `const val FILTER_TAG = "filter"`.
- Slim rail: width 72 dp, `NavItem`s icon-only, 56 dp tall (48 dp target), label kept as content description, Settings at the foot; vertical padding 8 dp, so 4×56 + 3×4 + 16 + 48 = 300 dp ≤ 360.
- Composers: no chips row; the header shows a filter `IconButton` (label `R.string.library_composers`, tag `FILTER_TAG`) when `showsComposers(...)`, opening the existing `ComposerSheet`.

- [ ] **Step 1: Write the failing tests** in `SlimLayoutTest`:

```kotlin
@Test fun short_window_gets_the_slim_rail_not_the_side_panel() {        // runs also at w915dp-h412dp
    show(Fixture.library())
    compose.onNodeWithTag(SLIM_RAIL_TAG).assertIsDisplayed()
    compose.onNodeWithTag(SIDE_PANEL_TAG).assertDoesNotExist()
    compose.onNodeWithTag(CHIPS_TAG).assertDoesNotExist()
}
@Test fun slim_rail_items_are_labelled_and_settings_fits() {
    show(Fixture.library())
    compose.onNodeWithContentDescription("Favorites").assertIsDisplayed()
    compose.onNodeWithContentDescription("Settings").assertInsideWindow(compose)
}
@Test @Config(qualifiers = "w640dp-h360dp-xhdpi", fontScale = 1.3f)
fun slim_rail_fits_360dp_at_font_scale_1_3() { /* same Settings assertion */ }
@Test fun filter_opens_the_composer_sheet() {
    show(Fixture.library())
    compose.onNodeWithTag(FILTER_TAG).performClick()
    compose.onNodeWithText("Bach").performClick()
    assertEquals(Action.SelectSection("composer:bach"), actions.last())
}
@Test fun composer_sheet_survives_rotation_to_landscape() {
    RuntimeEnvironment.setQualifiers("+w384dp-h832dp-xhdpi"); show(Fixture.library())
    /* open the composer sheet via the bottom-bar toolbar */
    RuntimeEnvironment.setQualifiers("+w832dp-h384dp-xhdpi"); compose.waitForIdle()
    compose.onNodeWithText("Bach").assertIsDisplayed()
}
@Test @Config(qualifiers = "pl-w832dp-h384dp-xhdpi")
fun polish_rail_is_labelled() { show(Fixture.library()); compose.onNodeWithContentDescription("Wszystkie utwory").assertIsDisplayed() }
```

(Use the `show()` pattern from `PhoneLayoutTest`, including `assertAllClickablesLabelled` and `assertClickTargetsAtLeast48dp`. Check the fixture's composer key and Polish section label against `Fixture.kt` and `values-pl/strings.xml`.)

- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.ui.library.SlimLayoutTest'` — expect FAIL.
- [ ] **Step 3: Implement** `SlimLibrary` (mirrors `MediumLibrary` with the slim rail and no chips row) and the chrome switch: `BOTTOM_BAR → CompactLibrary`, `SLIM_RAIL → SlimLibrary`, `RAIL → MediumLibrary`, `SIDE_PANEL →` the existing expanded box. The composer sheet state lives in `SlimLibrary`, `rememberSaveable` so it survives rotation; if it is open in `CompactLibrary` at rotation, carry the flag in `LibraryScreen` instead (one `rememberSaveable` shared by both).
- [ ] **Step 4: Run** `SlimLayoutTest` and `PhoneLayoutTest` — expect PASS.
- [ ] **Step 5: Commit** `feat(library): slim rail layout for short windows`

### Task 4: One-line pill buttons

**Files:**
- Modify: `ui/components/PillButton.kt:75-80`, `ui/settings/SettingsScreen.kt:209-212`
- Test: `ui/settings/SettingsScreenTest.kt`

**Interfaces:**
- Produces: `PillButton` label always one line; About card buttons in a `FlowRow` (wrap as whole pills, `Arrangement.spacedBy(8.dp)` both ways).

- [ ] **Step 1: Write the failing test**:

```kotlin
@Test @Config(qualifiers = "pl-w360dp-h640dp-xhdpi")
fun about_buttons_keep_one_line_labels() {
    show(settingsSnapshot())
    val licences = compose.onNodeWithText("Licencje").fetchSemanticsNode()
    assertTrue(licences.layoutInfo.height < 30 * density)           // one line at 16 sp
    compose.onNodeWithText("Licencje").assertInsideWindow(compose)
}
```

(Fetch the exact Polish label from `values-pl/strings.xml`, key `settings_licenses`.)
- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.ui.settings.SettingsScreenTest'` — expect FAIL.
- [ ] **Step 3: Implement**: `Text(..., maxLines = 1, softWrap = false, overflow = TextOverflow.Ellipsis)`; Settings row → `FlowRow`.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `fix(ui): keep pill button labels on one line`

### Task 5: Library header moves actions into ⋮

**Files:**
- Create: `ui/library/HeaderActions.kt`
- Modify: `ui/library/LibraryScreen.kt:355-430` (`Header`)
- Test: `ui/library/HeaderActionsTest.kt` (pure JUnit) and `ui/library/SlimLayoutTest.kt` (add cases)

**Interfaces:**
- Produces:
  - `internal data class HeaderAction(val key: String, val width: Dp, val priority: Int, val pinned: Boolean)`
  - `internal fun fitActions(actions: List<HeaderAction>, available: Dp, overflowWidth: Dp = 48.dp, gap: Dp = 4.dp): Pair<List<String>, List<String>>` → (shown keys in original order, overflowed keys by priority). Pinned actions never overflow; lowest priority overflows first; if anything overflows, room for the ⋮ is reserved.
  - Priority (lowest overflows first): `scan`=0, `share`=1, `view`=2, `reorder`=3, `settings`=4, `newSet`/`add`/`done`=5; pinned: `sidebarToggle`, `setBack`, `search`.
  - `const val HEADER_TITLE_MIN = 160` (dp); tag `const val HEADER_OVERFLOW_TAG = "headerOverflow"`.
  - Overflow menu reuses `MenuItem` from `PhoneChrome.kt:370` in an `ActionSheet`, one item per overflowed action with its existing label and icon.
- Header layout: a `SubcomposeLayout` (or `Layout`) measures the actions at intrinsic width, calls `fitActions(available = width - HEADER_TITLE_MIN)`, then places title column + shown actions + ⋮. Subtitle and set notes: `maxLines = 1`, ellipsis. Slim chrome: title 24 sp (rail/compact keep 30 sp, side panel 40 sp); subtitle dropped when height COMPACT and the header has set notes (notes win).

- [ ] **Step 1: Write the failing tests**:

```kotlin
// HeaderActionsTest
@Test fun everything_fits() = assertEquals(listOf("search","view","scan") to emptyList<String>(),
    fitActions(listOf(a("search",48,9,true), a("view",48,2), a("scan",48,0)), 400.dp))
@Test fun scan_goes_first_and_reserves_overflow() {
    val (shown, over) = fitActions(listOf(a("setBack",48,9,true), a("search",48,9,true), a("view",48,2),
        a("reorder",140,3), a("share",48,1), a("add",110,5), a("scan",48,0)), 360.dp)
    assertEquals(listOf("scan","share"), over)
    assertTrue("setBack" in shown && "search" in shown && "add" in shown)
}
@Test fun pinned_never_overflow() { /* available = 0.dp → shown == pinned keys */ }

// SlimLayoutTest (also at w840dp-h600dp and pl-w1280dp-h800dp)
@Test fun set_header_keeps_its_title() {
    show(Fixture.palmSunday())
    val title = compose.onNodeWithText(/* palm set name */).fetchSemanticsNode().boundsInRoot
    assertTrue(title.width >= HEADER_TITLE_MIN * density)
    compose.onNodeWithTag(GRID_TAG).assertInsideWindow(compose)
}
@Test fun overflowed_actions_stay_reachable() {
    show(Fixture.palmSunday()); compose.onNodeWithTag(HEADER_OVERFLOW_TAG).performClick()
    compose.onNodeWithText("Scan").performClick(); assertEquals(Action.Rescan, actions.last())
}
@Test @Config(qualifiers = "w915dp-h412dp-xhdpi")
fun set_header_with_notes_fits_a_short_window() {
    show(Fixture.palmWithNotes()); compose.onNodeWithTag(GRID_TAG).assertIsDisplayed()
}
```

- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.ui.library.HeaderActionsTest'` and `SlimLayoutTest` — expect FAIL.
- [ ] **Step 3: Implement** `fitActions` and the measured `Header`.
- [ ] **Step 4: Run** both, plus `LibraryScreenTest` and `PhoneLayoutTest` — PASS.
- [ ] **Step 5: Commit** `fix(library): move header actions into a menu instead of squeezing the title`

### Task 6: One-line, height-aware titles on other screens

**Files:**
- Create: `ui/components/ScreenTitle.kt`
- Modify: `ui/settings/SettingsScreen.kt:130`, `ui/settings/LicensesScreen.kt:78`, `ui/search/SearchScreen.kt` (title/field row), `ui/details/DetailsScreen.kt:291,448`
- Test: `ui/ShortWindowTest.kt` (create; Robolectric, per-test qualifiers)

**Interfaces:**
- Produces: `@Composable fun screenTitleSize(narrow: Boolean): TextUnit` → 24 sp when `windowHeightClass() == COMPACT`, else 26 sp if `narrow`, else 32 sp. Callers pass `maxLines = 1, overflow = TextOverflow.Ellipsis` (Details title in the stacked layout may keep 2 lines; side-by-side 2 lines).

- [ ] **Step 1: Write the failing test** `settings_title_is_one_line_in_short_window` at `w640dp-h360dp`: title node height < 1.6 × 24 sp line.
- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.ui.ShortWindowTest'` — FAIL.
- [ ] **Step 3: Implement** and apply at the listed call sites.
- [ ] **Step 4: Run** `ShortWindowTest`, `SettingsScreenTest`, `LicensesScreenTest`, `SearchScreenTest`, `DetailsScreenTest` — PASS.
- [ ] **Step 5: Commit** `fix(ui): one-line screen titles that shrink in short windows`

### Task 7: Grid columns from width and height

**Files:**
- Modify: `ui/library/ScoreItems.kt:101-106`, `ui/sets/SetsPane.kt:93-97`
- Test: `ui/library/GridColumnsTest.kt` (pure JUnit) and `ui/ShortWindowTest.kt`

**Interfaces:**
- Produces: `internal fun gridColumns(inner: Dp, gap: Dp, minCell: Dp, visibleHeight: Dp, aspect: Float, extra: Dp): Int` in `ScoreItems.kt`. Columns from width as today (`floor((inner + gap) / (minCell + gap))`, ≥ 1); then, if a cell of that width would be taller than `visibleHeight` (`cellWidth * aspect + extra > visibleHeight`), increase columns until it fits or the cell would be narrower than 96 dp. Score grid: `aspect = 1.3f, extra = 64.dp`; set grid: `aspect = 0.85f, extra = 88.dp`. `visibleHeight` = the grid box's `maxHeight - inset - 20.dp`.

- [ ] **Step 1: Write the failing tests**:

```kotlin
@Test fun tall_window_keeps_width_columns() = assertEquals(3, gridColumns(600.dp, 16.dp, 176.dp, 900.dp, 1.3f, 64.dp))
@Test fun short_window_adds_columns_until_a_card_fits() {
    val n = gridColumns(496.dp, 16.dp, 176.dp, 240.dp, 1.3f, 64.dp)
    val cell = (496.dp - 16.dp * (n - 1)) / n
    assertTrue(cell * 1.3f + 64.dp <= 240.dp)
}
@Test fun never_narrower_than_96dp() { /* visibleHeight = 80.dp → cell ≥ 96.dp */ }
// ShortWindowTest at w640dp-h360dp, w832dp-h384dp, w700dp-h500dp:
@Test fun a_score_title_is_visible() { show(Fixture.library()); compose.onAllNodesWithText("Agnus Dei")[0].assertInsideWindow(compose) }
@Test fun a_set_name_is_visible() { show(Fixture.sets()); /* first set name inside window */ }
```

- [ ] **Step 2: Run** `GridColumnsTest` and `ShortWindowTest` — FAIL.
- [ ] **Step 3: Implement** `gridColumns` and use it in both grids.
- [ ] **Step 4: Run** both plus `LibraryScreenTest`, `SetsScreenTest` — PASS.
- [ ] **Step 5: Commit** `fix(library): size grid columns so a full row with titles fits`

### Task 8: Details preview sized by height

**Files:**
- Modify: `ui/details/DetailsScreen.kt:400-406`
- Test: `ui/ShortWindowTest.kt`

- [ ] **Step 1: Write the failing test** `details_actions_visible` at `w640dp-h360dp` and `w700dp-h500dp`: details from library; "Open score" and the title are `assertInsideWindow` without scrolling.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement**: in the stacked branch cap the preview by height too: `minOf(..., (cardHeight - 200.dp) / 1.3f)`, floor 96 dp (the current 140 dp floor drops when height is COMPACT). If the card is wide enough for side by side (`cardWidth >= 560.dp && cardWidth > cardHeight` when height is COMPACT), use the `wide` layout.
- [ ] **Step 4: Run** `ShortWindowTest`, `DetailsScreenTest` — PASS.
- [ ] **Step 5: Commit** `fix(details): keep the title and actions in view in short windows`

### Task 9: Metadata editor pins Cancel and Save

**Files:**
- Modify: `ui/details/MetadataEditor.kt:85-120`
- Test: `ui/details/MetadataEditorTest.kt`

- [ ] **Step 1: Write the failing test** `save_stays_visible_in_short_window` at `w640dp-h360dp`: Save `assertInsideWindow`; and Save is not a descendant of the node with `hasScrollAction()`.
- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.ui.details.MetadataEditorTest'` — FAIL.
- [ ] **Step 3: Implement** the R47 pattern from `SetOverlays.kt:112-141`: fields in `Column(Modifier.weight(1f, fill = false).verticalScroll(...))`, `DialogButtons` after it outside the scroll, `imePadding()` on the surface.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `fix(details): pin Cancel and Save in the metadata editor`

### Task 10: Add picker footer keeps its size

**Files:**
- Modify: `ui/sets/AddToSet.kt:189`
- Test: `ui/ShortWindowTest.kt`

- [ ] **Step 1: Write the failing tests** at `w832dp-h384dp` and `w640dp-h360dp`: add picker open (`Fixture` picker state as used in `SetsScreenshotTest`); the "Done" text is displayed and `assertInsideWindow`; at 640×360 "Add to set" shows at least two set rows inside the window.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement**: the list takes the remaining space with `Modifier.weight(1f, fill = false)` and `heightIn(min = 56.dp)` instead of the fixed `maxOf(144.dp, …)`; the footer is measured first.
- [ ] **Step 4: Run** `ShortWindowTest`, `SetsScreenTest` — PASS.
- [ ] **Step 5: Commit** `fix(sets): keep the add picker's Done button in short windows`

### Task 11: Welcome and Reconnect in short windows

**Files:**
- Modify: `ui/setup/WelcomeScreen.kt:62-100`
- Test: `ui/setup/WelcomeScreenTest.kt`

- [ ] **Step 1: Write the failing test** at `w640dp-h360dp` with `SetupScreen.PermissionLost`: the body text (`setup_reconnect_body`) and the primary button are both `assertInsideWindow`.
- [ ] **Step 2: Run** `--tests 'com.c2yz.notae.ui.setup.WelcomeScreenTest'` — FAIL.
- [ ] **Step 3: Implement**: when height is COMPACT, a `Row`: intro (icon 56 dp, title, body, scrollable) on the start half, the feature card and buttons on the end half; otherwise unchanged.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `fix(setup): lay out the welcome side by side in short windows`

### Task 12: Smaller fixes (R5)

**Files:**
- Modify: `ui/library/ReorderList.kt:245-300`, `ui/library/PhoneChrome.kt` (`NavItem` label), `ui/sets/SetsPane.kt:184` (edited line), `ui/search/SearchScreen.kt:108-130`, `ui/settings/LicensesScreen.kt:90-100`, `ui/library/LibraryScreen.kt` (chips row)
- Test: `ui/ShortWindowTest.kt`, `ui/sets/ReorderTest.kt`

**Interfaces:**
- Reorder at width COMPACT: up/down move into `semantics { customActions = listOf(CustomAccessibilityAction(moveUpLabel) {...}, ...) }` on the row; the drag handle and Remove stay visible. Keyboard focus order unchanged at wider widths.
- `NavItem` labels: `maxLines = 1`, ellipsis (already for bottom bar; check rail).
- Search content and results: `Modifier.widthIn(max = 720.dp)` centered, as Settings (`SettingsScreen.kt:142`).
- Search scope chips and the library chips row: `horizontalScroll` with a 24 dp end fade (`drawWithContent` gradient) when content overflows.
- Licenses: monospace text in `horizontalScroll`, `softWrap = false`.

- [ ] **Step 1: Write the failing tests**: `reorder_titles_get_room_at_360dp` (`w360dp-h640dp`: the "Kyrie eleison" row title node is not ellipsized — `TextLayoutResult.hasVisualOverflow == false` via `getTextLayoutResult`); `reorder_moves_via_accessibility_actions_at_360dp` (`performSemanticsAction` custom "Move … up" → `Action.MoveSetItem(id, -1)`); `search_results_are_capped_at_1440dp` (`w1440dp-h900dp`: results list width ≤ 720 dp); `search_scopes_scroll_at_360dp` (`hasScrollAction()` on the chips row).
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement** the Interfaces list.
- [ ] **Step 4: Run** `ShortWindowTest`, `ReorderTest`, `SearchScreenTest`, `LicensesScreenTest` — PASS.
- [ ] **Step 5: Commit** `fix(ui): tidy reorder rows, chips, search width and licenses on small windows`

### Task 13: Layout matrix screenshots and baseline re-record

**Files:**
- Create: `ui/LayoutMatrixScreenshotTest.kt`
- Modify: `app/src/test/screenshots/*` (re-recorded baselines)

**Interfaces:**
- Consumes: `Windows.MATRIX` (Task 1); fixtures from `Fixture.kt`.
- States: `library-grid`, `library-list`, `set-grid`, `sets`, `details-library`, `details-set`, `search-results`, `settings`, `metadata-editor`, `add-picker`, `welcome`. Render through `NotaeNavDisplay` as `nav/ShellFrameTest.kt` does, so chrome and two-pane rules apply. Files: `src/test/screenshots/matrix-<state>-<window>[-pl|-fs13].png`. Skip pairs already captured by existing tests at 1280×800 and 800×1280 (keep this list in a `SKIP` set in the test). Dark theme at `w384dp-h832dp` and `w1280dp-h800dp` for `library-grid` only.
- Each capture also runs `assertAllClickablesLabelled` and `assertClickTargetsAtLeast48dp`.

- [ ] **Step 1: Write** the parameterised test (`@RunWith(ParameterizedRobolectricTestRunner::class)` over `MATRIX × states`, minus `SKIP`).
- [ ] **Step 2: Record** `./gradlew :app:recordRoborazziDebug --tests 'com.c2yz.notae.ui.LayoutMatrixScreenshotTest'`, then re-record all: `./gradlew :app:recordRoborazziDebug`.
- [ ] **Step 3: Look at** every new/changed PNG (`git status app/src/test/screenshots`): no clipped toolbars, no wrapped labels, a title visible in every grid, main action visible. Fix and re-record if not.
- [ ] **Step 4: Verify** `./gradlew :app:verifyRoborazziDebug` — PASS; then from the repo root `scripts/check.sh full` — PASS.
- [ ] **Step 5: Commit** `test(layout): screenshot every key screen across the size matrix` (baselines in the same commit, listing changed tablet baselines in the body for review).

---

## Self-review notes

- Spec coverage: R1 → Tasks 1–3; R2 → 4–6; R3 → 7; R4 → 8–11 (Add to set two rows: Task 10); R5 → 12 (list-detail at 840×600: chips scroll in Task 12; "Open score on the pane's bottom edge" is covered by Task 8's height-aware preview — verify in Task 13 screenshots); matrix → 13.
- Device check for system insets (spec "left to the builder") happens on the tablet/phone during the 0.8.0 RC checklist, not in this plan.
