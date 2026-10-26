# Favorites and Singing Sets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Favorites section and named, ordered Singing Sets (pieces plus text notes) to the Notae Android library screen, stored app-private, with export/import.

**Architecture:** Two new pure-Rust stores (`src/favorites.rs`, `src/sets.rs`) hold the data and its JSON files. `LibrarySection` grows `Favorites`, `Sets` and `Set(id)`; set rows carry an item id so repeats keep their own place. The Slint library screen is reused: new folder card and set row components, a set header, a set dialog, an add picker and reorder rows. Export/import uses Android's `ACTION_CREATE_DOCUMENT` / `ACTION_OPEN_DOCUMENT` through the existing JNI activity-result path.

**Tech Stack:** Rust 1.98.1, Slint 1.18 (`@tr` bundled translations), `serde` + `serde_json` (workspace deps, see Global Constraints), JNI via the existing `src/android/saf.rs` helpers, `i_slint_backend_testing` for UI tests.

**Spec:** `docs/superpowers/specs/2026-10-26-favorites-and-sets-design.md`. Mockup: https://claude.ai/artifact/TmnPPiUvhXARrTpHvGP4Jx

## Global Constraints

- Base branch: `feat/favorites-and-sets` is cut from `main`; the language work (`feat/language-switching`, with `pick()`, `@tr` and the translation tests) must be merged or rebased in **before Task 3**, because every new string needs Polish.
- **Dependency approval needed before Task 1:** add `serde = { workspace = true }` and `serde_json = { workspace = true }` to `apps/notae-android/Cargo.toml`. Both already ship in the workspace (`notae-domain`, `notae-index`), so no new package enters `Cargo.lock`, but it is a new direct dependency of the app crate. No other dependency (no `rand`, no `chrono`, no `uuid`).
- Ids: 16 lowercase hex chars from SHA-256 (`sha2`, already a dependency) of the current time in nanoseconds, a process counter and the name/text. Dates: ISO `YYYY-MM-DD` strings, validated by hand. `modified`: Unix seconds (`u64`).
- Files: `favorites.json` and `sets.json` under `internal_data_path()/notae`, next to `recent.txt` and `ui-prefs.txt`; `"schema_version": 1`; written to `<name>.tmp` then renamed. A missing or unreadable file loads as empty; an unknown `schema_version` loads as empty and is **not** overwritten until the user changes something.
- Never write into the library folder.
- Production code: no `unwrap`/`expect`; `#![forbid(unsafe_code)]` in every new module except the JNI file, which follows `src/android/saf.rs`.
- Every new visible string: `@tr()` in Slint or `pick(en, pl)` in Rust, with Polish in `translations/pl/LC_MESSAGES/notae-android.po`; the existing translation tests must pass.
- One global search (icon plus Ctrl K); the Sets list and set header show the same search icon.
- Look: Samsung One UI, blue accent, light and dark, tablet only. Reuse `Card`, `Theme`, `PillButton`, `IconButton`, `Glyph`.
- New `Glyph` kinds need SVGs in `ui/material-icons/`: `star`, `star-filled`, `folder-set` (reuse `folder` if it reads well), `add`, `arrow-up`, `arrow-down`, `drag`, `note`, `more-vertical` (exists).
- Conventional Commits; every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Quality gates from `docs/engineering-standards.md` before each phase is called done, plus the Android arm64 `cargo apk2 check`.

## Review Focus

1. **The same piece twice in one set:** tapping, ⋮, details and previous/next on the second copy must act on the second copy, not jump to the first. Test in Task 6.
2. **A rescan that replaces a PDF (new content key, same path):** favorites and set items re-link by path and the files are rewritten; a PDF that is gone shows "Missing from library" and keeps its place. Test in Task 10.
3. **Coming back from the PDF app after Android killed Notae:** the open set is restored from UI prefs, and a saved `set:<id>` whose set was deleted falls back to Sets, not a blank screen. Test in Task 4.
4. **Corrupt or future-version JSON:** loads as empty without crashing and is not overwritten by an unrelated save. Tests in Tasks 1 and 3.
5. **Importing a file with a set id that already exists:** nothing is replaced until the named confirmation is accepted; Cancel leaves both stores unchanged. Test in Task 11.

---

## Phase 1: Favorites

### Task 1: Favorites store

**Files:**
- Modify: `apps/notae-android/Cargo.toml` (serde, serde_json; after approval)
- Create: `apps/notae-android/src/favorites.rs`
- Modify: `apps/notae-android/src/lib.rs` (declare the module)
- Test: `apps/notae-android/tests/favorites.rs`

**Interfaces:**
- Produces:
  - `pub struct PieceRef { pub key: String, pub path: String }` (serde, `Clone`, `PartialEq`, `Eq`, `Debug`). Shared with sets.
  - `pub struct Favorites` with `from_json(&str) -> Self`, `to_json(&self) -> String`, `load(&Path) -> Self`, `save(&self, &Path) -> io::Result<()>`, `toggle(&mut self, piece: PieceRef) -> bool` (returns the new state; adding puts it first), `contains(&self, key: &str) -> bool`, `items(&self) -> &[PieceRef]`, `merge(&mut self, other: &Favorites)` (adds missing keys at the end, keeps existing order), `relink(&mut self, library: &[(String, String)]) -> bool` (pairs of `(key, path)`; returns true if anything changed; implemented in Task 10).
  - `pub(crate) fn write_atomically(path: &Path, text: &str) -> io::Result<()>` used by sets too.
  - JSON shape: `{"schema_version":1,"items":[{"key":"<64 hex>","path":"Psalmy/005.pdf"}]}`.

- [ ] **Step 1: Write the failing tests** in `tests/favorites.rs`: `toggle_adds_newest_first`, `toggle_twice_removes`, `json_round_trip`, `invalid_keys_and_repeats_are_dropped_on_load` (reuse the 64-hex check from `recent.rs`), `corrupt_json_loads_empty`, `future_schema_version_loads_empty`, `save_writes_through_tmp_and_leaves_no_tmp` (tempfile dir), `merge_keeps_order_and_adds_missing_at_end`.
- [ ] **Step 2: Run** `cargo test -p notae-android --test favorites` and see them fail to compile.
- [ ] **Step 3: Implement** `src/favorites.rs`. Move the content-key check from `recent.rs` into `pub(crate) fn is_content_key` in `favorites.rs` (or keep it in `recent.rs` and make it `pub(crate)`; do not duplicate it).
- [ ] **Step 4: Run** the test file, then `cargo test -p notae-android --test recent`; both pass.
- [ ] **Step 5: Commit** `feat(favorites): store favorites in an app-private JSON file`.

### Task 2: Favorites section, star toggle and badge

**Files:**
- Modify: `src/ui/browse.rs`, `src/ui.rs`, `src/ui/search.rs` (`library_row`), `src/android_main.rs`, `ui/library.slint`, `ui/details.slint`, `ui/app.slint`, `ui/components.slint`, `translations/pl/LC_MESSAGES/notae-android.po`, `ui/material-icons/star*.svg`
- Test: `tests/browse.rs`, `tests/library_actions_ui.rs`, `tests/library_ui.rs`

**Interfaces:**
- Consumes: `Favorites`, `PieceRef` (Task 1).
- Produces:
  - `LibrarySection::Favorites`, key `"favorites"`, title `pick("Favorites", "Ulubione")`, glyph `star`.
  - `section_rows(rows, section, ctx: &SectionContext)` where `pub struct SectionContext<'a> { pub recent: &'a [String], pub favorites: &'a [PieceRef], pub sets: &'a SetStore }`. Task 2 adds the struct with `recent` and `favorites`; Task 3 adds `sets`. Update `sections()` and `preview_neighbours()` callers to match.
  - `sections()` order: All, Recent, Favorites (Sets added in Task 4).
  - `UiModel::set_favorites_path(PathBuf)`, `UiModel::toggle_favorite(&mut self, note_key: &str) -> io::Result<bool>` (looks up the row's `relative_path`), `UiModel::is_favorite(&self, key) -> bool`.
  - `LibraryRow` gains `favorite: bool`; `library_row(row, thumbnail, favorite: bool)`.
  - Slint callback `favorite_toggled(string)` on `AppWindow` (argument: note key); the ⋮ sheet shows "Add to favorites"/"Remove from favorites"; the details header has a star `IconButton` with accessible label "Add to favorites"/"Remove from favorites". Cards and rows show a small filled star badge when `favorite`.
  - The `LIBRARY_ROWS` cache key in `android_main.rs` includes the favorites list.

- [ ] **Step 1: Failing tests.** `browse.rs`: `favorites_section_lists_newest_first_and_skips_missing`, `sections_include_favorites_with_count`, `favorites_key_round_trips`. `library_actions_ui.rs`: `action_sheet_offers_add_to_favorites_then_remove` (open sheet, click "Add to favorites", assert the callback received the key; with `favorite: true` the label is "Remove from favorites"). `library_ui.rs`: `details_star_has_accessible_label`.
- [ ] **Step 2: Run** them and see them fail.
- [ ] **Step 3: Implement.** Wire `favorites.json` beside `recent.txt` in `android_main.rs`; `on_favorite_toggled` calls `toggle_favorite` then `sync_window`. Add the Polish strings: Favorites → "Ulubione", Add to favorites → "Dodaj do ulubionych", Remove from favorites → "Usuń z ulubionych".
- [ ] **Step 4: Run** `cargo test -p notae-android`; all pass, including `translations`.
- [ ] **Step 5: Commit** `feat(favorites): add the Favorites section and star toggle`.

## Phase 2: Sets storage and the Sets section

### Task 3: Set store

**Files:**
- Create: `src/sets.rs`
- Modify: `src/lib.rs`, `src/ui/browse.rs` (`SectionContext.sets`)
- Test: `tests/sets.rs`

**Interfaces:**
- Consumes: `PieceRef`, `write_atomically`, `is_content_key` (Task 1).
- Produces:
  - `pub enum ItemKind { Piece(PieceRef), Note { text: String } }` (serde, internally tagged `"kind": "piece" | "note"`).
  - `pub struct SetItem { pub id: String, pub kind: ItemKind }`.
  - `pub struct Set { pub id: String, pub name: String, pub date: Option<String>, pub notes: Option<String>, pub modified: u64, pub items: Vec<SetItem>, pub last_opened_item: Option<String> }`.
  - `pub struct SetFields { pub name: String, pub date: Option<String>, pub notes: Option<String> }` with `validate(self) -> Result<SetFields, SetFieldError>`: name trimmed and non-empty; empty date/notes become `None`; date must be a real `YYYY-MM-DD` (check month 1–12 and day against month length with leap years).
  - `pub enum SetFieldError { EmptyName, InvalidDate }` (`thiserror`).
  - `pub struct SetStore` with `from_json`, `to_json`, `load`, `save` (same rules as `Favorites`), `sets_by_modified(&self) -> Vec<&Set>` (newest `modified` first, ties by name), `get(&self, id) -> Option<&Set>`, `create(&mut self, fields: SetFields, now: u64) -> String` (returns the id), `update(&mut self, id, fields, now) -> bool`, `delete(&mut self, id) -> bool`, `add_piece(&mut self, set_id, piece: PieceRef, now) -> Option<String>` (appends; returns item id), `add_note(&mut self, set_id, text, now) -> Option<String>`, `remove_item(&mut self, set_id, item_id, now) -> bool`, `move_item(&mut self, set_id, item_id, delta: i32, now) -> bool` (clamped; returns false at an edge), `mark_opened(&mut self, set_id, item_id) -> bool` (does **not** touch `modified`), `sets_containing(&self, key) -> Vec<String>`.
  - `pub fn new_id(seed: &str) -> String` (see Global Constraints).
  - JSON shape: `{"schema_version":1,"sets":[{"id":"…","name":"…","date":"2026-11-01","notes":"…","modified":1793000000,"last_opened_item":null,"items":[{"id":"…","kind":"piece","key":"…","path":"…"},{"id":"…","kind":"note","text":"…"}]}]}`.
- [ ] **Step 1: Failing tests** in `tests/sets.rs`: `create_validates_name_and_date` (`"  "` → `EmptyName`; `"2026-02-30"` → `InvalidDate`; `"2028-02-29"` ok), `sets_order_most_recently_edited_first`, `add_piece_appends_and_allows_repeats` (same key twice → two item ids), `remove_and_move_bump_modified`, `move_item_stops_at_edges`, `mark_opened_keeps_modified`, `delete_removes_set`, `json_round_trip_with_notes_and_pieces`, `corrupt_and_future_version_json_load_empty`, `item_ids_are_unique_within_a_set` (add 100 items).
- [ ] **Step 2: Run** `cargo test -p notae-android --test sets`; fails.
- [ ] **Step 3: Implement** `src/sets.rs`; extend `SectionContext` with `sets`.
- [ ] **Step 4: Run** the tests; pass.
- [ ] **Step 5: Commit** `feat(sets): add the set store and its JSON file`.

### Task 4: Sets section, folder cards, create/edit/delete

**Files:**
- Modify: `src/ui/browse.rs`, `src/ui.rs`, `src/ui_prefs.rs`, `src/android_main.rs`, `ui/library.slint`, `ui/app.slint`, `ui/components.slint`, the `.po` file
- Create: `ui/sets.slint` (folder card, set row, set dialog, delete confirmation; kept out of the 700-line `library.slint`)
- Test: `tests/browse.rs`, `tests/ui_prefs.rs`, new `tests/sets_ui.rs`

**Interfaces:**
- Consumes: `SetStore`, `SetFields`, `SetFieldError` (Task 3).
- Produces:
  - `LibrarySection::Sets` (key `"sets"`, title `pick("Sets", "Zestawy")`, glyph `folder`) and `LibrarySection::Set(String)` (key `"set:<id>"`, title is the set's name). `sections()` order: All, Recent, Favorites, Sets (count = number of sets).
  - `UiPrefs.section: String` (key `section=`, default `"all"`). `UiModel::select_section` saves it; on load, `set:<id>` with an unknown id falls back to `Sets`, and an unknown composer falls back to `All`.
  - Slint struct `SetCard { id: string, name: string, date: string, count: string, edited: string }`; `AppWindow` properties `set_cards: [SetCard]`, `show_set_dialog: bool`, `set_dialog_id: string` (empty = new), `set_dialog_name`, `set_dialog_date`, `set_dialog_notes`, `set_dialog_error: string`, `confirm_delete_set_name: string`; callbacks `new_set_requested()`, `set_opened(string)`, `edit_set_requested(string)`, `delete_set_requested(string)`, `delete_set_confirmed()`, `delete_set_cancelled()`, `set_dialog_saved(string, string, string, string)` (id, name, date, notes), `set_dialog_cancelled()`.
  - `UiModel::set_sets_path`, `create_set(SetFields) -> Result<String, SetFieldError>`, `update_set(id, SetFields) -> Result<(), SetFieldError>`, `delete_set(id)`. Saving a new set opens it (`Set(id)`), as in the mockup.
  - Display helpers in `src/ui/browse.rs`: `format_set_date("2026-11-01") -> "1 Nov 2026"` / Polish `"1 lis 2026"`; `format_edited(modified, now) -> "Edited today" | "Edited yesterday" | "Edited 3 days ago" | "Edited 1 Nov 2026"` (Polish via `pick` and `polish_plural`). The dialog's date field is a text field taking `YYYY-MM-DD` with a clear button, and a `DatePickerPopup` from `std-widgets` if it fits the One UI style; the stored value is always ISO.
  - Grid layout shows folder cards; list layout shows set rows, both from `set_cards`, following `library_list_view`. Each has a ⋮ with "Edit set" and "Delete set". The Sets header has "New set" and the search icon.
- [ ] **Step 1: Failing tests.** `browse.rs`: `sets_section_counts_sets`, `set_key_round_trips`, `format_edited_buckets`, `format_set_date_en_and_pl`. `ui_prefs.rs`: `section_round_trips`, `missing_set_falls_back_to_sets` (through `UiModel`). `sets_ui.rs`: `new_set_dialog_requires_name` (Save with empty name shows the error, callback not fired), `sets_grid_shows_folder_cards`, `delete_asks_for_confirmation_naming_the_set`.
- [ ] **Step 2: Run** them; fail.
- [ ] **Step 3: Implement.** Polish: Sets → "Zestawy", New set → "Nowy zestaw", Edit set → "Edytuj zestaw", Delete set → "Usuń zestaw", Name → "Nazwa", Date (optional) → "Data (opcjonalnie)", Notes (optional) → "Notatki (opcjonalnie)", Save set → "Zapisz zestaw", "Delete “{0}”?" → "Usunąć „{0}”?", "Only the name is required. You add pieces after saving." → "Wymagana jest tylko nazwa. Utwory dodasz po zapisaniu." (owner to review all Polish terms).
- [ ] **Step 4: Run** `cargo test -p notae-android`; all pass, including the viewport tests at the tablet sizes in `tests/support`.
- [ ] **Step 5: Commit** `feat(sets): add the Sets section with create, edit and delete`.

## Phase 3: Inside a set

### Task 5: Set rows and the set header

**Files:**
- Modify: `src/ui/search.rs`, `src/ui/browse.rs`, `src/ui.rs`, `src/android_main.rs`, `ui/library.slint`, `ui/sets.slint`, `ui/app.slint`, the `.po` file
- Test: `tests/browse.rs`, `tests/sets_ui.rs`

**Interfaces:**
- Consumes: `SetStore`, `LibrarySection::Set` (Tasks 3–4).
- Produces:
  - `SearchRow` gains `item_id: Option<String>` (None outside sets) and `missing: bool`. `section_rows` for `Set(id)` returns one row per **piece** item in set order, with `item_id` set; a piece not in the library becomes a placeholder row (`title` = the stored path's file stem, `missing: true`, status `pick("Missing from library", "Brak w bibliotece")`).
  - `pub fn set_entries(rows, set: &Set) -> Vec<SetEntry>` where `pub enum SetEntry { Piece { position: usize, row: SearchRow }, Note { position: usize, item_id: String, text: String } }`; position is 1-based across all items. Notes are added to the Slint model only (Task 9 renders them); Task 5 can render pieces only.
  - `LibraryRow` gains `item_id: string`, `position: string` (empty outside sets), `missing: bool`, `just_opened: bool`.
  - Set header in `LibraryScreen`: back arrow (accessible "Back to sets") → `section_selected("sets")`; title = set name; subtitle = date · `n pieces` · `n notes`; a notes line when set; buttons "Add", "Reorder", search icon. Card and row show the position number when non-empty.
  - Android Back inside `Set(id)` goes to `Sets` (extend the existing back handling in `android_main.rs`).
- [ ] **Step 1: Failing tests.** `browse.rs`: `set_rows_follow_set_order_with_repeats`, `missing_piece_becomes_placeholder_in_place`, `set_entries_number_notes_and_pieces`. `sets_ui.rs`: `set_header_back_returns_to_sets`, `set_cards_show_position_numbers`.
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement.** Opening a missing row does nothing except show the status; its ⋮ offers only "Remove from set".
- [ ] **Step 4: Run** `cargo test -p notae-android`; pass.
- [ ] **Step 5: Commit** `feat(sets): show a set's pieces in set order`.

### Task 6: Selection and previous/next by item id

**Files:**
- Modify: `src/ui.rs`, `src/ui/browse.rs`, `src/android_main.rs`, `ui/library.slint`, `ui/app.slint`
- Test: `tests/browse.rs`, `tests/library_ui.rs`

**Interfaces:**
- Consumes: `SearchRow.item_id` (Task 5).
- Produces:
  - A row's **selection id**: `pub fn row_id(row: &SearchRow) -> &str` = `item_id` if set, else `note_key`. `UiModel.selected_note` becomes `selected: Option<String>` holding a selection id; `select_note`, `selected_row`, `forget_missing_selection` and `neighbours()` all match on `row_id`. `Neighbours.previous/next` hold selection ids.
  - Slint: `score_selected(string)`, `selected_previous_key`, `selected_next_key` and the ⋮ sheet carry the selection id; `open_pdf_requested(relative_path, note_key)` keeps the note key and gains a third argument `item_id` for Task 10.
  - Missing rows are skipped by previous/next.
- [ ] **Step 1: Failing tests.** `browse.rs`: `neighbours_in_set_with_repeats_use_item_ids` (set A, B, A: from the second A, previous is B and next is None; position 3 of 3), `neighbours_skip_missing_rows`. `library_ui.rs`: `selecting_second_copy_keeps_its_position` (details shows "3 of 3").
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement** the rename across `ui.rs` and `android_main.rs`; keep library-wide behaviour unchanged (outside sets `row_id` is the note key).
- [ ] **Step 4: Run** `cargo test -p notae-android`; all existing selection tests still pass.
- [ ] **Step 5: Commit** `fix(sets): track the selected row by item id so repeats keep their place`.

### Task 7: Add to set (⋮ sheet and picker) and remove

**Files:**
- Modify: `src/ui.rs`, `src/android_main.rs`, `ui/library.slint`, `ui/sets.slint`, `ui/app.slint`, the `.po` file
- Test: `tests/sets_ui.rs`, `tests/library_actions_ui.rs`

**Interfaces:**
- Consumes: `SetStore::add_piece`, `remove_item`, `sets_containing` (Task 3); selection ids (Task 6).
- Produces:
  - ⋮ sheet gains "Add to set…" → a checklist sheet: one row per set (`sets_by_modified`), checked if `sets_containing(key)` includes it, plus "New set" (opens the set dialog; on save the piece is added to the new set). Checking appends the piece; unchecking removes the **last** item with that key from that set. Callbacks `add_to_set_requested(string)` (note key), `set_membership_toggled(string, string)` (set id, note key).
  - Inside a set, the ⋮ sheet also has "Remove from set" → `remove_set_item_requested(string)` (item id), no confirmation.
  - Add picker (mockup board AddPieces, "Pieces" tab): a dialog with its own search field over the existing search projection (reuse the operation worker's search used by the global search; scope everything), rows with add/added round buttons (accessible "Add {title} to set" / "Remove {title} from set"), a footer "{n} added at the end of the set" and Done. Rows already in the set before the picker opened show "already in this set" in their meta line and are added again if tapped (repeats allowed). Callbacks `add_picker_opened()`, `add_picker_search_changed(string)`, `add_picker_toggled(string)` (note key), `add_picker_closed()`. Removing in the picker removes only items added during this picker session.
  - `UiModel` methods with matching names; each saves `sets.json`.
- [ ] **Step 1: Failing tests.** `library_actions_ui.rs`: `add_to_set_sheet_lists_sets_with_checks`. `sets_ui.rs`: `remove_from_set_needs_no_confirmation`, `picker_adds_at_end_and_counts`, `picker_untoggle_removes_only_this_sessions_item`.
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement.** Polish: Add to set… → "Dodaj do zestawu…", Remove from set → "Usuń z zestawu", Add → "Dodaj", Pieces → "Utwory", already in this set → "już w tym zestawie", "{n} added at the end of the set" → plural forms via `polish_plural`, Done → "Gotowe".
- [ ] **Step 4: Run** `cargo test -p notae-android`; pass.
- [ ] **Step 5: Commit** `feat(sets): add and remove pieces from a set`.

## Phase 4: Reorder

### Task 8: Reorder mode

**Files:**
- Modify: `src/ui.rs`, `src/android_main.rs`, `ui/sets.slint`, `ui/library.slint`, `ui/app.slint`, the `.po` file, `ui/material-icons/{arrow-up,arrow-down,drag}.svg`
- Test: `tests/sets_ui.rs`

**Interfaces:**
- Consumes: `SetStore::move_item`, `remove_item` (Task 3); set entries (Task 5).
- Produces: `AppWindow.set_reorder_mode: bool` (not persisted; leaving the set ends it). In reorder mode the set always shows the list layout with rows from mockup board SetReorder: position, drag handle, thumbnail or note chip, title and meta, up, down, remove (accessible labels "Move {title} up", "Move {title} down", "Remove {title} from set", "Drag to reorder {title}"). Header subtitle "Drag a row, or use the arrows, to change the order"; "Done" ends it and the previous grid/list choice is shown again (the global preference is never changed). Callbacks `reorder_started()`, `reorder_finished()`, `set_item_moved(string, int)` (item id, −1 or +1). Up is disabled on the first row, down on the last.
- Drag: implement with a `TouchArea` on the handle that moves the row by whole row heights and calls `set_item_moved` per row crossed. Keep it only if it behaves on the tablet emulator; if not, remove the handle and the "Drag a row" wording in the same commit and note it in the PR.
- [ ] **Step 1: Failing tests:** `reorder_mode_shows_list_rows_with_arrows`, `move_down_then_up_restores_order`, `up_disabled_on_first_row`, `done_restores_grid_layout`.
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement.** Polish: Reorder → "Zmień kolejność", Move {0} up → "Przesuń {0} w górę", Move {0} down → "Przesuń {0} w dół", Drag to reorder {0} → "Przeciągnij, aby zmienić kolejność: {0}".
- [ ] **Step 4: Run** `cargo test -p notae-android`; pass.
- [ ] **Step 5: Commit** `feat(sets): reorder a set with up and down buttons`.

## Phase 5: Text notes

### Task 9: Text notes

**Files:**
- Modify: `src/ui.rs`, `src/android_main.rs`, `ui/sets.slint`, `ui/library.slint`, `ui/app.slint`, the `.po` file, `ui/material-icons/note.svg`
- Test: `tests/sets_ui.rs`

**Interfaces:**
- Consumes: `SetStore::add_note`, `SetEntry::Note` (Tasks 3, 5).
- Produces: the add picker's "Text note" tab (multi-line field, "Added at the end of the set.", Cancel / "Add note"; empty text keeps Add disabled); `add_note_requested(string)`. Notes render as tinted cards (grid) and tinted rows (list) in their set position using two new theme colors `Theme.note` / `Theme.on-note` (light `#FFF4E0` / `#6B3A00`, dark `#3A2A12` / `#FFD9A0`, from the mockup). Tapping a note does nothing; its ⋮ offers "Edit note" (same field, `note_edited(string, string)`) and "Remove from set". Notes are never part of previous/next (they are not `SearchRow`s).
- `SetStore::edit_note(&mut self, set_id, item_id, text, now) -> bool` (add to Task 3's tests: `edit_note_bumps_modified`).
- [ ] **Step 1: Failing tests:** `add_note_appends_tinted_card`, `empty_note_cannot_be_added`, `details_previous_next_skip_notes`.
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement.** Polish: Text note → "Notatka", Note → "Notatka", Add note → "Dodaj notatkę", Edit note → "Edytuj notatkę", "Added at the end of the set." → "Dodana na końcu zestawu.". Change the mockup's "Drag it into place afterwards." to match whether drag survived Task 8.
- [ ] **Step 4: Run** `cargo test -p notae-android`; pass.
- [ ] **Step 5: Commit** `feat(sets): add text notes between pieces`.

## Phase 6: "Just opened" and re-linking

### Task 10: "Just opened" highlight and re-linking replaced PDFs

**Files:**
- Modify: `src/favorites.rs`, `src/sets.rs`, `src/ui.rs`, `src/android_main.rs`, `ui/library.slint`, the `.po` file
- Test: `tests/favorites.rs`, `tests/sets.rs`, `tests/sets_ui.rs`

**Interfaces:**
- Consumes: `open_pdf_requested(path, key, item_id)` (Task 6).
- Produces:
  - Opening a piece from inside a set calls `SetStore::mark_opened` and saves; the row with `last_opened_item` gets `just_opened: true`: accent-soft background, a "Just opened" chip, and the label is part of its accessible name. Only one per set.
  - `Favorites::relink` and `SetStore::relink(&mut self, library: &[(String, String)]) -> bool`: for each stored piece whose key is not in the library, if a library entry has the same path, take its key. Called from `UiModel::set_search_rows` after each rescan projection; when it returns true the file is saved.
- [ ] **Step 1: Failing tests.** `favorites.rs` / `sets.rs`: `relink_by_path_when_key_changes`, `relink_leaves_missing_entries_in_place`, `relink_does_not_change_modified`. `sets_ui.rs`: `opening_item_marks_just_opened_without_bumping_modified`, `just_opened_moves_to_the_last_opened_copy`.
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement.** Polish: Just opened → "Ostatnio otwarte".
- [ ] **Step 4: Run** `cargo test -p notae-android`; pass.
- [ ] **Step 5: Commit** `feat(sets): highlight the piece just opened and re-link replaced PDFs`.

## Phase 7: Export and import

### Task 11: Export and import in Settings

**Files:**
- Create: `src/backup.rs` (pure: build and parse the export file, plan an import)
- Create: `src/android/backup.rs` (JNI: launch `ACTION_CREATE_DOCUMENT` / `ACTION_OPEN_DOCUMENT`, write/read the chosen URI through `ContentResolver`), following the request-code and `nativeOnActivityResult` pattern in `src/android/saf.rs`
- Modify: `src/android/mod.rs`, `src/android/saf.rs` (route the two new request codes), `NotaeActivity.java` only if result routing needs it, `src/ui.rs`, `src/android_main.rs`, `ui/setup.slint` (Settings card), `ui/app.slint`, the `.po` file
- Test: new `tests/backup.rs`, `tests/sets_ui.rs`

**Interfaces:**
- Consumes: `Favorites`, `SetStore` (Tasks 1, 3).
- Produces:
  - Export file: `{"schema_version":1,"app":"notae","favorites":[…PieceRef…],"sets":[…Set…]}`, suggested name `notae-sets-YYYY-MM-DD.json`, MIME `application/json`.
  - `pub fn export_json(favorites: &Favorites, sets: &SetStore) -> String`.
  - `pub fn parse_export(text: &str) -> Result<Backup, BackupError>` (`BackupError::{NotNotae, UnsupportedVersion, Invalid}`).
  - `pub struct ImportPlan { pub new_sets: usize, pub replaced: Vec<String> /* names */ }` and `pub fn plan_import(current: &SetStore, backup: &Backup) -> ImportPlan`; `pub fn apply_import(favorites: &mut Favorites, sets: &mut SetStore, backup: Backup)` (adds new sets, replaces same-id sets, merges favorites).
  - Settings card "Sets and favorites" with "Export sets" and "Import sets"; after picking a file, a confirmation: "Import {n} sets?" and, when `replaced` is non-empty, "These sets will be replaced: …" with Import / Cancel. Errors and success show in the existing settings status text.
  - Callbacks `export_sets_requested()`, `import_sets_requested()`, `import_sets_confirmed()`, `import_sets_cancelled()`.
- [ ] **Step 1: Failing tests.** `backup.rs`: `export_then_parse_round_trips`, `plan_counts_new_and_names_replaced`, `apply_merges_favorites_without_duplicates`, `rejects_other_json_and_future_versions`. `sets_ui.rs`: `import_cancel_changes_nothing`, `import_confirmation_names_replaced_sets`.
- [ ] **Step 2: Run**; fail.
- [ ] **Step 3: Implement** the pure module, then the JNI file. Run the Android arm64 `cargo apk2 check` after the JNI part.
- [ ] **Step 4: Run** `cargo test -p notae-android` and the full quality gates; pass.
- [ ] **Step 5: Commit** `feat(sets): export and import sets and favorites from Settings`.

### Task 12: Finish

- [ ] Run every quality gate from `docs/engineering-standards.md` and the Android arm64 check.
- [ ] Build the APK with the local script and check the main flows on the tablet emulator: favorite a piece; build a set with a repeat and a note; reorder; open a piece, return, see "Just opened"; kill the app while the PDF app is in front and come back to the same set; export, delete a set, import.
- [ ] Add the 0.3.0 CHANGELOG entry together with the language work (release itself is a separate step).
- [ ] Delete scratch files.

## Self-review

- Spec coverage: §3 decisions → Tasks 2, 4, 5, 7; §5.1 model → Task 3; §5.2 storage → Tasks 1, 3; §5.3 section reuse and item ids → Tasks 4–6; §5.4 reorder → Task 8; §5.5 translation → every task; §6 calls (newest-first favorites, append at end, delete confirmation, search icon, notes line, global layout toggle, persisted section) → Tasks 2, 4, 5, 7, 8; §7 decisions (re-link by path, "Just opened", repeats, export/import, set fields, ordering, notes skipped) → Tasks 3, 6, 9, 10, 11; §8 phases → Phases 1–7.
- Open items for the owner: approve `serde`/`serde_json` as direct dependencies of the app crate; review all Polish terms ("Zestawy", "Ostatnio otwarte", "Brak w bibliotece").
