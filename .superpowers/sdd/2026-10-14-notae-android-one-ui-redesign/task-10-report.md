# Task 10: Search screen and Ctrl K

## Status
Implemented and committed as `feat(android): add search screen with scopes and Ctrl K`.

## Implementation
- Added `ui/search.slint` with a focused search field, five scope chips, result cards, lyric-match highlighting/snippets, empty-results message, accessible result controls, and result-selection callbacks.
- Integrated `SearchScreen` into `AppWindow`; added search scope/callback properties and a `FocusScope` handling Ctrl+K, Escape-to-close-search, and Escape-to-clear-details. Retained the Task 9 score-list breakpoint at 1024px.
- Wired search open/query/scope/close/result handlers through `UiEvent`, `SearchScope`, and `OperationController`. Query updates use the current scope. Closing and choosing a result dispatch `CloseSearch`, preserve the active scope for an empty-query projection via `set_search(String::new(), scope)`, and retain existing selection/details behavior while worker rows refresh.
- `sync_window` now sends `search_scope` to Slint.

## TDD evidence
1. Added `search_screen_owns_search_and_keyboard_interactions` before production edits.
2. RED: `cargo test -p notae-android --test details_ui search_screen_owns_search_and_keyboard_interactions` failed as expected because `ui/search.slint` did not exist.
3. GREEN: after implementing screen and wiring, `cargo test -p notae-android --test details_ui` passed (3 tests).
4. The Android crate suite passed: `cargo test -p notae-android` (99 passed, 15 suites).

## Slint corrections
- Initial compile reported `Unknown element 'ListView'` in `search.slint`; imported `ListView` from `std-widgets.slint`, after which Slint compilation and tests passed.
- Used `score` instead of reserved property name `row`, including loop item and result data property; `PagePreview.score` receives the full result model.
- Removed the now-unused `SearchField` import from `app.slint`; it now belongs to `search.slint`.

## Checks
- `cargo test -p notae-android`: passed (99 tests, 15 suites).
- `cargo build -p notae-android`: passed.
- `cargo apk2 check -p notae-android --target aarch64-linux-android`: could not run because Android NDK is unavailable (`Android NDK is not found. Please set the path to the Android NDK with $ANDROID_NDK_ROOT environment variable.`).
- `cargo clippy -p notae-android --all-targets -- -D warnings`: passed.
- Device/emulator check was deliberately deferred per the ledger ruling; no device actions were run.

## Changed task files
- `apps/notae-android/ui/search.slint` (new)
- `apps/notae-android/ui/app.slint`
- `apps/notae-android/src/android_main.rs`
- `apps/notae-android/tests/details_ui.rs`
- `.superpowers/sdd/2026-10-14-notae-android-one-ui-redesign/task-10-report.md` (this report)

## Self-review
Search close and result selection both refresh with an empty query in the scope active before closing, preventing stale search-filtered library rows. Result selection continues to select the note and opens details when the unfiltered worker rows return. The 1024px score-list cutoff is unchanged. `README.md` was already modified and was not included in this task or staged. No new crate dependencies were added; search remains local-only.

## Commit
`feat(android): add search screen with scopes and Ctrl K`

`Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`
