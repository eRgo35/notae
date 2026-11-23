# Viewer layout, zoom and notes panel

- **Date:** 2026-11-23
- **Follows:** the in-app viewer (`2026-11-22-in-app-viewer-and-drawings-design.md`), shipped in v0.8.0.
- **Next, separately:** importing a score folder as a `.zip` through Share, with a "From a link" helper that opens
  the browser and explains what to tap. Notae itself stays without the `INTERNET` permission. Not part of this design.

## Intent

The maintainer tried 0.8.0 on a tablet: everything works, but the viewer does not behave like the PDF readers
people know. Pinch zoom only works in the middle of the screen, a page turn has no motion so it is unclear what
happened, there is no continuous "page under page" scrolling, notes cover the score instead of sitting beside it
on a large screen, and the layout (one or two pages, scale) cannot be chosen. The gestures that work today (edge
tap, swipe, double tap, arrow keys and pedal) must keep working.

## Goals

1. Pinch zoom works anywhere on the page area, in every layout, with or without drawing mode.
2. A page turn is animated: the page follows the finger and settles; an edge tap, arrow key or pedal turns with the
   same slide.
3. Two layouts: **Pages** (turned one spread at a time, as today) and **Scroll** (pages, or pairs, one under the
   other, scrolled continuously).
4. **One or two pages** on screen, chosen by the user.
5. **Scale:** fit page(s), fit width, fit height, or a percentage (50, 75, 100, 125, 150, 200 %).
6. On an EXPANDED-width window the notes open as a right-hand side panel; the pages narrow to make room.
7. Layout, pages on screen and scale are remembered for the whole device.

## Non-goals

- Importing scores from a link or zip (next design).
- Per-score view settings.
- Changes to drawing tools or the overlay format.

## Anti-goals

- Losing a turn the user asked for: an edge tap, key or pedal always moves exactly one spread (Pages) or to the next
  page's top (Scroll).
- A pinch that turns the page, or a swipe meant to turn that pans instead, at fit scale.
- The pen not drawing because a gesture was taken by zoom (pen input keeps priority in drawing mode).

## Decisions (maintainer)

- Pages and Scroll both exist, switched in a view menu (option B), next to 1/2 pages and scale.
- All three fit modes (page, width, height) plus percentages.
- Remembered device-wide.
- Build on Compose's own pieces: `HorizontalPager` for Pages, `LazyColumn` for Scroll, one shared zoom layer
  (approach 1 below).

## Approaches

1. **Compose pager and lazy column with a shared zoom layer (picked).** Mature fling, snapping and animation;
   reuses the `LazyColumn` the viewer already has. Cost: zoom and page swipe must be separated carefully.
2. **One custom "camera" canvas** with all pages laid out and a single pan/zoom transform; a turn animates the
   camera. Uniform zoom, but fling, snapping and lazy rendering would all be written by hand. Rejected: much more
   code for the same result.

## Design

**Prefs (core).** `UiPrefs` gains three keys, exposed in the snapshot's layout and set by new actions:
- `viewer_layout = auto | pages | scroll`
- `viewer_pages = auto | one | two`
- `viewer_scale = auto | fit_page | fit_width | fit_height | <percent>`

`auto` keeps today's behaviour until the user picks something: two pages in a landscape EXPANDED×REGULAR window,
else one; Scroll at fit width on a COMPACT height, else Pages at fit page. An unknown value falls back to `auto`,
as other prefs do. Pinch zoom is not stored.

**View menu.** A "View" button in the viewer's top bar opens a menu with three groups: Layout (Pages / Scroll),
On screen (1 page / 2 pages), Scale (Fit page(s) / Fit width / Fit height / 50–200 %). The current choice is
marked. Pinching moves the scale off the preset; the menu then shows the live percentage as selected text with no
preset marked, and double tap returns to the chosen preset.

**Percent.** 100 % is the page's printed size on this screen: the PDF page width in points / 72 × the display's
dpi. `PageSource` gains the page's width in points.

**Pages layout.** A `HorizontalPager` of spreads (pairs in two-page mode, odd last page alone). The spread is laid
out at the chosen scale; when it is larger than the area it pans (and starts at its top-left after a turn). The
pager's own swipe is enabled only when the spread does not overflow horizontally; otherwise an edge tap or key
still turns. Edge tap, keys and pedal call `animateScrollToPage`.

**Scroll layout.** A `LazyColumn` of rows (one page, or a pair) at the chosen scale, centred, scrolling
horizontally too when wider than the area. Edge tap, keys and pedal animate to the next/previous row's top. The
page counter shows the first visible page.

**Zoom.** One transform handler on the page area that sees pointer events before the pages and the drawing overlay
(Initial pass), takes over only with two or more fingers, and in drawing mode never takes a pen pointer. Zoom range
is the fit scale ÷ 2 … 4× printed size (builder may tune). In Pages, zoom scales the spread; in Scroll, it changes
the row width (relayout, keeping the point under the fingers in place). Rendering uses the shown width, as today.

**Notes panel.** At EXPANDED width the notes button toggles a 360 dp panel on the right of the page area with the
same content as today's sheet (score note, set note, tap to edit). Narrower windows keep the bottom sheet. Whether
the panel is open is not remembered.

**Unchanged.** Middle-third tap toggles the bars; drawings are drawn per page in every layout; Back/Escape as today.

## Calls left to the builder

- Menu component and icon, in the One UI style the app uses.
- Animation spec (pager default spring is fine).
- Exact zoom limits and the 2× render cap from today.
- Tests: core prefs round-trip; spread/row layout and scale math as plain functions; Robolectric tests for edge
  tap/key turns in both layouts, the menu, and the panel at EXPANDED width; refreshed screenshot matrix.
