# Adaptive layout fixes (0.8.0)

- **Date:** 2026-11-22
- **Milestone:** v0.8.0 (with the fast scan, the in-app viewer #46 and drawings #48)
- **Audit:** 35 screen states × 12 window sizes, plus Polish (5 sizes) and font scale 1.3 (4 sizes), rendered
  through `NotaeNavDisplay` with Robolectric/Roborazzi on 2026-11-22. Findings below; captures were kept in the
  session scratchpad, not in the repo.

## Intent

Every element sits where it belongs on phones and tablets, portrait and landscape, at any size: no
toolbars floating mid-screen, no buttons wrapping letter by letter, no main action pushed off-screen, no
screen where no score title is visible. The maintainer saw "toolbars or elements in the wrong place, weird
heights or wrapping". The layout should be predictable from size to size.

## Goals

1. Chrome is chosen from the window's **width and height**, so short windows (a phone on its side) get a
   layout made for them.
2. Headers never collapse: the title keeps its room, and actions that do not fit move into ⋮.
3. Text in buttons, titles and subtitles never wraps mid-word. One line, ellipsis where needed.
4. The main action of every screen and dialog is visible without scrolling at every size in the matrix.
5. Grids always show at least one full row of cards with their titles.
6. Screenshot tests cover the whole size matrix, so this does not regress.

## Non-goals

- New features or a visual redesign. One UI look stays (blue accent, rounded cards, large titles where there
  is room).
- Dark-theme-specific fixes (none found; the matrix runs light, plus dark at two sizes as a smoke check).
- The viewer's own layout (its design covers it).

## Anti-goals

- A layout that jumps between very different chrome at neighbouring sizes (today 832×384 gets the rail and
  915×412 the full side panel).
- Hiding actions with no way to reach them. Anything moved into ⋮ stays reachable and labelled.
- Fixing one size by breaking the tablet at 1280×800 and 800×1280, which are fine today in English.

## Decisions (maintainer)

- **Short windows (height under 480 dp) get a slim layout (option A):** a narrow icon-only rail, a one-line
  header with a smaller title, extra header actions in ⋮, and cards sized so at least one row with titles
  fits. Rejected: the portrait bottom bar turned sideways (costs another 80 dp of height).
- Audit every screen at the size matrix below.

## Approaches considered

| Option | Verdict | Why |
| --- | --- | --- |
| **Extend `ui/components/WindowWidth.kt` into a window class with a height class** | **Chosen** | No new dependency; one place decides; the existing width rule stays for tall windows. |
| `androidx.compose.material3.adaptive` window size classes | Rejected | New dependency for two thresholds the app already computes. |
| Patch each screen at the sizes that broke | Rejected | Leaves the root cause (width-only rule) and regresses at the next size. |

## Root causes and fixes

### R1. Width-only chrome (`WindowWidth.kt`, used at `LibraryScreen.kt:229`)

- Add a height class: `COMPACT` under 480 dp, otherwise `REGULAR`. Expose a single `windowClass()` returning
  width and height classes (or a derived `Chrome` enum: `BOTTOM_BAR`, `SLIM_RAIL`, `RAIL`, `SIDE_PANEL`).
- Chrome rule:
  - width COMPACT (under 600) → bottom bar (phone portrait, unchanged);
  - height COMPACT, width 600 or more → **slim rail** (icons only, no labels, ~72 dp wide, settings in the
    rail's end, fits 360 dp of height), no side panel at any width;
  - otherwise width MEDIUM → rail (unchanged); EXPANDED → side panel (unchanged).
- The rail must fit 360 dp: today 4 × 68 dp items + padding + settings ≈ 364 dp (`PhoneChrome.kt:507–523`).
- **No library-beside-details in short windows.** *(Builder's call, recorded.)* The two-pane scene
  (`NotaeNavDisplay.kt:96`, `TWO_PANE_MIN_WIDTH`) also requires height class REGULAR; at 915×412 the details
  open full-screen like a phone. The list-detail baselines at that size change accordingly.
- **The filter action opens the existing composer sheet** *(builder's call)* that the bottom-bar layout
  already uses (`PhoneChrome.kt`), so no new UI is invented and composers stay reachable in the slim layout.

### R2. Header row collapse (`LibraryScreen.kt:355–430`, `PillButton.kt:75`)

- The header measures its actions; those that do not fit after the title's minimum width (builder sets it,
  around 160 dp) move into a ⋮ overflow, in a fixed priority order (builder's call, recorded in code: Back and
  search stay; scan, share, view toggle, Reorder go first).
- Title and subtitle: `maxLines = 1`, ellipsis.
- In the slim layout: title at a smaller size (builder picks from the type scale), no subtitle line if it
  does not fit, chips row hidden behind a filter action when height is COMPACT.
- **Scope of the slim header** *(builder's call)*: the measuring/overflow header is the library's (the only
  header with many actions). Settings, Search, Details and Licenses keep their own headers but get one-line
  titles with ellipsis and the smaller title size when height is COMPACT, through a shared title-style helper,
  not a shared header component.
- `PillButton`: label `maxLines = 1`, `softWrap = false`, ellipsis; pills in a row get weights or wrap to the
  next line as a whole (Settings About card, `SettingsScreen.kt:209–212`).

### R3. Grid cards sized by width only (`ScoreItems.kt:106`, `SetsPane.kt:97`)

- Column count from width **and** available height: card height (page preview + title) must not exceed the
  grid's visible height minus room for the title line. Pick more columns when the card would be too tall.
- Fixes: no titles at 640×360, 832×384, 700×500; two huge columns at 600×960 and 840×600.

### R4. Main actions off-screen in short windows

- **Details** (`DetailsScreen.kt:406`): preview size from width **and** height in the stacked layout; title,
  Open score and Edit details visible at 640×360 and 700×500.
- **Metadata editor** (`MetadataEditor.kt:89–120`): Cancel and Save pinned outside the scroll area, as the set
  dialog already does (rule R47).
- **Add picker** (`AddToSet.kt:189`): list height may shrink below 144 dp; footer and Done keep their size.
- **Welcome / Reconnect** (`WelcomeScreen.kt:68–74`): in short windows, intro and buttons side by side, or the
  body text visible with a scroll affordance.
- **Add to set** at 640×360: at least two rows visible.

### R5. Smaller fixes

- Reorder rows at 360 dp: titles get the room (controls tighter or drag handle only), not cut at ~9 chars.
- Polish labels: bottom bar and rail labels, set card "Edytowano…", search scope chips scroll with an edge
  fade.
- List-detail at 840×600: chips row scrolls; Open score not on the pane's bottom edge.
- Licenses at 360 dp: horizontal scroll for the monospace text instead of ragged hard wraps.
- Search results at 1440×900: content width capped like Settings.

## Size matrix (screenshot tests)

- Phone portrait: 360×640, 384×832, 412×915
- Phone landscape: 640×360, 832×384, 915×412
- Foldable / small tablet: 600×960, 700×500, 840×600
- Tablet: 800×1280, 1280×800, 1440×900
- Polish at 360×640, 832×384, 840×600, 1280×800, 800×1280; font scale 1.3 at 384×832 and 1280×800.

A parameterised Roborazzi test renders the key states (library grid/list, inside a set, sets, details from
library and set, search, settings, metadata editor, add picker, welcome) across the matrix and records
baselines. Unit tests assert the chrome rule per size and that header actions overflow rather than shrink the
title. Existing a11y checks (labels, 48 dp targets) run at every size.

*(Builder's calls, recorded.)* The matrix test sits **alongside** the existing per-screen screenshot tests
(their parity-named captures stay) and skips size/state pairs those tests already cover, keeping new PNGs to
roughly 100. Existing baselines at 1280×800 and 800×1280 **may change** (for example a one-line
`PillButton`); they are re-recorded and the diffs are reviewed by the maintainer in the PR, not required to be
pixel-identical.

## Left to the builder

- Exact thresholds inside the bands (title minimum width, card maximum height), the overflow priority order,
  and the slim title size.
- Whether `Chrome` is an enum or a pair of classes.
- Which states beyond the list above get matrix screenshots.
- System insets: the audit did not simulate system bars or the keyboard; check on a device (phone landscape is
  shorter still).
