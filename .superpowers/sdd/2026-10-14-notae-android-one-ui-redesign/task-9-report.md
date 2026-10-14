# Task 9 report: Score details pane

Status: DONE_WITH_CONCERNS
Commit: pending

## Implementation
- Added `apps/notae-android/ui/details.slint` with `ScoreList` and `ScoreDetails`, selected-row styling, score preview/details, metadata origin/status, and open/edit/close callbacks.
- `AppWindow` now exposes `selected: LibraryRow` and `selection_cleared()`, uses the two-argument `open_pdf_requested(string, string)`, and passes the details/list children to `LibraryScreen`'s always-present slot.
- `android_main.rs` synchronizes selected row, selected key and details visibility; selection/close updates `UiModel`; PDF launches record the opened note only when launch succeeds, with recent-save errors logged.

## TDD evidence
- RED: `cargo test -p notae-android --test details_ui` failed because `ui/details.slint` did not exist (expected missing-feature failure).
- During initial integration, that test/build also surfaced Slint errors: `ListView` was not imported, and the detail children were accidentally inserted among `LibraryScreen` properties/callbacks, which Slint rejected. Corrected both and reran.
- GREEN: `cargo test -p notae-android --test details_ui` passed (1 test).
- Full tests: `cargo test -p notae-android -p notae-index` — 185 passed, 0 failed.
- `cargo build -p notae-android` — passed.
- `ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d JAVA_HOME=/usr/lib/jvm/java-25-openjdk JDK_JAVAC_OPTIONS="-source 17 -target 17" JAVA_SOURCE_VERSION=17 JAVA_TARGET_VERSION=17 cargo apk2 check -p notae-android --target aarch64-linux-android` — passed.
- `cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings` — passed cleanly.


## Fix round 1: responsive details breakpoint
- Review finding: at widths just above the 840px expanded threshold, the 400px list left too little width for the fixed-width preview and details content.
- Updated `app.slint` to show the list only at `root.width >= 1024px`; below that, the selected details pane uses the available width and its back button is shown. This retains narrow-window back-to-list behavior while keeping the two panes only when there is enough width.
- TDD RED: `cargo test -p notae-android --test details_ui score_list_stays_hidden_until_details_have_room_for_both_panes` failed because the 1024px threshold was absent.
- GREEN: `cargo test -p notae-android --test details_ui` — 2 passed.
- `cargo build -p notae-android` — Finished successfully.
- Android check: `ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d JAVA_HOME=/usr/lib/jvm/java-25-openjdk JDK_JAVAC_OPTIONS="-source 17 -target 17" JAVA_SOURCE_VERSION=17 JAVA_TARGET_VERSION=17 cargo apk2 check -p notae-android --target aarch64-linux-android` — Finished successfully.
- Clippy: `cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings` — Finished successfully.
- Device verification remains deferred; no emulator/tablet actions were taken.
## Corrections and integration notes
- Renamed the brief's reserved Slint component property `row` to `score` in `ScoreListItem` and `ScoreDetails`, including every `PagePreview` call (`score:`), consistent with Task 8's Slint 1.18 correction.
- Imported `ListView` from `std-widgets.slint`; otherwise Slint reports an unknown element.
- Passed the contents as children of `LibraryScreen`'s slot after its named properties/callbacks; Slint does not permit children interleaved into that property/callback section.
- Preserved the Task 8 slot contract: its children remain in an always-present slot shown by `details-open`. No hidden child is programmatically focused; only user activation of the score-list row focuses its input.
- The selected row's thumbnail is obtained from the existing `LibraryRowsCache` helper, avoiding the incompatible cache-reference argument to `library_row` and keeping thumbnail decoding behavior consistent.

## Changed files
- `apps/notae-android/ui/details.slint` (new)
- `apps/notae-android/ui/app.slint`
- `apps/notae-android/src/android_main.rs`
- `apps/notae-android/tests/details_ui.rs` (new)
- `.superpowers/sdd/2026-10-14-notae-android-one-ui-redesign/task-9-report.md` (this report)

## Self-review / concerns
- The new host test checks the component/API contract, while the Slint build and Android target check verify the actual UI and Rust callback bindings. No new dependencies were added.
- Build and Android-target checks passed after resolving the reported diagnostics; no known implementation blockers remain.

## Deferred to device check
- Per the Task 8 ledger ruling, no emulator or tablet/device interaction was performed. Verify list/detail split layout and selected highlighting, PDF launch/recent count, metadata editing/origin labels, and narrow-width back-to-list behavior during the consolidated device check. This is deferred, not a blocker.
