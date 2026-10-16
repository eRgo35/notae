# Task 1: Startup and input feedback

**Status:** Implemented and verified the safe chip-geometry fix for #3 and added regression coverage for index-side Polish query behavior. Startup flash, caret placement, and Android IME acceptance remain open; no cause or fix is claimed for those gates.

## Reproduction matrix and separate findings

Executed on the assigned API 35 scratch AVD using SDK ADB serial `emulator-5556` only. A restored library with one indexed non-sensitive `hex-contents` score was visible. Evidence is under ignored `target/emulator-evidence/task-1/`.

| Issue | Reproduction/evidence | Finding and status |
|---|---|---|
| #1 startup flash | Captured settled restored Library at `current.png`, then installed/relaunched and inspected the restored Library again at `relaunch.png`; no Welcome/empty-library screen was visible. No cold-start video/frame capture established the first rendered frame. | Source order has `sync_window` immediately after `AppWindow::new`, with persisted-root restoration later in `android_main.rs`; this ordering alone does not prove the unwanted visible frame's origin. No startup state/error projection was added without establishing the displayed frame or an actionable failure state. Reproduction/state coverage and acceptance remain open. |
| #2 first-tap caret | Opened Search from Library. Search displayed Gboard and the field's caret at its initial insertion point (`search-landscape-light.png`). After rotating to portrait, a tap inside the field caused the keyboard to disappear (`search-first-field-tap-dark.png`); a second tap did not reopen it (`search-second-field-tap-dark.png`). | This is emulator evidence of unexpected focus/keyboard behavior, not proof of the reported handle-at-caret symptom. No focus timestamp, field rectangle, keyboard resize geometry, or backend cursor rectangle was instrumented. No blind focus workaround was added. Keep #2 open pending both entry paths, refocus after dismissal, cover keyboard, and physical tablet. |
| #3 chip/layout drift | Headless generated-UI test on a 1280×800 Search surface failed before the fix: changing scope moved the following **Titles** chip x from 143 to 142 px. | Confirmed application-level cause: selected **All** changed its font weight from 500 to 600 while chip width derives from preferred text width. Made chip font weight invariant at 500; Search and compact Library Flickables are interactive only when their content exceeds viewport width. Regression passes. Emulator screenshots show the unchanged visible scope row in landscape/portrait, light/dark. Physical press/release, small drift, deliberate drag, and narrow-row gesture tests remain unverified. |
| #13 Polish input | Gboard was visible in Search, but no Polish composition or character entry was performed. Added a real in-memory index-store test for title `Łódź — Pieśń żółta`. | Exact and case-variant title tokens `łódź`, `ŁÓDŹ`, `pieśń`, `żółta` match; `lodz` does not. First red run mistakenly used `lodz.pdf`, which made the filename match `lodz`; corrected fixture to `score.pdf` and the focused test passed. This is evidence about index tokenization only, not the Android backend/query-edited boundary. Keep #13 open pending committed/composing text evidence with Gboard, Samsung Keyboard, hardware Polish layout, editing, and physical testing. |

## TDD evidence

- **#3 RED:** `cargo test -p notae-android --test headers_ui changing_search_scope_keeps_following_chip_bounds_stable -- --exact` failed as expected with `left: 142.0`, `right: 143.0`.
- **#3 GREEN:** The same command passed after removing selected-only font weight change.
- **#13 fixture correction:** First run of `cargo test -p notae-index --test store_and_search title_search_preserves_polish_letters_and_records_accentless_behavior -- --exact` failed because the fixture filename `lodz.pdf` itself matched `lodz`; the failure did not demonstrate an index defect. After using `score.pdf`, that focused test passed and confirmed title-token behavior. No tokenizer/schema workaround was introduced.

## Checks and device evidence

Passed:
- `cargo test -p notae-android --test headers_ui` — 9 passed.
- `cargo test -p notae-android --test search_state` — 10 passed.
- `cargo test -p notae-android --test storage_state` — 9 passed.
- `cargo test -p notae-index --test store_and_search` — 28 passed.
- `cargo apk2 build -p notae-android --target x86_64-linux-android` — succeeded; Java 17 source/target and JDK/apksigner warnings only.
- `cargo apk2 check -p notae-android --target aarch64-linux-android` — succeeded.
- Installed the x86_64 APK on `emulator-5556`, relaunched, opened Search from Library, and captured the Search layout. Crash buffer capture was empty (0 bytes). Gboard presence is visible in screen captures.

Screen captures: `current.png`, `relaunch.png`, `search-landscape-light.png`, `search-landscape-dark.png`, `search-portrait-dark.png`, `search-portrait-light.png`, `search-first-field-tap-dark.png`, `search-second-field-tap-dark.png`; `crash-buffer.txt` is empty. AVD theme and rotation were restored to light and landscape after captures. No logs were cleared, and `emulator-5554` was not used.

## Changed files

- `apps/notae-android/ui/components.slint` — invariant Chip label font weight.
- `apps/notae-android/ui/search.slint`, `apps/notae-android/ui/library.slint` — allow horizontal Flickable gestures only when overflowing.
- `apps/notae-android/tests/headers_ui.rs` — generated-UI chip-bound behavioral regression.
- `crates/notae-index/tests/store_and_search.rs` — Polish exact/case/accentless query regression and tokenizer observation.
- `docs/android-debugging.md` — separate issue evidence, limitations, and device gates.

## Self-review and commits

Self-review found no unrelated production changes, dependency changes, source-text assertions, arbitrary timeouts, ASCII substitutions, or unsafe/unchecked production calls. The test checks geometry through the generated UI; the index test uses the real `IndexStore` in-memory fixture. Both Flickable seams (Search and compact Library) received the same overflow predicate. Note the Search Flickable padding and Library's existing section population remain unchanged.

- Signed implementation commit: `5ae7a6ec73e90ca30ab729347ec8454e503e1e6e` — `fix(search): stabilize scope chip layout` (`Refs #3`). Signature verified as a good signature from Michał Czyż.
- This report will be committed separately with a GPG signature; its commit SHA is returned in the task handoff.

## Unresolved acceptance gates

- #1: cold-launch recording must identify the first visible frame; implement and test Restoring/LoadingRows/Ready plus actionable worker failure; exercise true saved-root restoration and recovery.
- #2: instrument focus/viewport/field geometry and reproduce the first-tap handle placement on tablet with soft and cover keyboards, both entry paths, and post-dismissal refocus. The observed AVD keyboard dismissal needs controlled follow-up, not a guessed fix.
- #3: actual touch drift/drag testing on Search and compact Library, including a narrow overflowing row, on the Samsung tablet remains necessary.
- #13: inspect backend committed/composing strings and query-edited receipt using Gboard and Samsung Keyboard plus hardware Polish layout. Typing, composing/long-press alternates, deletion, and cursor movement on physical hardware remain unverified.
