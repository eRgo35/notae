# Final branch review fix report

## Status

All three findings from the final whole-branch review (`6d4c0f5..5db2d83`) are addressed. Changes are local-only; no dependencies were added. Commit: pending.

## Findings addressed

1. **Search scopes unreachable on compact widths** — Wrapped all five scope chips in a horizontally scrollable `ScrollView`, with the existing 48px chip target / 40px visible pill unchanged. The horizontal scrollbar is hidden and touch drag panning is enabled, while all scope chips remain in the row and can be reached.
2. **Three-pane details too narrow at 1024px** — Raised the score-list / details split threshold from 1024px to 1280px. Below 1280px, the score list remains hidden and details retain full available width with the existing back-to-list behavior. The separate `expanded` threshold remains 840px.
3. **Welcome and settings fixed content widths** — Constrained the welcome text/card to a 560px maximum and settings content to a 720px maximum, each bounded by externally supplied available-width values. This avoids deriving child widths from the component's intrinsic width and thus avoids cyclic intrinsic sizing.

## Files changed

- `apps/notae-android/ui/search.slint`
- `apps/notae-android/ui/app.slint`
- `apps/notae-android/ui/setup.slint`
- `apps/notae-android/tests/details_ui.rs` — Updated the pre-existing breakpoint assertion to expect the new 1280px threshold; no source-string tests were added.
- `.superpowers/sdd/2026-10-14-notae-android-one-ui-redesign/final-review-fix-report.md` — this report.

The pre-existing uncommitted changes in `README.md` and `task-10-report.md` were not part of this fix and must remain unstaged/uncommitted.

## Test approach

There is no existing runtime UI layout test harness for these Slint screens. No source-string test was added. The existing focused details UI contract test was updated to track the changed split threshold; the Slint compiler/build validated the screen definitions and actual UI layout bindings.

## Verification commands and outputs

Focused:

- `cargo test -p notae-android --test details_ui` — **passed**, 3 passed, 0 failed.

Full/requested:

- `cargo build -p notae-android` — **passed**, finished dev profile.
- `cargo test -p notae-android -p notae-index` — **passed**, 187 passed across 20 suites, 0 failed.
- `cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings` — **passed**, finished dev profile with no warnings.
- `ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d JAVA_HOME=/usr/lib/jvm/java-25-openjdk JDK_JAVAC_OPTIONS="-source 17 -target 17" JAVA_SOURCE_VERSION=17 JAVA_TARGET_VERSION=17 cargo apk2 check -p notae-android --target aarch64-linux-android` — **passed**, finished dev profile.

The first build attempt caught a cyclic width binding in the initial responsive setup sizing. The implementation was changed to pass the app viewport's available width into setup screens explicitly; the subsequent build and all checks above passed. The first full test run caught the obsolete 1024px assertion; updating that existing assertion resolved it, and the focused and full suites then passed.

## Limits and deferred checks

No emulator/device interaction, app installation, APK launch, or visual device check was performed. Device/emulator verification remains deferred per the project constraints. No additional runtime UI layout harness exists, so responsive layout was checked via Slint compilation and the existing host test suite rather than on-device rendering.