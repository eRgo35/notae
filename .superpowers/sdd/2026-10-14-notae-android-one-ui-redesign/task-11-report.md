# Task 11: First run, settings and dialogs

## Status
Implemented and committed as `feat(android): restyle first run, settings and dialogs`.

## Implementation
- Added `ui/setup.slint` with the required `WelcomeScreen` and `SettingsScreen` interfaces, first-run/recovery copy, feature card, library rescan control, reduced-motion setting, and app information card.
- Replaced the old welcome panel and settings view with the new screens in `AppWindow`; the existing operation banner remains visible while settings are open.
- Restyled metadata/import modal sheets and import entries to the One UI card/field palette, and migrated all remaining UI token uses before deleting the nine legacy `Theme` aliases.
- Updated `Chip` to have a 48px component and `ActionInput` hit/accessibility area with a centered visual pill at y=4px and height=40px. Keyboard, pointer, and accessibility activation remain attached to the outer target; the visible pill remains 40px tall.

## TDD evidence
No behavioral Rust or model logic changed, and the brief does not specify tests. Host build compiles the Slint UI. Full requested test suites were run after implementation; no test-first UI test was added.

## Slint corrections and integration notes
- The first host build caught the chip width referring to the old unnamed `text` element; named the visual `label-text` and used its preferred width.
- Slint reported cyclic layout sizing when the new screens derived child widths from their own intrinsic `root.width`. Kept explicit 560px welcome content and 720px settings content widths to remove the cycle; the card placement remains centered. Host build then passed.
- The setup/settings screen replaces the former settings `ScrollView` as required. `CheckBox` remains imported only by `setup.slint`.
- Preserved Task 9's `score:` Slint model property, Task 8's always-present details slot, the >=1024px score list cutoff, and Task 10's search screen and keyboard `FocusScope`.
- Additional ruling 5 correction: 48px interactive/accessibility Chip bounds while retaining the exact 40px visible pill; the outer hit area is transparent and the visual is centered vertically.

## Checks
- `cargo build -p notae-android` — passed (after correcting Slint chip id and sizing loop findings).
- `ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d JAVA_HOME=/usr/lib/jvm/java-25-openjdk JDK_JAVAC_OPTIONS="-source 17 -target 17" JAVA_SOURCE_VERSION=17 JAVA_TARGET_VERSION=17 cargo apk2 check -p notae-android --target aarch64-linux-android` — passed.
- `cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings` — passed.
- `cargo test -p notae-android -p notae-index` — passed, 187 tests across 20 suites.
- No device/emulator checks were run. Task 11 Step 6 is deferred to device check per the constraints ruling.

## Changed files
- `apps/notae-android/ui/setup.slint` (new)
- `apps/notae-android/ui/app.slint`
- `apps/notae-android/ui/components.slint`
- `.superpowers/sdd/2026-10-14-notae-android-one-ui-redesign/task-11-report.md` (this report)

## Self-review
Confirmed no remaining `Theme.surface/container/high/primary/on-primary/primary-container/on-container/outline/success` token references in UI consumers and no corresponding legacy alias declarations remain. The selected details pane and breakpoint are unchanged; chip action handling remains active for all activation modes. No new dependencies were added. README.md was preserved and not staged. Local-only file access and existing UI/API behavior remain unchanged.

## Commit
`feat(android): restyle first run, settings and dialogs`

`Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`
