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
- GitHub Releases are tag-triggered (`v*`) and require passing host, arm64, and emulator jobs. A manual workflow dispatch may build/sign and validate in dry-run mode but must never publish a release. Do not create a version tag or publish a release as part of this plan without a separately approved version/release request.
- Samsung One UI-specific keyboard behavior and real TalkBack acceptance remain physical-device gates; emulator or host tests do not discharge those gates. If #17's controls are not exposed to Android accessibility, implement the verified Slint bridge/upgrade required by the existing issue plan before marking the code task complete.

---

### Task 0: Boot and baseline the API 35 tablet emulator

**Files:** `docs/android-debugging.md` only if verified instructions need correction; build output under ignored `target/`.

**Consumes:** Existing `notae-api35-tablet` AVD and API 35 x86_64 image. **Produces:** A clean scratch tablet AVD for app interactions plus install/launch and UI-flow evidence.

- [ ] Use the SDK ADB and exact serial from `adb devices -l`. Read `docs/android-debugging.md` before interacting; it documents that `-gpu swiftshader_indirect` segfaults on this host and `-gpu host` is the known working graphics mode.
- [ ] `notae-api35-tablet` currently contains an existing library with 70 indexed scores. Preserve it; do not select/search/scan its scores, clear its app data, or wipe the AVD. Create `notae-test-api35` as a separate API 35 Google APIs x86_64 Pixel Tablet AVD if it does not already exist:

```sh
printf 'no\n' | avdmanager create avd --name notae-test-api35 --package "system-images;android-35;google_apis;x86_64" --device pixel_tablet
```
- [ ] Verify creation with `avdmanager list avd` before retrying. On this host `avdmanager create avd` printed a missing `devices.xml` error but did create the named AVD; do not repeat creation if the AVD is listed.

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

Issue-level progress:
- [x] #1 saved-root startup: implementation, transition tests, generated no-root Welcome gate, cold-start AVD evidence, docs and focused checks completed; see `docs/android-debugging.md`.
- [ ] #2 first-focus caret: AVD geometry diagnosis recorded; Opus-reviewed fix and physical Samsung caret acceptance remain outstanding.
- [ ] #3 chip geometry and #13 Polish IME: current-surface verification remains outstanding; #13 requires Samsung Keyboard/hardware-layout evidence.

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
- [ ] If Android accessibility cannot reach/activate the controls, identify and adopt a verified Slint accessibility implementation or implement the missing native accessibility bridge as required by existing plan Task 7. Test the bridge with `uiautomator dump` and real TalkBack gestures on the AVD; do not mark #17's code task complete until controls are exposed and operable. Keep physical TalkBack acceptance open until verified on hardware; do not claim visible labels or host tests prove it.

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

### Task 9: CI tablet emulator build and interaction smoke test

**Files:** `.github/workflows/ci.yml`; create `scripts/android-emulator-smoke.py`; `docs/engineering-standards.md`; `docs/android-debugging.md`.

**Consumes:** Task 0 successful local x86_64 APK build, clean AVD, and accessibility controls from Task 4. **Produces:** Repeatable pull-request/push emulator coverage of a real folder-picker path, navigation, screenshots, and crash evidence.

- [ ] Add an `android-emulator` job using `actions/setup-java@v4` (Temurin 17) and `android-actions/setup-android@v3`; install `platform-tools`, `platforms;android-34`, `build-tools;34.0.0`, `emulator`, and `system-images;android-35;google_apis;x86_64`.
- [ ] Before booting the x86_64 AVD, enable KVM for the GitHub runner exactly as documented by GitHub: write `KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"` to `/etc/udev/rules.d/99-kvm4all.rules`, run `sudo udevadm control --reload-rules` and `sudo udevadm trigger --name-match=kvm`, and assert `/dev/kvm` is readable/writable.
- [ ] Install NDK `26.3.11579264`, Rust target `x86_64-linux-android`, and pinned cargo-apk2 1.4.0. Create a `pixel_tablet` AVD from API 35 Google APIs x86_64, cold boot headlessly with hardware acceleration, and wait for `sys.boot_completed=1` with a bounded timeout that prints useful emulator/ADB diagnostics on failure.
- [ ] Build with `cargo apk2 build -p notae-android --target x86_64-linux-android`; verify `target/debug/apk/notae-android.apk`. Create an empty `Download/NotaeSmoke` folder on the disposable emulator.
- [ ] Implement the Python-standard-library smoke driver. It must install the APK; launch `org.notae.android/.NotaeActivity`; assert the resumed foreground activity and process; inspect `uiautomator dump` XML and find/click controls by exact visible text/content description and returned bounds (never fixed screen coordinates); use Android DocumentsUI to select `Download/NotaeSmoke` and grant SAF access; assert Notae resumes into Library; navigate to Settings, return with Back, open Search, enter a query, and verify the expected screen/query state. If app controls are absent from the accessibility tree, fail with the hierarchy/screenshot and fix the missing bridge through Task 4 rather than weakening the check.
- [ ] Capture screenshot and relevant logcat plus UI hierarchy at each checkpoint, including on failure; fail for missing APK/device, boot timeout, missing expected control, picker grant failure, wrong resumed activity, missing process, or fatal Java/native crash. Do not clear logcat buffers.
- [ ] Run `python3 -m py_compile scripts/android-emulator-smoke.py`, `cargo test --workspace` remains in `rust-quality`, and execute the smoke driver against both local scratch AVD and GitHub runner. Upload screenshots/logcat/UI hierarchy as workflow artifacts with `if: always()` and a short retention period. Include a controlled bad-APK invocation to verify the driver returns nonzero.

### Task 10: Signed tag-triggered GitHub APK release

**Files:** `.github/workflows/ci.yml`; `apps/notae-android/Cargo.toml`; `docs/android-debugging.md`; add `docs/android-release.md`; `README.md` release download section.

**Consumes:** Passing `rust-quality`, `android-target`, and `android-emulator` jobs. **Produces:** A verified signed arm64 APK and SHA-256 checksum attached to a GitHub Release for pushed `v*` tags, plus a non-publishing manual dry-run.

- [ ] Remove the local absolute `$HOME/.android/debug.keystore` release signing configuration from `apps/notae-android/Cargo.toml`. Confirm from pinned cargo-apk2 1.4.0 docs/source that `CARGO_APK_RELEASE_KEYSTORE` and `CARGO_APK_RELEASE_KEYSTORE_PASSWORD` override manifest signing and that signing passes no explicit key alias.
- [ ] Add `workflow_dispatch` with a boolean `dry_run` input defaulting to true. A manual run with `dry_run=false` must skip all release jobs; manual dry-run builds/signs/checks but cannot publish.
- [ ] Add `android-release-build` with `needs: [rust-quality, android-target, android-emulator]`, `contents: read`, and an `if` allowing pull requests, branch pushes, `refs/tags/v*` pushes, or manual dispatches with `dry_run=true`. Pull requests and branch pushes must generate a disposable single-key test keystore under `$RUNNER_TEMP` with a random password; never expose release secrets to those events. Tags and manual dry-runs must use `NOTAE_RELEASE_KEYSTORE_BASE64` and `NOTAE_RELEASE_KEYSTORE_PASSWORD`, decode to a mode-0600 file, export Cargo env vars without printing values, and remove temporary keys in `always()` cleanup. Apply `::add-mask::` to any generated/secret password before use. Fail if required real-release secrets or key validation are missing.
- [ ] Build with `cargo apk2 build --release -p notae-android --target aarch64-linux-android`; check `target/release/apk/notae-android.apk`; validate package and arm64 ABI; read APK `versionName` with SDK `aapt dump badging` and fail unless it exactly equals `${GITHUB_REF_NAME#v}` on tag pushes (manual dry runs compare with the workspace package version); verify signer with SDK `apksigner`; generate `sha256sum`; upload the APK/checksum as a short-retention Actions artifact.
- [ ] Before cargo-apk2, validate the keystore without emitting credentials: `keytool -list -v` must show exactly one `PrivateKeyEntry`, then `keytool -importkeystore` must open it using environment-backed store and key passwords into a disposable validation store under `$RUNNER_TEMP`; remove validation files in `always()` cleanup. This verifies the same password can unlock the private key and prevents cargo-apk2's password-bearing error path from handling a malformed key.
- [ ] Add a separate `publish-release` job with `needs: [android-release-build]`, `contents: write`, and a tag-push-only condition. Download the build artifact and run `gh release create "$GITHUB_REF_NAME" --verify-tag --generate-notes` with the APK and checksum assets, `GH_TOKEN: ${{ github.token }}`, and `GH_REPO: ${{ github.repository }}`; do not check out repository content in this publisher job. Manual dry runs must never enter this job; CI must never create tags.
- [ ] Use a local disposable test keystore under ignored `target/` to exercise local signing, then use the GitHub `workflow_dispatch` dry-run after the workflow is on the default branch to exercise the hosted signed build; verify no GitHub Release is created. Document the GitHub secret names, single-key/password constraints, tag/version match, dry-run, download behavior, and local release-build env vars in `docs/android-release.md`; update the old release-build instructions in `docs/android-debugging.md`.
- [ ] Document the one-time signing migration: existing installs were signed with the development debug key and cannot be upgrade-installed over with the new dedicated release key. The first release-key install requires uninstall/reinstall and loses app-private preferences/grants/index; never uninstall or overwrite the user's existing tablet app/data as part of this plan. Validate signing/install behavior only on the clean scratch AVD. Continue using the debug key for local debug installs; use the dedicated release key for local release APKs and future release-to-release upgrades.

### Task 11: Final qualification, review, and merge

**Files:** `docs/engineering-standards.md`, `docs/android-debugging.md`, `docs/android-release.md`, `docs/superpowers/IMPLEMENTATION-STATE.md`, the existing SDD ledger, and any issue-specific docs from prior tasks.

**Consumes:** All completed product tasks and CI/release jobs. **Produces:** Verified local branch merged to `main`, with hardware gates explicitly retained where unavailable.

- [ ] Run every documented host quality command, x86_64 emulator build/interaction smoke, arm64 Android check and local signed release build. Run the complete local emulator checklist from the existing issue plan, including folder selection, all screen flows, keyboard/voice availability, light/dark, orientation, operations, Back navigation, and fresh-key upgrade behavior. Save evidence without private data.
- [ ] Confirm workflows have not weakened existing quality gates, no signing secrets/keystores are tracked, temporary key files are removed, PR/branch jobs use only disposable keys, release permissions are minimal, version/tag mismatch fails, and publishing cannot run on branch/PR/manual dry-run events.
- [ ] Run independent code review on the complete diff; address all blocking findings. Verify every signed commit with GPG, using `pinentry-qt` when a passphrase prompt is requested.
- [ ] Push the verified feature branch and observe the GitHub CI run to completion before merging. Diagnose failures and rerun until host, arm64, and emulator gates pass. Record the successful Actions run URL on issue #21 and close it only after its acceptance is met.
- [ ] Keep Samsung-only keyboard and physical TalkBack acceptance open if not observed; do not close those issues or claim a complete v0.2.0 release on emulator evidence alone.
- [ ] Merge the reviewed feature branch into local `main` as requested, push the merged `main`, and observe its CI run. Do not push a version tag or publish a release without the required keystore secrets and a separately approved release version.
- [ ] After the workflow is on the default branch, trigger `workflow_dispatch` with `dry_run=true` and observe the hosted release build; if secrets are not configured or the run fails, report the exact blocker or fix on the feature branch, repeat its CI gates, and merge the correction. Confirm the run created no GitHub Release.


## Release signing secret setup

Before the first release tag, the maintainer must add two repository Actions secrets without sharing their values in chat:

- `NOTAE_RELEASE_KEYSTORE_BASE64`: base64 encoding of the dedicated, long-lived single-private-key release keystore.
- `NOTAE_RELEASE_KEYSTORE_PASSWORD`: its keystore password; the private key uses the same password because cargo-apk2's signer has no separate key-password/alias configuration.

The repository is currently private; GitHub Release assets will be downloadable only to accounts with repository access unless repository visibility is independently changed.

## Execution order

The issue-specific task sequence is intentionally serialized because Tasks 1–8 share UI files and interfaces. Task 9 starts after Task 4's verified Android accessibility exposure and all product/UI tasks through Task 8, because its CI interaction depends on the accessible app paths; Task 10 follows Task 9 because its release builder depends on all three quality gates. Final qualification, branch CI, review, and merge are last. The real-key hosted dry-run runs after the workflow reaches the default branch. No task closes an issue without the acceptance evidence described by its source plan.
