# Notae Tablet Fixes, Emulator CI, and APK Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the remaining v0.2.0 tablet-feedback issue work that can be verified in the API 35 tablet emulator, add repeatable emulator CI, publish a signed arm64 APK on version tags, and merge reviewed work to `main`.

**Architecture:** Keep the existing Notae Rust/Slint UI and operation state as the product implementation, following the remaining sequential Tasks 4–11 in the existing v0.2.0 issue plan. Add a dedicated x86_64 emulator build/test path without changing the arm64 release target. Extend existing CI with an emulator gate and tag-conditional signed release job; keep release signing material in GitHub Actions secrets.

**Tech Stack:** Rust 1.98.1, Slint 1.18.1, Android SDK API 34/35, NDK r26d, JDK 17 in CI, cargo-apk2 1.4.0, GitHub Actions, `adb`, `apksigner`, GitHub CLI.

**Spec:** `docs/superpowers/specs/2026-10-16-android-emulator-and-release-design.md`. Product issue acceptance and sequencing: `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`, Tasks 4–11 and Task 12 release qualification.

## Global Constraints

- Keep the project's Rust toolchain pinned to 1.98.1; Android min SDK 23, target SDK 34, and NDK r26d remain unchanged unless a verified build requirement forces a reviewed change.
- Execute shared UI work in the order of existing plan Tasks 4–11. Do not overlap edits to `app.slint`, `components.slint`, `ui.rs`, or `android_main.rs`.
- Continue the already approved issue scope; do not silently drop any v0.2.0 issue or close an issue without its acceptance evidence.
- Use SDK ADB at `$ANDROID_SDK_ROOT/platform-tools/adb`, pass an explicit emulator serial, and do not reset the ADB server or clear logs before collecting evidence.
- Build emulator APKs with `--target x86_64-linux-android`; build distributable APKs with `--target aarch64-linux-android`. Do not claim the arm64 APK was runtime-tested on the x86_64 AVD.
- Release keys/passwords must not enter source control or logs. The Cargo APK release keystore must be a single-private-key keystore whose key and store passwords are the same; pinned cargo-apk2 invokes `apksigner` without an explicit alias.
- GitHub Releases are tag-triggered (`v*`) and require passing host, arm64, and emulator jobs. Do not create a version tag or publish a release as part of this plan without a separately approved version/release request.
- Samsung One UI-specific keyboard behavior and real TalkBack acceptance remain physical-device gates; emulator or host tests do not discharge those gates.

---

### Task 0: Boot and baseline the API 35 tablet emulator

**Files:** `docs/android-debugging.md` only if verified instructions need correction; build output under ignored `target/`.

**Consumes:** Existing `notae-api35-tablet` AVD and API 35 x86_64 image. **Produces:** A clean scratch tablet AVD for app interactions plus install/launch and UI-flow evidence.

- [ ] Use the SDK ADB and exact serial from `adb devices -l`. Read `docs/android-debugging.md` before interacting; it documents that `-gpu swiftshader_indirect` segfaults on this host and `-gpu host` is the known working graphics mode.
- [ ] `notae-api35-tablet` currently contains an existing library with 70 indexed scores. Preserve it; do not select/search/scan its scores, clear its app data, or wipe the AVD. Create `notae-test-api35` as a separate API 35 Google APIs x86_64 Pixel Tablet AVD if it does not already exist:

```sh
printf 'no\n' | avdmanager create avd --name notae-test-api35 --package "system-images;android-35;google_apis;x86_64" --device pixel_tablet
```

- [ ] Start the scratch AVD with `-gpu host`, `-no-snapshot-load`, `-no-snapshot-save`, `-no-window`, `-no-audio`, and `-no-boot-anim`; wait for `sys.boot_completed=1`. If it remains offline or exits, capture process output and read-only `adb logcat`/crash data; do not run ADB reset commands or guess at product fixes.
- [ ] Build the development APK for x86_64 with the JDK 17 source/target settings already documented in `docs/android-debugging.md`:

```sh
export ANDROID_SDK_ROOT="$HOME/Android/Sdk"
export ANDROID_HOME="$ANDROID_SDK_ROOT"
export ANDROID_NDK_ROOT="$ANDROID_SDK_ROOT/ndk/android-ndk-r26d"
export PATH="$ANDROID_SDK_ROOT/emulator:$ANDROID_SDK_ROOT/platform-tools:$PATH"
export JAVA_HOME="/usr/lib/jvm/java-25-openjdk"
export JDK_JAVAC_OPTIONS="-source 17 -target 17"
export JAVA_SOURCE_VERSION=17
export JAVA_TARGET_VERSION=17
rustup target add x86_64-linux-android
cargo apk2 build -p notae-android --target x86_64-linux-android
```

- [ ] Install `target/debug/apk/notae-android.apk`, launch `org.notae.android/.NotaeActivity`, verify the app process remains alive, capture a screenshot, and inspect the crash buffer. Record emulator API, logical resolution/density, APK revision, and any failure before reinstalling.
- [ ] Exercise existing Welcome/Setup, folder picker, Library, Search, Details, Settings, and Back flows on the emulator using `docs/android-debugging.md`'s checklist. Save non-sensitive screenshots under ignored `target/emulator-evidence/`; never use real/private scores.
- [ ] Specifically re-verify already-implemented welcome/Settings/Search changes: first-run and recovery copy, short viewport scrolling, Settings Change/maintenance Scan, no Search/Settings header Scan, scoped search, hardware-keyboard hint on/off and clearing, voice recognizer available/absent/result/cancel where the AVD supports them, and operation status. Repeat relevant screens in light/dark, portrait/landscape, and large text. Keep unsupported recognizer/IME cases explicitly unverified.
- [ ] Update the implementation ledger with the emulator serial, API, boot result, and evidence paths. Do not mark physical-only acceptance complete.

### Task 1: Resolve startup/input feedback (#1, #2, #3, #13)

**Files:** Follow the exact ownership and file list in existing release plan Task 4, `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** Task 0 emulator baseline and the existing C0 generated-UI harness. **Produces:** Reproduced issue evidence, minimal fixes chosen from the plan's diagnostic trees, and regression coverage.

- [ ] Execute existing Task 4's reproduction matrix before editing. Record separate findings for startup flash (#1), first-tap caret (#2), chip/layout drift (#3), and Polish input (#13); do not infer the cause of one from another.
- [ ] For each behavior, add/retain the exact behavioral regression described in existing Task 4, observe it fail where feasible, then make the smallest fix at the diagnosed seam. Keep production code within the pinned lint policy.
- [ ] Run the focused tests and Android x86_64/arm64 build checks named by existing Task 4. Exercise the changed paths in portrait/landscape and light/dark emulator configurations; capture screenshots/logs and update `docs/android-debugging.md`.
- [ ] Keep Samsung Keyboard and first-caret acceptance explicitly open if it cannot be reproduced on this API 35 AVD; do not close #2 or #13 based on Gboard/host-only behavior.

### Task 2: Shared accessible motion (#10)

**Files:** Existing plan Task 5 file list and C2 interface in `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** Task 1's startup/focus behavior. **Produces:** Shared duration tokens and animation-scale-aware screen/sheet transitions.

- [ ] Implement the exact C2 tokens and Android animator-scale mapping from existing plan Task 5. Add behavior tests for zero scale, running-sheet dismissal, rapid Back/screen changes, and destination focus.
- [ ] Run the focused Slint UI tests; verify the emulator screen/sheet transitions with Android animations enabled and disabled. Update the old no-animation documentation only after observed behavior matches the new contract.

### Task 3: Persist grid/list and sidebar preferences (#14, #20)

**Files:** Existing plan Task 6 file list and C1 persistence interface in `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** C0/C2/C3. **Produces:** Persisted Grid/List mode, orientation-specific sidebar preference, and `ScoreRow` for Task 4.

- [ ] Implement C1 and its malformed/partial settings and save/reload tests before wiring UI preferences.
- [ ] Implement the accessible view toggle and wide-only sidebar toggle following existing plan Task 6; preserve ordering and use the existing library row model.
- [ ] Verify widths around 840 and 1280 logical pixels, orientation changes, app restart persistence, and light/dark rendering on the emulator. Run the exact focused tests in existing Task 6 and update docs with measured logical bounds.

### Task 4: Full-width preview and item actions (#15, #16, #17)

**Files:** Existing plan Task 7 file list and C4 action contract in `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** Task 3 `ScoreRow`/preferences, existing PDF opener and metadata editor. **Produces:** Full-width preview, direct Recent open, stable-key Edit/context actions, and honest accessibility evidence.

- [ ] Implement Task 7's stable-key callbacks, Back/previous/next semantics, long-press cancellation, Recent open, and preview/editor behavior. Add the listed behavioral tests before implementation and run its browse/recent/metadata targets.
- [ ] On the emulator, capture and exercise preview, external viewer return, Edit details, context actions, menu dismissal, Back restoration, light/dark and breakpoints. Inspect `uiautomator dump` and perform actual TalkBack interactions where the AVD exposes TalkBack.
- [ ] If TalkBack cannot reach or activate the score controls, keep #17's physical accessibility acceptance open and record the specific missing platform behavior. Do not close #17 based on visible labels or host tests.

### Task 5: Themed, draggable scrollbars (#19)

**Files:** Existing plan Task 8 file list and geometry contract in `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** Task 2 C2, Task 3 library layouts, Task 4 preview. **Produces:** One shared Scrollbar component used on the specified long content surfaces.

- [ ] Implement the exact logical geometry and drag mapping from existing plan Task 8; add generated-UI regression checks for fit, endpoints, minimum thumb, reversible midpoint, resize clamping, idle fade, and visible drag.
- [ ] Run the dedicated scrollbar UI target. On emulator, drag long Library/Details content in portrait/landscape and light/dark; inspect logs for binding loops and verify replaced default scrollbars are absent.

### Task 6: Truthful operation phases (#18)

**Files:** Existing plan Task 9 file list and C5 operation ownership in `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** Existing worker events and current state projection. **Produces:** One app-wide operation phase that remains busy through actual post-scan work and does not imply Android OCR.

- [ ] Add the existing plan's failing post-`Completed` sidecar event-order regression using its fake catalog seam; verify it fails before changing ordering.
- [ ] Implement explicit phases only where work can be measured. Preserve cancellation/confirmation semantics and ensure post-scan errors do not contradict successful counts.
- [ ] Run operation/ocr adapter tests and scan a non-sensitive fixture library on the emulator. Observe status across Library/Search/Settings, cancellation, failures, and idle; capture logs and update help copy/docs.

### Task 7: Launcher contrast and icon consistency (#7)

**Files:** Existing plan Task 10 resource list in `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md`.

**Consumes:** Existing icon source and Task 0 emulator. **Produces:** Contrast-verified adaptive, legacy, and monochrome launcher assets.

- [ ] Measure actual source-pixel luminance/contrast before changing assets. Regenerate only verified density variants, retaining distinct monochrome and adaptive layers.
- [ ] Install/upgrade without clearing app data; inspect launcher surfaces in light/dark and themed-icon mode on the emulator. Update icon provenance/contrast docs. Keep any Samsung-specific launcher proof pending physical device.

### Task 8: README presentation and emulator screenshots (#6)

**Files:** `README.md`; new non-sensitive captures in `docs/images/`; use existing app build and documented commands.

**Consumes:** Completed Tasks 1–7. **Produces:** Accurate product overview, download information, quick start, and screenshots labelled by their actual platform.

- [ ] Capture current Library, Search, and full-width preview from the running API 35 tablet emulator with a non-sensitive fixture library. Label these as emulator captures; do not present them as Samsung screenshots.
- [ ] Add project badges and feature overview, a link to the GitHub Releases APK, quick start before technical details, and the accurate workflow diagram from existing plan Task 11. Keep all existing technical instructions intact.
- [ ] Render/inspect the README's relative image links, badges, diagram, download link, and CLI commands. Run CLI `--help`; update issue #6 evidence only with captures from the verified build.

### Task 9: CI tablet emulator build and smoke test

**Files:** `.github/workflows/ci.yml`; create `scripts/android-emulator-smoke.sh`; `docs/engineering-standards.md`; `docs/android-debugging.md`.

**Consumes:** Task 0 successful local x86_64 APK build and existing host UI test seam. **Produces:** A reproducible pull-request/push emulator job that builds, installs, launches, and captures runtime evidence.

- [ ] Add an `android-emulator` job using `actions/setup-java@v4` (Temurin 17) and `android-actions/setup-android@v3`; install `platform-tools`, `platforms;android-34`, `build-tools;34.0.0`, `emulator`, and `system-images;android-35;google_apis;x86_64`.
- [ ] Install NDK `26.3.11579264`, Rust target `x86_64-linux-android`, and pinned cargo-apk2 1.4.0. Create a tablet AVD from the API 35 Google APIs x86_64 image, cold boot it headlessly, and wait for `sys.boot_completed=1` with a bounded timeout that prints useful emulator/ADB diagnostics on failure.
- [ ] Add the smoke script with arguments/env for SDK ADB, APK path, serial, and evidence directory. It must fail for a missing APK/device, install failure, failed activity start, absent app process, or fatal Java/native app crash; save a screenshot and full relevant logcat as workflow artifacts even on failure. Do not clear logcat buffers.
- [ ] Use the built-in x86_64 override, confirmed in cargo-apk2 1.4.0: `cargo apk2 build -p notae-android --target x86_64-linux-android`; verify output at `target/debug/apk/notae-android.apk`. Run host generated-UI behavior tests in the same workflow as the screen-level interaction contract.
- [ ] Validate the script with `bash -n`, run it on the local AVD, and exercise a failing install/start case in a disposable test invocation to verify it returns nonzero. Then run the full workflow job and preserve screenshots/logs as artifacts.

### Task 10: Signed tag-triggered GitHub APK release

**Files:** `.github/workflows/ci.yml`; `apps/notae-android/Cargo.toml`; add `docs/android-release.md`; `README.md` release download section.

**Consumes:** Passing `rust-quality`, `android-target`, and `android-emulator` jobs. **Produces:** A verified signed arm64 APK and SHA-256 checksum attached to a GitHub Release for pushed `v*` tags.

- [ ] Remove the local absolute `$HOME/.android/debug.keystore` release signing configuration from `apps/notae-android/Cargo.toml`. Confirm from pinned cargo-apk2 1.4.0 docs/source that `CARGO_APK_RELEASE_KEYSTORE` and `CARGO_APK_RELEASE_KEYSTORE_PASSWORD` override manifest signing and that signing passes no explicit key alias.
- [ ] Add a tag-conditional release job with `needs: [rust-quality, android-target, android-emulator]`, and job-only `permissions: contents: write`. Decode `NOTAE_RELEASE_KEYSTORE_BASE64` into a mode-0600 file under `$RUNNER_TEMP`, export the supported cargo-apk2 signing variables without printing them, and remove the temporary key via an `always()` cleanup step. Fail before publishing if secrets are absent or keystore validation fails.
- [ ] Build with `cargo apk2 build --release -p notae-android --target aarch64-linux-android`; check `target/release/apk/notae-android.apk`, validate package/version/ABI, verify the signer with SDK `apksigner`, and generate `sha256sum` for the APK.
- [ ] Publish the existing pushed tag using `gh release create "$GITHUB_REF_NAME" --verify-tag --generate-notes` with the APK and checksum as assets and `GH_TOKEN: ${{ github.token }}`. Do not create tags in CI or from this task. Keep tag-triggered release creation gated on all three CI jobs.
- [ ] Verify the release package path and signature flow using a locally configured test keystore that is not committed; document the exact GitHub secret names, required single-key/password constraints, tag format, download behavior, and manual release steps in `docs/android-release.md`.

### Task 11: Final qualification, review, and merge

**Files:** `docs/engineering-standards.md`, `docs/android-debugging.md`, `docs/android-release.md`, `docs/superpowers/IMPLEMENTATION-STATE.md`, the existing SDD ledger, and any issue-specific docs from prior tasks.

**Consumes:** All completed product tasks and CI/release jobs. **Produces:** Verified local branch merged into `main`, with hardware gates explicitly retained where unavailable.

- [ ] Run every documented host quality command, x86_64 emulator build/smoke, arm64 Android check and release APK verification. Run the emulator interaction checklist from the existing plan, including folder selection, screen flows, keyboard/voice availability, light/dark, orientation, operations, Back navigation, and upgrade behavior. Save evidence without private data.
- [ ] Confirm workflows have not weakened existing quality gates, no signing secrets/keystores are tracked, temporary key files are removed, release permissions are minimal, and the release job cannot publish on branch/PR events.
- [ ] Run independent code review on the complete diff; address all blocking findings. Verify every signed commit with GPG, using `pinentry-qt` when a passphrase prompt is requested.
- [ ] Keep Samsung-only keyboard, physical TalkBack, and any tablet-only acceptance open if not observed; do not close those issues or claim a complete v0.2.0 release on emulator evidence alone.
- [ ] Merge the reviewed feature branch into local `main` as requested, then push the merged `main` branch so GitHub CI runs. Do not push a version tag or publish a release without the required keystore secrets and a separately approved release version.

## Release signing secret setup

Before the first release tag, the maintainer must add two repository Actions secrets without sharing their values in chat:

- `NOTAE_RELEASE_KEYSTORE_BASE64`: base64 encoding of the dedicated, long-lived single-private-key release keystore.
- `NOTAE_RELEASE_KEYSTORE_PASSWORD`: its keystore password; the private key uses the same password because cargo-apk2's signer has no separate key-password/alias configuration.

The repository is currently private; GitHub Release assets will be downloadable only to accounts with repository access unless repository visibility is independently changed.

## Execution order

The issue-specific task sequence is intentionally serialized because Tasks 1–8 share UI files and interfaces. Task 9 can be implemented after the emulator build in Task 0, and Task 10 after Task 9. Final qualification and merge are last. No task closes an issue without the acceptance evidence described by its source plan.
