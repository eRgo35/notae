# Slint parity reference and checklist

This records how the Slint Android app (0.3.0) looks and behaves, so the Compose app (0.5.0) can be checked against it screen by screen. The screenshots are in `slint/`. The checklist below lists every user action reachable in 0.3.0, grouped by screen, with the result to expect. Tick a line when the Compose app does the same.

## How the screenshots were made

- Device: the disposable `notae-test-api35` AVD (API 35 x86_64, 2560×1600 at 320 dpi, so 1280×800 logical in landscape). Portrait was `adb shell wm size 1600x2560` (800×1280 logical, reset afterwards), dark was `adb shell cmd uimode night yes`. Images are raw `adb exec-out screencap -p`, so landscape is 2560×1600 and portrait is 1600×2560.
- App: the Android code of this tree is identical to v0.3.0 (0.4.0 changed scripts, CI and docs only). The emulator builds are `cargo apk2 build -p notae-android --target x86_64-linux-android --profile apk-dev`.
- Library: a non-sensitive fixture of 20 generated one-line PDFs (not the owner's scores), listed exactly under "Fixture library" below. The setup screens do not depend on it.
- Where a PDF thumbnail shows "?" in place of a Polish letter, that is the generated PDF's font, not the app.

### File names

`<screen>-<state>-<portrait|landscape>-<light|dark>.png`. Every state has light and dark in landscape. Only these states also have portrait (light and dark): `details-in-set`, `details-library`, `library-all-grid`, `library-all-list`, `search-results`, `set-inside-grid`, `set-inside-list`, `sets-grid`, `settings`, `setup-error`, `setup-permission-lost`, `setup-unconfigured`. Every other state, including all sheets and dialogs, the other library sections, empty states, the sidebar-collapsed view, the scrolled set views, the banners, `search-empty`, `search-no-results`, `sets-list`, `set-inside-empty`, `settings-export-done` and `pdf-launch-error`, is landscape only. Screens:

| Screen | States captured |
| --- | --- |
| `setup` | `unconfigured`, `permission-lost`, `error` (landscape and portrait) |
| `library` | `all`, `recent`, `favorites` with `grid` or `list`; `all-sidebar-collapsed`; `composer`; `empty-all`, `empty-recent`, `empty-favorites`, `empty-sets` |
| `sheet` | `score-menu`, `set-menu`, `set-item`, `missing-item` (the ⋮ sheets), `add-to-set` |
| `details` | `library`, `in-set` (portrait too) |
| `metadata-editor` | normal, `-error` |
| `search` | `empty`, `results`, `no-results` (results has portrait) |
| `settings` | normal (portrait too), `export-done` |
| `banner` | `running`, `done`, `failed` |
| `sets` | `grid`, `list` (grid has portrait) |
| `set-dialog` | `new`, `edit`, `error`; `set-delete-confirm` |
| `set-inside` | `grid`, `list` (both portrait too), `grid-scrolled`, `list-scrolled`, `empty` |
| `add-picker` | `empty`, `results` |
| `pdf-launch-error` | the red line when no PDF app can open a score |

### Known limits of the reference

- `setup-permission-lost` cannot be reached on a device in 0.3.0 by revoking the folder grant: the app reads the saved folder at launch, and a revoked grant makes that read return nothing, so the Welcome screen shows (`setup-unconfigured`), not "Reconnect your library". The recovery screen needs the second read, inside `reopen`, to fail right after the first one succeeded. The four `setup-permission-lost` images were made with a throwaway build whose `reopen` returned `PermissionLost` when a marker file existed. It runs the real `StorageState::PermissionLost` path and screen; the patch was reverted and is not in the tree.
- `setup-error` is real: a corrupt local `index.sqlite` gives "storage error: cannot open local index: sqlite error: file is not a database" above the Choose button.
- Screenshots of the 20-score fixture library were taken on 2026-10-30; the four `setup-*` states added on 2026-10-31 were taken on the same AVD with the same build setup.

## Fixture library

The generator is gone and the emulator no longer has the library, so this is what the screenshots show, read off them. Task 21 should capture the same set on Compose. The file name is the title plus `.pdf` (the details screen shows `Ave Maria.pdf` for Ave Maria; the other file names are not visible). Each PDF is one page holding the line `<title> lyrics line <N>`; N is given where it is visible. All 20 scores are in All scores, sorted by title, which is the order below.

| # | Title (file name) | Composer | Number | Favorite | `lyrics line` |
| --- | --- | --- | --- | --- | --- |
| 1 | Agnus Dei | none | none | no | 7 |
| 2 | Alleluja | none | none | no | 5 |
| 3 | Ave Maria | Schubert (edited in the app; the title is an edit too) | none | yes | 0 |
| 4 | Ave verum corpus | none | none | no | 18 |
| 5 | Bardzo długi tytuł utworu liturgicznego na wejście procesji z towarzyszeniem chóru i organów w uroczy… (the long title; the full text is cut off in every screenshot, so use a title of this length that starts and runs the same) | none | none | no | not visible |
| 6 | Barka | none | none | no | 8 |
| 7 | Bogurodzica | none | none | no | 1 |
| 8 | Ciebie Boga wysławiamy | none | none | no | not visible |
| 9 | Gloria in excelsis | none | none | no | 4 |
| 10 | Jezu ufam Tobie | none | none | no | 12 |
| 11 | Kyrie eleison | none | none | no | 3 |
| 12 | Lacrimosa dies illa | none | none | no | 15 |
| 13 to 20 | Magnificat, Sanctus and Te Deum laudamus, plus five more titles | see below | none | Magnificat and Te Deum laudamus: yes | Magnificat 13, Sanctus 6 |

- Rows 13 to 20 are not fully visible: no screenshot scrolls the All list past row 12 (`library-all-sidebar-collapsed-landscape-*` shows rows 1 to 12). Magnificat and Sanctus are seen only inside the first set and in Favorites, Te Deum laudamus only in Favorites. `search-results-portrait-*` shows four more with their lines: Panis angelicus 16, Salve Regina 17, Ubi caritas 19 and Zdrowaś Maryjo 11. The last title never appears on screen, so any title will do. Every count in the captures is 20 scores.
- Composers (side panel and chips): Bach 1, Palestrina 1, Schubert 1. Schubert is Ave Maria. Bach and Palestrina belong to two of rows 13 to 20 (not to any of rows 1 to 12, whose cards show no composer line) and which two is not visible. Captures taken before about 2:50 on 2026-10-30 (for example `library-all-grid-*`, `library-all-list-*`, `library-favorites-*`, `library-recent-*`, `sets-*`) show no composer on Ave Maria and no Composers list; the later ones (for example `library-composer-*`, `library-all-sidebar-collapsed-*`, `details-library-*`, `metadata-editor-*`, `set-dialog-*`, `add-picker-*`) show Schubert, Bach and Palestrina. The early captures therefore have no Composers list in the side panel and no composer chips; the Compose captures should use the later state (with the Composers list).
- Favorites (newest first): Te Deum laudamus, Magnificat, Ave Maria. That is 3.
- Recent (newest first): Alleluja in the early captures (count 1); later captures show a count of 2 and the second entry is not visible.
- Sets, most recently edited first:
  1. "Niedziela Palmowa", date 2026-03-29, notes "Msza 10:00, kantor: Anna", 9 items in this order: 1 Ave Maria, 2 Kyrie eleison, 3 Magnificat, 4 Ave Maria (a repeat), 5 Alleluja, 6 Sanctus, 7 the long-title score (row 5 above), 8 "Pan Jezus już się zbliża" (not in the library: shows "Missing from library"), 9 Alleluja (a repeat).
  2. "Pusty zestaw", no date, no items. Its notes are not visible.
  3. "Wielki Piatek - liturgia Meki Panskiej z adoracja krzyza i procesja do Grob…" (a long name, cut off in every screenshot), date 2026-04-03, no items. Its notes are not visible.
- Not in the fixture: tags (the Tags row of the details screen shows "—"), score numbers, text notes.


## Checklist

Each line is one action and what should happen. Section and screen names are the English UI strings; every string also exists in Polish and follows the Language setting.

### Setup and recovery (Welcome)

- [ ] Launch with no saved folder → Welcome screen: logo, "Welcome to Notae", the one-line intro, a card with "Files stay where they are" and "Find any score fast", **Choose score folder** and "Pick a folder on device storage or an SD card."
- [ ] Launch while the library is still loading → a plain "Loading library…" screen, never the Welcome screen, until the saved folder is known.
- [ ] Launch with a saved folder whose grant was revoked → the "permission lost" setup screen: "Reconnect your library", "Notae no longer has access to “<folder>”. Choose it again to reconnect your scores." and **Reconnect folder** (Pick again). 0.3.0 shows the plain Welcome screen here instead; Compose must show the recovery screen.
- [ ] Tap **Choose score folder** → Android's folder picker opens, starting at the saved folder when there is one.
- [ ] Choose a folder in the picker and allow access → the Library screen opens on the chosen folder.
- [ ] Cancel the picker → Welcome stays as it was, nothing changes, no error.
- [ ] Choose a folder when the saved root cannot be reopened (permission lost) → "Reconnect your library" with "Notae no longer has access to “<folder>”. Choose it again to reconnect your scores." and **Reconnect folder**; tapping it opens the picker again.
- [ ] Fail to open the local index or the worker (storage error) → Welcome stays visible with the error text in red above the button; **Choose score folder** retries.
- [ ] Narrow window (under 420 px wide) → smaller title; on a short landscape window the intro scrolls above the fixed button.

### Library header and navigation

- [ ] Open the app on a library → Library screen on the section last used (All scores by default), in the layout (grid or list) last used.
- [ ] Wide window (840 px or more logical) → the side panel is visible: logo and "Notae", All scores, Recent, Favorites, Sets (each with a count), a "Composers" list when any score has a composer (each with a count), and **Settings** at the bottom.
- [ ] Tap a side-panel row → that section opens, the row is highlighted, the title and "N scores" subtitle change, and any open details view closes.
- [ ] Tap a composer in the side panel → only that composer's scores show, titled with the composer's name; matching ignores case, and case variants of one name are one entry.
- [ ] Open a composer section whose last score is then edited away → the library falls back to All scores.
- [ ] Tap **Hide sidebar** (panel icon in the header, wide windows only) → the panel slides out and the content fills the width; the icon becomes **Show sidebar**. Landscape and portrait remember their choice separately and keep it across restarts; portrait starts collapsed.
- [ ] Narrow window (under 840 px, including a phone and the 800 px portrait tablet) → no side panel and no sidebar toggle; a row of chips (All scores, Recent, Favorites, Sets, then composers) replaces it, scrolls sideways when it does not fit, and the header gains a **Settings** icon.
- [ ] Tap a chip → same as tapping the side-panel row; the selected chip is filled. Inside a set, the Sets chip is the selected one.
- [ ] Tap **Search** (magnifier in the header) → the Search screen opens.
- [ ] Tap **Switch to list view** or **Switch to grid view** in the header → the section switches layout, and the choice is kept across restarts.
- [ ] Tap **Scan library** (right-most header icon) → a library scan starts and the operation banner appears; the icon is disabled while any operation runs.
- [ ] Tap **Settings** (side panel, or the header icon when the panel is not shown) → the Settings screen opens.
- [ ] Restart the app → it reopens on the saved section, including an open set when that set still exists.

### Score cards and rows (All scores, Recent, Favorites, composers, inside a set)

- [ ] Tap a card or row → the score opens at once in the external PDF app (no preview step). On return, the same section and scroll position are shown, and the score is first in Recent.
- [ ] Press Enter or Space on a focused card or row, or use the screen reader's default action → same as a tap.
- [ ] A card shows the page thumbnail, title (one line, elided), then "number · composer" (either part alone when the other is missing), a star badge when it is a favorite, and a ⋮. A row shows the same in one line plus the tags line when there are tags. Inside a set a number badge also shows the position.
- [ ] A score without a thumbnail yet → a blank page placeholder until the scan draws it.
- [ ] No PDF app can open the file → a red message appears under the header; the score is not added to Recent.
- [ ] Tap the **⋮** on a card or row → the actions sheet opens (see next section); the content behind it ignores taps.
- [ ] Scroll the grid or list → the page scrolls; a drag handle (scrollbar) on the right edge scrolls it too.
- [ ] Empty All scores → folder icon, "A little space for your scores", "Scan your library folder to add the PDFs it contains." and a **Scan library** button that starts a scan.
- [ ] Empty Recent → "No recent scores", "Scores you open appear here." Recent holds the 30 newest, newest first.
- [ ] Empty Favorites → "No favorites yet", "Star a score to keep it here." Favorites are newest first.
- [ ] Empty composer section → "No scores here" with "No scores by this composer are in the library."

### Score actions sheet (⋮)

- [ ] The sheet opens at the bottom (640 px wide at most): handle, page thumbnail, title, "number · composer", a divider, then the actions. Tapping the dimmed area, Android Back or Escape closes it without doing anything.
- [ ] **Open score** → closes the sheet and opens the PDF as a tap on the card does.
- [ ] **Add to favorites** / **Remove from favorites** → toggles the star; the badge, the Favorites count and the section update; saved across restarts.
- [ ] **Add to set…** → closes the sheet and opens the add-to-set sheet.
- [ ] **Score details** → closes the sheet and opens the full-width details screen for this score (inside a set, for this copy of a repeated piece).
- [ ] **Edit details** → closes the sheet and opens the metadata editor.
- [ ] **Remove from set** (inside a set only) → the piece leaves the set at once, without confirmation, and the positions renumber; the PDF is untouched.
- [ ] A piece missing from the library (inside a set) → its sheet shows the title with "Missing from library" and only **Remove from set**.
- [ ] Focus lands on the first action when the sheet opens; Tab stays inside it.

### Score details screen

- [ ] The details screen replaces the whole library chrome (no side panel, chips or header): back arrow, "N of M" position, favorite star, previous, next.
- [ ] Wide layout (800 px or more) → page preview on the left; on the right the "No. <number>" line, title, composer, **Open score**, an **Edit details** icon, and an info card with Title, Composer, Tags, File (path and its status). Narrow layout stacks them centered. Missing values show "—". The page scrolls when it is taller than the window.
- [ ] Tap **Back to library**, press Android Back or Escape → returns to the layout, section and scroll position it came from.
- [ ] Tap the star (**Add to favorites** / **Remove from favorites**) → toggles the favorite; the star is filled while it is one.
- [ ] Tap **Previous score** / **Next score**, or press Left / Right arrow → moves to the neighbour in the current section's order, without wrapping; at either end the button is disabled. Inside a set it follows the set's order and each copy of a repeated piece is its own step.
- [ ] A score opened from Search that is not in the current section → no neighbours; both buttons are disabled.
- [ ] Tap **Open score** → opens the PDF in the external viewer and records it in Recent.
- [ ] Tap **Edit details** → opens the metadata editor.
- [ ] A refresh removes the shown score → the details screen closes and does not reopen if the score returns.
- [ ] Arrow keys and Back do nothing while a sheet or dialog is open, or on Search or Settings.

### Metadata editor ("Score details" dialog)

- [ ] The dialog shows "Update the details without renaming your PDF." with Title, Composer or artist, Score number (optional) and Tags (comma-separated) fields filled from the score, and **Cancel** and **Save details**. Close (×), Cancel, Back or Escape discard the edit.
- [ ] Edit the fields and tap **Save details** → the dialog closes, the score's title, composer, number and tags update everywhere (cards, details, search, composer list), and the PDF is not renamed. Tags are split on commas, trimmed, and empty ones dropped.
- [ ] Save with an empty title → the dialog stays open with the red message "title must not be empty"; nothing is saved.
- [ ] Any other save or load failure → the error text appears in red in the dialog.

### Search

- [ ] The Search screen opens with the keyboard up and the field focused, a back arrow, the field with a clear (×) button and a microphone, and the scope chips All, Titles, Composers, Lyrics, Tags (All selected). Ctrl K does the same from any library screen.
- [ ] Empty query → "Search your library", "Find scores by title, composer, tag or lyrics."
- [ ] Type a query → results update as you type, scoped by the chip; each result shows the thumbnail, title and a snippet with the matched word in bold blue.
- [ ] Tap a scope chip → the same query runs in that scope and the chip is selected; the chips scroll sideways when they do not fit.
- [ ] No match → "No scores found", "Try another word, or switch the scope above."
- [ ] Tap the clear (×) button → the query empties and the empty prompt returns.
- [ ] Tap the microphone (shown only when a speech recognizer exists) → the system voice recognizer opens; the spoken phrase fills the query and searches. If the recognizer is unavailable the button disappears.
- [ ] Tap a result → the Search screen closes and that score's details screen opens.
- [ ] Tap the back arrow, press Android Back or Escape → returns to the library, the section as it was.
- [ ] The operation banner shows here too while a scan runs.

### Operation banner (library, Search, Settings)

- [ ] Scan running → a card with the phase text and a bar: "Scanning library…", "Indexing N PDFs…", "Reading PDF text and thumbnails: X of N" (a filled bar), "Checking for a desktop sidecar…" (a sweeping bar), and a **Cancel** button.
- [ ] Tap **Cancel** → the scan stops and the banner reports the outcome.
- [ ] Scan done → "Finished: <summary>" with a **Dismiss** button; it disappears by itself after 5 seconds, or at once on **Dismiss**. The grid fills in and the counts update.
- [ ] Scan failed → the reason text in the failure style with **Dismiss**; it stays until dismissed.
- [ ] While any operation is running or showing a result → the Scan icon and the **Scan library** button are disabled.
- [ ] Scan a library with new, changed or deleted PDFs → new scores appear, deleted ones go (and their favorites and set items show "Missing from library" until the file is back), and a replaced PDF at the same path re-links its favorites and set items.

### Settings

- [ ] The Settings screen shows a back arrow, "Settings", and cards for Library folder, Library maintenance, About Notae, Language and Sets and favorites; the page scrolls and has a scrollbar handle.
- [ ] Tap **Back to library**, Android Back or Escape → returns to the library.
- [ ] Library folder card → the folder's name and a **Change** button; **Change** opens the folder picker like the Welcome screen and, once a folder is chosen, switches the library to it.
- [ ] Library maintenance card → "Indexed scores", "PDFs without extractable text", "Last scan" ("No scan this session" before one), and, after a scan, "ID conflicts" and "Failures" (red when not 0).
- [ ] About Notae card → Version, Build, License, Repository, Sidecar schema, and the note "Your library stays on this device. Notae does not send data over the network."
- [ ] Language card → **System default**, **English**, **Polski**; the current one has a check mark. Choosing one switches the whole UI at once and keeps the choice (Android's per-app language when the system has it, Notae's own preference otherwise).
- [ ] Sets and favorites card → **Export sets** opens Android's save-file picker with a suggested name `notae-sets-<date>.json`. Saving shows "Favorites and sets exported" under the button; cancelling shows nothing; a failure shows "Could not export sets: <reason>".
- [ ] The operation banner shows at the top of Settings while a scan runs.

### Sets list (Sets section)

- [ ] Open Sets → the title "Sets", the subtitle "N sets · recently edited first", a **New set** button in the header and the sets as folder cards (grid) or rows (list), most recently edited first.
- [ ] A grid card shows the folder with its count badge ("N items"), the name, the date ("29 Mar 2026" or "No date") and "Edited today", "Edited yesterday", "Edited N days ago" or "Edited <date>"; a list row shows the same on two lines with the edit time at the right.
- [ ] Tap a set → the set opens (see "Inside a set").
- [ ] Tap **New set** → the set dialog opens empty.
- [ ] Tap the ⋮ on a set → the set sheet opens with the folder icon, the name, "date · N items", **Edit set** and **Delete set**.
- [ ] **Edit set** → closes the sheet and opens the set dialog filled with the set's name, date and notes.
- [ ] **Delete set** → closes the sheet and opens the delete confirmation.
- [ ] Switch grid or list in the header → the same global layout choice applies as in the library.
- [ ] No sets → "No sets yet", "Make a set for each Mass or service, then add its pieces."
- [ ] Android Back or Escape inside a set → returns to the Sets list.

### Set dialog (New set / Edit set)

- [ ] The dialog titled "New set" shows "Only the name is required. You add pieces after saving."; "Edit set" omits that line. Fields: Name ("Set name"), Date (optional, "YYYY-MM-DD") with a **Clear date** button, Notes (optional, "Occasion, time, cantor…"), **Cancel** and **Save set**. The name field is focused on open.
- [ ] Save a new set with a name → the dialog closes and the new set opens (inside a set, empty), unless the dialog came from "Add to set… → New set".
- [ ] Save an edit → the dialog closes and the set's name, date, notes and "Edited" time update.
- [ ] Save with an empty name → the dialog stays open with "Name is required".
- [ ] Save with a date that is not a real YYYY-MM-DD date → the dialog stays open with "Use a real date as YYYY-MM-DD". A blank date is allowed.
- [ ] Tap **Clear date** → the date field empties.
- [ ] Cancel, Back or Escape → the dialog closes, nothing saved, and a piece waiting from "Add to set… → New set" is forgotten.

### Delete confirmation

- [ ] The dialog reads "Delete “<name>”?" with "The set is removed. Its pieces stay in your library.", **Cancel** (focused) and **Delete**.
- [ ] **Delete** → the set is removed and the Sets list shows without it; the pieces remain in the library.
- [ ] Cancel, Back or Escape → closes and keeps the set.

### Inside a set

- [ ] The header shows a back arrow (**Back to sets**), the set's name as the title, "29 Mar 2026 · 9 pieces" (date, then the piece count, each only when present), the notes line when the set has notes, **Search**, the layout toggle, a **Add** button and Scan.
- [ ] Pieces are in the set's own order, numbered from 1 by a badge on each card or row; the same piece can appear more than once and each copy keeps its own number.
- [ ] Tap a piece → opens its PDF; returning to Notae shows the same set, and Android Back then goes to the Sets list.
- [ ] A piece whose PDF is gone → a placeholder card with its title, "Missing from library" in red, no open action, and a ⋮ that offers **Remove from set**.
- [ ] A piece's ⋮ → the score sheet with the extra **Remove from set**.
- [ ] Tap **Back to sets** → the Sets list.
- [ ] Tap **Add** → the add picker opens.
- [ ] Empty set → "This set is empty", "Pieces you add to this set appear here."
- [ ] Open a piece's details → previous and next follow the set's order, and steps over repeats one by one.

### Add to set sheet

- [ ] The bottom sheet reads "Add to set…" with the piece's title, a checkbox row for each set (most recently edited first; about six show at once, more scroll) and a **New set** row.
- [ ] Tap a set row not containing the piece → it is appended at the end of that set and the row shows as added.
- [ ] Tap a set row that already contains it → the last copy in that set is removed and the row shows as not added.
- [ ] Tap **New set** → the sheet closes and the set dialog opens; saving creates the set with this piece in it and stays on the current screen.
- [ ] Tap outside, Back or Escape → the sheet closes.

### Add picker (Add inside a set)

- [ ] The dialog is titled "Pieces" with a search field (focused), a list of the whole library (title and "number · composer" per row, with a round add button), a footer and **Done**.
- [ ] A blank query lists the whole library; typing searches titles, composers, tags and lyrics and lists the hits; no hits shows "No scores here".
- [ ] Tap a row's add button → the piece is added at the end of the set and the button becomes a check; tap again → the copy added in this session is removed. A piece that was already in the set shows "already in this set" and can still be added again as a repeat.
- [ ] The footer shows "N added at the end of the set" (plural forms in Polish too).
- [ ] Tap **Done**, outside, Back or Escape → the picker closes; what was added stays.

### Removed in 0.5.0 (sidecar import review)

The sidecar import-review flow exists in 0.3.0 but is removed in Task 3; Compose has no counterpart, so these lines are not parity targets and have no screenshots. They are listed so their removal is deliberate.

- [ ] Finish a scan that finds a desktop sidecar → the banner reads "Review N proposed sidecar changes" and stays until decided.
- [ ] Open the review → a dialog "Review desktop import" with the counts line and one card per file (path, change, "On device", "From desktop", imported and preserved fields, OCR flag, conflicts), **Keep local** and **Apply import**. (In 0.3.0 `app.slint` hard-codes `sheet-import-content: false`, so this content never renders; the sheet shows the details form instead.)
- [ ] **Apply import** → "Applying sidecar import…" runs, then "Finished: <summary>".
- [ ] **Keep local**, the close icon, Back or Escape → the preview is discarded and local metadata is kept.

## Deferred features (Compose only)

0.3.0 left these out of the favorites-and-sets work (`docs/superpowers/specs/2026-10-26-favorites-and-sets-design.md`); the Compose app adds them, so there is no Slint screenshot to compare.

- [ ] Reorder (a **Reorder** button in the set header, list layout) → each row gets a drag handle, up and down buttons and remove; up/down move the item one place and work with TalkBack and a keyboard; **Done** leaves reorder mode and restores the previous layout.
- [ ] Reorder by dragging the handle → the item moves and keeps its new position after a restart.
- [ ] Text notes → the Add dialog has a **Text note** tab beside Pieces; a note is added at the end of the set, shown as a tinted card or row between pieces, numbered with the others, editable and removable; the subtitle counts "N notes"; previous and next in the details screen skip notes.
- [ ] "Just opened" → after opening a piece from a set, returning to Notae shows that item with a visible highlight and the label "Just opened"; it is stored per set and does not change the set's edited time.
- [ ] Import → Settings has **Import sets** that reads an exported JSON file through Android's file picker: sets are added beside existing ones, a set with an existing id is replaced after a confirmation that names it, and favorites are merged.
- [ ] First-launch import → on the first launch of 0.5.0 (`com.c2yz.notae`), favorites and sets exported from 0.3.0 can be imported through the same flow, offered once.
