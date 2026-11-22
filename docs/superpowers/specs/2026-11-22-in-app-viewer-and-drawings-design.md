# In-app viewer and drawings on scores in a set (0.8.0)

- **Date:** 2026-11-22
- **Issues:** #46 (viewer), #48 (drawings); decided in ADR 0003
- **Milestone:** v0.8.0, built after the fast scan and the layout fixes, on their window classes
  (`2026-11-22-adaptive-layout-fixes-design.md`)

## Intent

Singing through a set on the tablet should never leave Notae. A score opens in Notae's own viewer; inside a
set, Previous/Next move to the neighbouring piece; with the S Pen the user marks up a score for that set, and
the marks come back the next time the piece opens from that set. The library PDF is never changed.

## Goals

1. **Viewer (#46)**, everything in the issue's scope and acceptance list: `PdfRenderer`, swipe and edge taps to
   turn, pinch zoom, double-tap to fit, page number, screen stays on, Previous/Next in a set, Back with the
   existing slide, Settings **Open scores in** → *Notae* (default) / *System default app*, one-time hint after
   updating, notes panel with the score note then the set note, editable.
2. **Drawings (#48)**, everything in the issue's rules and acceptance list: pen, highlighter, eraser, text;
   overlay per occurrence of a score in a set; show/hide; undo/redo; clear page with confirmation; export and
   import; "Include drawings" when sharing a set; a warning when the PDF changed.

## Non-goals

- Drawings outside a set (a score-wide layer is a later issue).
- Pressure-sensitive strokes, colour pickers, shapes, handwriting recognition.
- Writing anything into the PDF, or exporting a marked-up PDF.
- A PDF library other than `PdfRenderer` (only if it falls short, and then as a separate question).

## Anti-goals

- A resting palm leaving marks.
- Losing drawings: a drawing survives app restarts, set reordering, and a PDF content change (with a warning).
- *System default app* behaving any differently from 0.7.1.

## Decisions (maintainer)

- **Tablet landscape shows two pages side by side** (like an open book); everywhere else one page.
- **The pen draws and a finger moves the page.** While marking up, a finger pans, zooms and turns pages; only
  the S Pen draws. A device where no pen has been seen yet (a phone) lets a finger draw in drawing mode.
- Drawings are part of 0.8.0.
- **Imports reuse the set-note matching rule.** Per piece, the file's drawings win; local drawings stay where
  the file has none; "keep both" gives the file's drawings to the copy; importing a 0.7.x backup (no drawings)
  keeps local drawings.
- **Removing a piece asks first only when it has drawings.** Pieces without drawings are removed as today.

## Decisions (mine, stated to the maintainer, not objected to)

- Pages pair 1–2, 3–4, … (not the book convention of page 1 alone), since scores start on the right.
- Tapping the middle third of the page shows or hides the viewer's bars.
- A phone in landscape fits the page to width and scrolls vertically.

## Approaches considered

| Option | Verdict | Why |
| --- | --- | --- |
| **Viewer as a new core `Screen`, pages rendered in Kotlin with `PdfRenderer`, overlays stored by the Rust core** | **Chosen** | Fits ADR 0002 (Rust owns screen state and data, Kotlin renders and talks to Android); back stack, slide and set context come for free; overlays share the sets/backup/share code. |
| A separate Android `Activity` for the viewer | Rejected | Breaks the single back stack and slide; set context and notes would need a second state owner. |
| Overlays stored in Kotlin (Room or files) | Rejected | Export, import and sharing live in the core; a second store would split them. |
| Overlays inside `sets.json` | Rejected | Strokes would bloat the file that is rewritten on every set edit. |

## Design: viewer

### Core

- `Screen::Viewer` with: the score's content key and library path, the page to open at, and an optional set
  context (set id, item id, previous and next piece items, when they exist). Opening a score from Library,
  Recent, Favorites, Search or a set pushes it when the setting is *Notae*; Back pops it and returns to the
  screen and row it came from (the existing slide transition).
- The existing `Action::OpenPdf` path decides: setting *Notae* → push `Screen::Viewer`; *System default app* →
  today's `Effect::OpenPdf`, unchanged.
- Opening in the viewer counts as opening the score (Recent, "just opened", `last_opened_item` in a set), as
  `Effect::OpenPdf` success does today.
- `Action::ViewerPrevious` / `Action::ViewerNext` replace the viewer's score with the neighbouring piece of the
  set (text notes in the set are skipped), without growing the back stack.
- UI prefs gain `open_scores_in: Notae | SystemApp` (default `Notae`, also for 0.7.x prefs files that lack the
  field) and `viewer_hint_shown: bool`. The hint shows on the first viewer open after updating from a version
  before 0.8.0, never on a new install: "Scores now open in Notae. Change it in Settings."
- Settings gets the **Open scores in** row with the two choices.

### Kotlin

- `ui/viewer/ViewerScreen.kt`: the page area, the bars (title, page "3 / 5", notes, drawing toggle in a set,
  Previous/Next piece in a set), the notes sheet and the hint.
- `platform/PageRenderer.kt`: opens the document (`PdfOpener.find` → `ParcelFileDescriptor`) and renders pages
  to bitmaps off the main thread, at the displayed size times the zoom, with a small LRU cache (current, next
  and previous spread). `PdfRenderer` allows one open page at a time, so access is serialised.
- **Layout:** two pages when the window is landscape with width class EXPANDED and height class REGULAR (the
  layout fixes' `windowClass()`); one page fit to the screen otherwise; height class COMPACT fits to width with
  vertical scroll. Pinch zoom 1×–4×, pan while zoomed, double-tap returns to fit. Swipe or tap the outer fifth
  of either edge to turn (by a spread in two-page mode). Keyboard: arrows and Page Up/Down turn, Escape is Back.
- The screen stays on while the viewer is shown (`FLAG_KEEP_SCREEN_ON`, cleared when it leaves).
- **Notes sheet:** the score note, then the set note when opened from a set, editable in place with the
  existing note actions.
- An unreadable or missing PDF shows the existing PDF error in the viewer, with Back.

## Design: drawings

### Input

- A **Draw** toggle in the viewer's bar, only when opened from a set. In drawing mode a tool bar shows: pen
  (black, red), highlighter (yellow, translucent), eraser (removes whole strokes it touches), text, undo, redo,
  clear page, done. Outside drawing mode the overlay is shown read-only, with a show/hide toggle.
- Stylus events (`MotionEvent.TOOL_TYPE_STYLUS`, and the eraser end as eraser) draw. Finger events always pan,
  zoom and turn pages, once a stylus has been seen on this device (remembered in UI prefs as `stylus_seen`).
  Before that, a single finger draws in drawing mode and two fingers pan and zoom.
- Text: tap with the text tool places a box; it is edited with the keyboard and can be moved or deleted.

### Data and storage (core)

- An overlay belongs to one `(set id, item id)` and records the score's content key it was drawn on.
- Per page: strokes `{tool, colour, width, points}` and text boxes `{x, y, text, size}`. All coordinates and
  widths are fractions of the page's width and height (0–1), so they stay in place at any zoom, rotation,
  one- or two-page mode and screen size.
- Stored in Notae's private storage, one JSON file per overlay (`overlays/<set id>/<item id>.json`), written
  atomically when drawing mode ends or the viewer leaves. Removing a set item or a set removes its overlays.
- Undo/redo are in-memory for the drawing session. Clear page asks for confirmation and is itself undoable
  within the session.
- If the score's current content key differs from the overlay's, the viewer shows: "This score's PDF changed;
  drawings may not line up." The overlay is kept and still editable.

### Export, import and sharing

- Export sets (backup schema 3) includes overlays; Import restores them; schema 2 files still import.
- Share set gains **Include drawings** (the `.notae-set` format's version is bumped; files without drawings
  stay readable by 0.7.x where they were before). Score notes are still never shared.

## Testing

- Rust: viewer screen push/pop and set Previous/Next (skipping text notes), the setting's default and
  migration, the hint rule (update vs new install), overlay store round trip, removal with the item, content-key
  warning, backup schema 3 round trip and schema 2 import, share with and without drawings.
- Kotlin: the viewer chrome at the layout matrix's phone, phone landscape and tablet sizes (Roborazzi), one vs
  two pages per window class, gesture handling (edge tap, swipe, double-tap), stylus vs finger routing, the
  setting in Settings.
- On the tablet: a set walked through with Previous/Next, S Pen drawing with a resting palm, overlay alignment
  after rotation, library file hash unchanged after annotating (RC checklist).
- `README.md`: the lines about handing every PDF to another viewer and "No in-app PDF viewing or annotation" are
  updated when the viewer ships.

## Left to the builder

- Names of actions, records and files beyond those above; bitmap cache size; exact bar layout within the
  layout fixes' rules; stroke smoothing; default text size; how the tool bar sits in phone layouts.
