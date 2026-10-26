# Favorites and singing sets: design

Status: reviewed with the owner, 2026-10-26; ready for the implementation plan. Target release: 0.3.0.
Mockup: [Notae Favorites & Singing Sets](https://claude.ai/artifact/TmnPPiUvhXARrTpHvGP4Jx) (9 boards, light and dark).

## 1. Why

The owner sings in a schola and prepares music for each Mass ahead of time. Today Notae finds any
score fast, but there is no way to keep the pieces for one Mass together, in the order they are
sung, and step through them during the service. Favorites cover the other everyday need: the
handful of pieces used most, one tap away.

Success: before Sunday, the owner builds a set for that Mass in a few minutes; during Mass they open
piece 1 in the PDF app, come back to Notae, are still in the set, and open piece 2, without searching.

## 2. Goals, non-goals, anti-goals

Goals

- A **Favorites** section in the library sidebar, next to Recent.
- A **Sets** section below it: named, dated sets of pieces in a chosen order, with text notes
  between pieces.
- Sets reuse the library screen: the same grid/list toggle, cards, rows, sidebar and dialogs.
- Create, edit (name, date, notes), delete sets; add, remove and reorder pieces and notes.

Non-goals

- Sharing or syncing sets between devices or people.
- Writing anything into the library folder. Notae keeps read-only access to it (§5.2).
- Printing or exporting a set.
- Automatic set suggestions or liturgical calendars.

Anti-goals (a failure even if it works)

- A second, different-looking screen for sets. Sets must feel like the library.
- Losing a prepared set because a PDF was re-scanned or its text corrected.
- Anything that slows down opening a piece during Mass (extra confirmations, loading screens).

## 3. What the owner decided

- Favorites is its own section, parallel to Recent.
- Sets sits in the sidebar below Recent (and Favorites).
- Opening Sets shows the library screen with sets as folder cards (grid) or rows (list).
- Each set has a name and metadata, shows its last-modified date, and can be added, renamed,
  updated and deleted.
- Opening a set shows its pieces in grid or list view, in the set's own order, reorderable.
- During Mass: open a piece, return to Notae still inside the set, pick the next one; Back returns
  to the list of sets.
- Confirmed guesses (2026-10-26):
  - Favorites are toggled from a piece's ⋮ actions sheet and from its details screen (a star).
  - A set holds references to pieces, not copies; the same piece can be in several sets; removing
    it from a set never touches the PDF.
  - Pieces are added from the piece's ⋮ sheet ("Add to set…") and from an "Add" button inside a
    set that opens a search picker.
  - Inside a set, the details screen's previous/next arrows follow the set's order.
  - Sets and favorites stay on the tablet.
  - A set can contain **text notes**: short text items placed between pieces
    (e.g. "Psalm: kantor śpiewa wersety, wszyscy refren").

## 4. Screens (see the mockup)

1. **Library sidebar**: All scores, Recent, Favorites, Sets, each with a count. Favorited pieces
   show a star badge on their card.
2. **Favorites**: the normal library grid/list filtered to favorites.
3. **Piece actions sheet**: adds "Add to favorites"/"Remove from favorites" and "Add to set…",
   which opens a checklist of sets plus "New set".
4. **Sets (grid)**: folder cards with name, date, item count and "Edited …"; a "New set" button.
5. **Sets (list)**: one row per set; each set's ⋮ menu has Edit set and Delete set.
6. **New / edit set dialog**: Name (required), Date (optional), Notes (optional).
7. **Inside a set (grid)**: back arrow to Sets, set name as the title, date, counts, notes line;
   cards carry their position number; text notes are tinted cards.
8. **Inside a set (list, reorder mode)**: drag handle plus up/down arrows and remove on each row;
   Done leaves reorder mode.
9. **Add**: a search picker with add/added toggles, and a Text note tab.

## 5. How it is built

### 5.1 Data model

- A **piece reference** is the score's content key (lowercase SHA-256 of the PDF bytes), the same
  key Recent uses today, plus its library-relative path for re-linking (§7).
- **Favorites**: an ordered list of piece references, newest first.
- **Set**: `id` (random, stable), `name`, optional `date` (calendar date, no time), optional
  `notes`, `modified` (timestamp, updated on any change to the set or its items), and an ordered
  list of **items**, plus an optional `last_opened_item`. An item has its own `id` and is either a
  piece reference (content key and library-relative path) or a text note. The same piece may appear
  more than once.
- A piece reference whose PDF is no longer in the library stays in the set and is shown as
  "Missing from library" rather than silently dropped.

### 5.2 Storage

- App-private files next to `recent.txt` and `ui-prefs.txt` under the app's internal data
  directory: `favorites.json` and `sets.json`, both versioned JSON with `schema_version` (the
  export file uses the same shapes).
- Written through a temporary file and rename, like the existing preferences.
- Not in the library folder: Notae's folder access is read-only (`FLAG_GRANT_READ_URI_PERMISSION`
  only) and the welcome screen promises "Nothing is moved, renamed or uploaded". Backup is the
  export/import in §7.

### 5.3 Library screen reuse

- `LibrarySection` gains `Favorites`, `Sets` (the list of sets) and `Set(id)` (one set's items).
- `Favorites` and `Set(id)` produce ordinary `SearchRow` lists, so the existing grid, list,
  thumbnails, ⋮ sheet and details screen are reused. `Set(id)` keeps the set's order. Because a
  piece can repeat, rows inside a set are identified by item id rather than content key:
  selection, the ⋮ sheet and `neighbours()` use that id there (today they match the first row with
  the content key, which would jump between copies).
- The Sets list needs a folder card and a set row. These are new Slint components next to the
  existing score card and row, using the same `Card`, `Theme` and sizes.
- Text notes are shown in the set views only. Previous/next in the details screen skips them.

### 5.4 Reordering

- Up/down buttons on every row in reorder mode are the dependable path: they work with TalkBack
  and keyboards. Drag-to-reorder by the handle is added on top, if Slint's touch handling makes it
  reliable on the tablet; if not, it is dropped without blocking the release.

### 5.5 Translation

All new strings go through `@tr()` with Polish in the catalog, enforced by the existing tests from
the language work (`feat/language-switching`).

## 6. Calls the builder made (owner may overrule)

- Favorites list newest first.
- New pieces and notes are added at the end of the set.
- Deleting a set asks for confirmation; removing one item from a set does not.
- Search stays one global search (Ctrl K and the header icon). The Sets list and the inside-a-set
  header both show the same search icon as the library header (the mockup's set header omits it
  by mistake).
- Old sets are never archived or deleted automatically.
- The line under a set's title in the mockup ("Uroczystość · 10:00 · kantor: …") is the free-text
  Notes field, not separate occasion/time/cantor fields.
- Inside a set, the grid/list toggle is the same global library preference as everywhere else.
  Reorder mode is entered from its own "Reorder" button in the set header and works in the list
  layout; Done returns to the layout the owner had. (The mockup's list toggle jumping straight to
  reorder mode is a mockup shortcut.)
- The open section, including which set is open, is saved with the UI preferences, so if Android
  closes Notae while the PDF app is in front, the owner comes back to the same set.

## 7. Owner decisions from review (2026-10-26)

- **Replaced PDFs re-link by file path.** Each favorite and set item stores the content key and the
  library-relative path. When the key is no longer in the library but a score exists at the stored
  path, the entry switches to that score's key (and the files are rewritten). No re-linking by
  score number. Otherwise the entry shows "Missing from library" and stays in place.
- **"Just opened" highlight.** Inside a set, the item last opened gets a visible highlight and the
  label "Just opened", so the next piece is easy to find on return. Stored per set
  (`last_opened_item`); opening an item does not change the set's `modified` time.
- **Repeats allowed.** The same piece may appear more than once in a set. Every set item has its own
  `id`, carried through the set's rows, selection, the ⋮ sheet and previous/next, so each copy
  keeps its own position.
- **Export and import in Settings.** "Export sets" writes favorites and all sets to one JSON file
  the owner chooses (Android's save-file picker); "Import sets" reads such a file. Import adds the
  file's sets beside existing ones (a set whose `id` already exists is replaced after a
  confirmation that names it) and merges favorites. Notae still never writes into the library
  folder unless the owner picks a location there.
- **Set fields:** name (required), optional date, optional free-text notes.
- **Sets order:** most recently edited first.
- **Text notes are skipped** by previous/next in the details screen.

## 8. Phases

1. Favorites: storage, section, star toggle, tests.
2. Sets storage and the Sets section (grid, list, create/edit/delete).
3. Inside a set: items view, add from the ⋮ sheet and from the picker, remove, set-ordered
   previous/next.
4. Reorder mode (up/down, then drag if reliable).
5. Text notes.
6. "Just opened" highlight and re-linking replaced PDFs.
7. Export and import in Settings.

Each phase ships tests and Polish strings and leaves the app releasable.
