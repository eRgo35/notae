# Debugging Notae on Android

This guide covers the Rust/Slint Android app (`org.notae.android`) built with `cargo-apk2`, targeting the Samsung Galaxy Tab S10 FE+ (`SM_X626B`, ADB serial `R5GL61JXE8F`). The original report was that Notae stopped after choosing a folder.

The original failure was a JNI `UnsatisfiedLinkError` when Android returned from the SAF picker. `NotaeActivity` now loads `notae_android` statically; the API 35 emulator returned from the picker without a crash. That exposed a second bug: the SAF cursor lookup used the wrong display-name column. Android's `DocumentsContract.Document.COLUMN_DISPLAY_NAME` is `_display_name`; after correcting it, selecting the emulator's `Audiobooks` folder opened Notae's library screen. The emulator crash buffer was empty. This verifies the emulator path, not yet the physical Samsung tablet.

The UI follows a fixed One UI palette (not wallpaper-derived): background is `#F3F3F5` / `#000000`, cards `#FFFFFF` / `#17171A`, fields `#EBEBEF` / `#26262B`, and accent `#2F6FEB` / `#5B9BFF` (light / dark). Wide layouts show library sections and score cards; compact layouts adapt the library and details. Search opens from its icon or Ctrl K, with scope chips, lyric-match snippets, a contextual Ctrl K keycap for attached hardware keyboards, and an optional system voice-recognition action. Search phrases are returned through the same local query path; voice input hands off to Android's installed recognizer, which may use network services according to that app's behavior. Notae itself adds no network access or microphone permission. Metadata and import reviews use modal sheets.

The launcher icon is the blue folded score page with a music note (source `apps/notae-android/android/icon-source.png`). Its adaptive background is deep blue `#153976`, not the palette accent `#2F6FEB` the plan first proposed: the folded corner and exposed back sheet are blue foreground surfaces too, and their worst high-coverage pixel `#4B8DF9` is only 1.41:1 against the accent but 3.45:1 against `#153976`. The page fill `#FCFCFC` is 10.90:1 against `#153976` (pure white 11.18:1; 4.57:1 against the accent); the old `#EAF1FF` badge gave white 1.13:1. The 648×648 transparent foreground (art bbox 192–455 × 176–470, farthest pixel 170 px from centre, inside the 198 px safe-zone circle) is unchanged except for one alpha-2 pixel normalised to the badge blue. The API-33 monochrome mask (`ic_launcher_monochrome.png`, 648×648, a single RGB value, no pixel outside the foreground silhouette) is a separate asset and was not recoloured. The legacy `mdpi`/`hdpi`/`xhdpi`/`xxhdpi`/`xxxhdpi` icons (48/72/96/144/192 px, RGBA) are the foreground composited over `#153976` and area-filter resized, keeping the previous icons' rounded-corner alpha mask for API 23–25 launchers; each matches a fresh composite pixel-for-pixel. Cargo's Android resource metadata, not the reference manifest alone, packages them; the in-app logo is `ui/assets/notae.png`.

Contrast was measured from the PNG pixels with WCAG relative luminance (`c/12.92` for `c≤0.04045`, else `((c+0.055)/1.055)^2.4`; `L=0.2126R+0.7152G+0.0722B`; ratio `(Lmax+0.05)/(Lmin+0.05)`, unrounded). The silhouette band is every alpha≥200 pixel within 3 px of the transparent exterior. Against `#153976` its minima are 9.107:1 for the page (`#DDE9FA` at 392,441), 3.628:1 for the folded corner (`#4E92FA`, 386,178) and 3.448:1 for the back sheet (`#4B8DF9`, 209,338). Compositing opaque-edge pixels (alpha≥250) over the badge gives a 3.38:1 worst case (`#468DFD`, 239,212). A literal 3:1 for every antialiased pixel is not achievable on any background, because a pixel at coverage `a` tends to the badge colour (1:1) as `a→0`; the 3:1 claim therefore covers the solid page, fold and back-sheet regions, with partially covered edge pixels judged only in composited (never unpremultiplied) colour. This is source-pixel evidence: the physical launcher (One UI, light/dark, themed icon) has not yet been inspected, and the earlier tablet screenshot of the old icon was truncated and proves nothing about contrast.

The UI now uses shared 120ms and 220ms transitions for screens, sheets and state effects; Android animator scale zero disables motion. `motion_ui` covers intermediate/final states and zero-scale behavior, and the API 35 AVD was smoke-tested for Search entry with scale 1 and 0. Tablet visual acceptance in both themes, on-device sheet/section transitions and dropped-frame review remain pending.

## Window insets and the "app offset downward" report

Two causes were found by reading the code, not by running the tablet (the tablet's adb shell was unresponsive, so neither fix has been seen on a device):

1. The former app-level Scan row pushed configured screens down and let Search and Settings overrun the safe viewport. Each configured screen owns its own header; the maintenance Scan action belongs in Settings body rather than the Settings header. `app.slint` pads the layout by `safe-area-insets`, and `window-width`/`available-height` come from the window, never from child layouts (deriving them from children caused Slint binding loops).
2. Slint's Android backend always reports the window manager's system-bar and display-cutout insets as `safe-area-insets` and never makes the window edge-to-edge itself, while `NotaeActivity` ran with the default framework theme (action bar, decor fitting system windows). The UI was therefore inset by the status and navigation bars twice. `NotaeActivity.onCreate` now removes the title/action bar, calls `setDecorFitsSystemWindows(false)` (API 30+, legacy layout flags before), makes the bars transparent and chooses light or dark bar icons from the night mode. `app.slint` pads the layout by `safe-area-insets`, and `window-width`/`available-height` come from the window, never from child layouts (deriving them from children caused Slint binding loops).

Check on the tablet in landscape and portrait: the title starts just below the status bar with no empty band, the bottom row ends above the gesture bar, nothing is clipped, and system-bar icons are legible in light and dark.

## Implemented screen layout

- **Breakpoints** (window width minus side insets): `< 840 px` compact (chip row instead of side panel, Settings icon in the Library header, details take the full width); `>= 840 px` side panel; `>= 1280 px` list plus details side by side. Score details stack the preview above the title below 560 px of pane width and scroll vertically.
* **Headers:** Library: title and count left; Search, Settings (compact only), Scan right. Search: Back and search field, with a microphone action only when Android resolves a system speech recognizer. Settings: Back and title only; its maintenance Scan action is in the screen body. Search has no Scan header action.
* **Status banner:** running operation or its outcome on every configured screen (Cancel while scanning, Dismiss after an outcome; a failure uses the error colour and warning icon). Idle shows nothing.
* **Empty states:** Library (per section), Search with no query, and Search with no results each explain what to do. Text on neutral surfaces that is accent-coloured uses `Theme.accent-text` (4.5:1 or better in both themes).

## Follow-up UI verification

- Generated-UI behavior checks use `tests/support/mod.rs` and `ElementHandle`; they verify Search has no Scan action, Library and Settings maintenance Scan placement, and the contextual Ctrl K keycap. `search_keycap_tracks_typed_text_and_keyboard_presence_changes` sends pointer/key events to the SearchField, verifies typed text reaches the app query callback, the hint disappears, clearing restores it, and simulated keyboard presence detach/attach updates visibility. The headless window is laid out at compact and expanded widths; physical tablet rendering remains pending.
- The typed-keyboard regression first failed because SearchField's inner TextInput did not forward `edited`, leaving `AppWindow.search_text` empty after actual key events despite visible local text. Adding the existing TextField-style `edited` callback fixed query propagation. The test also checks the empty-state copy omits the old Ctrl K sentence, the keycap and clear action stay within SearchField, and keycap/clear are mutually exclusive. Focused test: `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo test -p notae-android --test headers_ui search_keycap_tracks_typed_text_and_keyboard_presence_changes` (1 passed); `headers_ui` plus `search_state` passed 24 tests.
- On the scratch API 35 x86_64 AVD, Android reports keyboard availability and the empty Search field shows Ctrl K (`target/emulator-evidence/task-3/search-empty.png`). Gboard touch input entered “Hex”; the hint disappeared, Clear search appeared, and the fixture result updated (`search-typed.png`). Clearing restored the hint (`search-cleared.png`). This is emulator-only evidence; no Galaxy Tab or keyboard cover was interacted with, and physical attach/detach and Samsung keyboard acceptance remain open.
- Welcome and recovery keep the intro, feature card and action in a scrollable viewport; tall-window top padding is capped. Settings uses 16 px card padding, keeps Change beside the wrapping folder name, reports the indexed count and actual scan counts, and says **No scan this session** without a scan summary. These dimensions and device colors still need visual confirmation on the Samsung tablet.
- About identifies the package version, Git build identity (or `unknown` in source archives), MIT license, repository URL and `notae_index::sidecar::SIDECAR_SCHEMA_VERSION`. It has one local-only privacy statement; indexed count appears only in maintenance.
- The Galaxy Tab S10 FE+ (`R5GL61JXE8F`) is connected but intentionally untouched because it contains user library data and no approval to alter it. No physical-device visual acceptance is claimed; those checks remain pending.
- Search's Android voice action is shown only when `resolveActivity` finds a system recognizer. It sends free-form recognition in the device's current locale and returns the first nonblank phrase through the existing query callback. No `RECORD_AUDIO` permission is requested; the installed recognizer may use network services independently of Notae.
- On an emulator or tablet, exercise recognizer present/absent, successful Unicode phrase, cancellation, keyboard attach/detach while Search is open, query clearing, and folder picking. Confirm no mic permission prompt appears. These device checks are pending until hardware/emulator is available.
- Open Search with the on-screen keyboard. Confirm scope chips scroll horizontally and results use the remaining viewport and scroll independently.
- Open score details and verify the icon-only Edit details action has the accessible name “Edit details” and opens the editor.
- Confirm screen, sheet, state-layer and progress transitions use shared motion durations; Android animator scale zero disables motion. Host transition regressions and API 35 AVD Search entry were exercised. Tablet and dark-theme visual checks remain pending.

### Task 1: startup and input feedback (#1, #2, #3, #13)

- Issue #1 startup is implemented locally in commit `7c8cd0e` (`Refs #1`). The disposable `notae-test-api35` AVD (`emulator-5554`, Android API 35, x86_64, 2560×1600 at 320 dpi / 1280×800 logical) cold-start recording and frame capture are `target/emulator-evidence/task-1/cold-start.mp4` and `cold-start.png`. The frame shows one non-sensitive fixture score in Library after the Android splash, with no Welcome/empty-Library frame. `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo test -p notae-android --test startup_state --test headers_ui --test storage_state --test smoke` passed 33 tests. Both x86_64 and arm64 APK builds succeeded; the generated UI regression asserts loading hides Welcome before readiness and no-saved-root shows Welcome when ready.
- Issue #3's earlier host tests (`changing_search_scope_keeps_following_chip_bounds_stable`, `search_scope_chips_scroll_only_when_the_row_overflows`) only changed selection directly and scrolled programmatically; they did not measure pointer-press geometry. Added `search_scope_chip_bounds_stay_fixed_during_pointer_press` and a separate three-logical-pixel-drift regression. They dispatch pointer events, assert exact x/y/width/height for all five chips, row bounds and the inferred leading viewport offset, and confirm Titles becomes selected. Both pass with `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo test -p notae-android --test headers_ui search_scope_chip_bounds` (2 passed). The source audit found fixed Chip dimensions and `interactive: content-width > width`; no production correction was warranted. On the disposable API 35 x86_64 AVD, a Search All→Titles tap left the settled chip row visually in place; before/after frames are `target/emulator-evidence/task-1/search-scope-before.png` and `search-scope-titles.png`. These are released-state screenshots, not press-frame or drag evidence. Overflow-row and compact-Library touch geometry, deliberate drag, and all physical press/drift/drag checks remain open. The connected Galaxy Tab S10 FE+ was intentionally not interacted with; #3 remains open.
- Issue #2 is reproduced on the scratch AVD. Initial SearchField geometry was y=48/h=640 logical px and produced cursor rect y=358.625 (physical y≈717); once field geometry was y=48/h=52, a moved caret produced y=64.625 (physical y≈129), and the native handle followed. Instrumented cold Library→Search event order is in `target/emulator-evidence/task-1/focus-events.log`: request h640/available-height744 while transition unfinished; field becomes h52 at the same available height; transition finishes and issues exactly one focus; focus becomes true; keyboard inset changes follow. This selects stale pre-layout geometry, not backend coordinate transform. The fix uses a one-shot after-layout focus gated by `field.height == field.preferred-height` and transition completion; correction refocus branches were removed. Close-before-transition and background/resume traces show no refocus after cancellation. Clean-build AVD screenshot `target/emulator-evidence/task-1/first-focus-after-fix-settled.png` shows the handle at the caret after one Search tap, with no second tap. A rendered host attempt using MinimalSoftwareWindow still left query text unchanged after simulated key events, so that unsuccessful harness test was removed; it is not evidence for or against Android behavior. Diagnostics are removed from source. Samsung hardware acceptance remains open.
- The Galaxy Tab S10 FE+ (`R5GL61JXE8F`) is currently connected but was intentionally not interacted with because it contains user library data and no approval to alter it. Samsung Keyboard, hardware Polish layout, composition/long-press alternates and physical caret placement were not tested. Keep #2 and #13 acceptance open; emulator Gboard and `adb shell input text` are not Samsung IME proof. No physical interaction or app-data changes were made.

### Task 3: voice search (#5)

- The request-code regression reproduced a real SAF collision: the 20,047th picker allocation returned the same `0x4e4f` code reserved by Java voice routing. `PickerState::next_request_code` now skips that reservation; Java continues to route voice before the generic SAF URI path. The focused picker regression failed before the fix at `left: 20047, right: 20047`.
- Slint 1.18.1 documents `Weak::upgrade` as valid only on the component-creating thread. Since Android delivers the Java activity result on its main looper, the native callback now schedules through `Weak::upgrade_in_event_loop` before touching `AppWindow`. Host tests exercise query preservation for cancellation, empty/blank phrases, and unrelated request codes; a Unicode result updates the query and invokes search exactly once. Separate event-loop and foreign-thread regressions cover a destroyed window and the thread-affinity constraint.
- `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo test -p notae-android --lib --test voice_search --test headers_ui --test search_state` passed 38 tests across four suites. `ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d JAVA_HOME=/usr/lib/jvm/java-25-openjdk JDK_JAVAC_OPTIONS='-source 17 -target 17' JAVA_SOURCE_VERSION=17 JAVA_TARGET_VERSION=17 PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo apk2 check -p notae-android --target aarch64-linux-android` completed successfully. The x86_64 APK built and installed on the scratch AVD.
- On the scratch API 35 AVD (`emulator-5554`), package resolution found `com.google.android.tts/com.google.android.apps.speech.tts.googletts.service.GoogleTTSActivity`. The installed Search screenshot `target/emulator-evidence/task-5/search-voice-visible.png` shows the microphone when the recognizer is available. The recognizer was not launched, so no spoken-result, cancellation, external-service, or runtime permission-prompt result is claimed. Notae declares no `RECORD_AUDIO` permission.
- Issue #5's code and host/emulator-availability evidence are local and unpushed. No physical Samsung device was touched; end-to-end spoken phrase/cancellation and physical acceptance remain open.


SAF readiness also synchronizes the controller's Library screen. Previously, selecting a folder opened the visible library but left the controller in Setup, so Settings was rejected. A host regression verifies that selecting ready storage enables Settings without starting a scan.

Earlier emulator checks (2560x1600 AVD, `-gpu host`; `swiftshader_indirect` segfaults here) covered saved-folder restoration, edge-to-edge layout, Settings/Search scroll and stretch, Android Back, thumbnails, and dark and compact layouts. Those historical checks do not verify Task 2's Settings redesign or Task 3's voice/hardware-keyboard paths; current Task 3 evidence is recorded above and in the follow-up verification notes.

## Protect the ADB session

- The host has two ADB binaries: `/usr/bin/adb` (37.0.0) and the SDK binary at `/home/mike/Android/Sdk/platform-tools/adb` (37.0.1). Use the SDK binary consistently for every command so troubleshooting does not alternate client versions. In the debugging terminal, put the SDK tools first:

  ```sh
  export PATH="/home/mike/Android/Sdk/platform-tools:$PATH"
  adb version
  ```

- Do not run `adb kill-server`, `adb start-server`, `adb disconnect`, `adb reboot`, or USB-reset commands as a debugging shortcut.
- Do not clear log buffers (`adb logcat -c`) before collecting the failure.
- Run one ADB command at a time. If a command hangs, stop there; do not queue more ADB work or repeatedly retry it. Use Android Studio's Logcat window on the already connected device instead.
- `adb devices -l` only confirms that ADB lists the device; it does not prove that device shell/logcat requests are responsive.
- Starting a live `adb logcat` stream is read-only. Stop the host-side stream with Ctrl+C after reproducing; this does not restart ADB or alter the app/device.
- If both the tablet and emulator are attached, pass the exact serial with `adb -s <serial>` for every device command; do not rely on implicit device selection.

## Capture the folder-selection failure

1. Connect the tablet as usual and leave the existing ADB server/device connection alone.
2. In a terminal, start a focused live crash-buffer capture:

   ```sh
   adb -s R5GL61JXE8F logcat -b crash -v threadtime
   ```

3. Choose the folder in Notae while the capture is running. Record the approximate time and whether the folder was internal storage, an SD card, or another document provider.
4. Stop the capture with Ctrl+C and save/share the output around the failure.
5. If the crash buffer has no relevant entry, use Android Studio's **Logcat** window with the same device selected and reproduce once. If needed, a live all-buffer capture is:

   ```sh
   adb -s R5GL61JXE8F logcat -b all -v threadtime
   ```

   This is noisier; stop it immediately after the failure and preserve the lines just before and after the stop.

The `crash` buffer contains crash records; `threadtime` adds timestamps, process IDs, and thread IDs. A Java failure usually includes `FATAL EXCEPTION` and a Java stack trace. A native failure usually includes `Fatal signal`, `SIGSEGV`/another signal, or a tombstone/backtrace. Preserve the complete relevant block, not only the final line.

If ADB is responsive after the failure, Android 11+ may expose the app's recent process exit reason with this read-only query:

```sh
adb -s R5GL61JXE8F shell dumpsys activity exit-info org.notae.android
```

If it hangs or the service rejects the command, do not reset ADB; use Android Studio Logcat on the connected device or capture a bug report from the tablet's Developer options. Avoid sharing unrelated logs containing personal identifiers, document names, or `content://` URIs.

## Rust/native follow-up

Notae's UI, SAF flow, and storage state run in a Rust shared library called by the Android activity/JNI adapter. The crash evidence determines which layer to inspect:

- `FATAL EXCEPTION` / Java stack: follow the Java or JNI exception path and include the full cause chain.
- `Fatal signal` / tombstone: retain the exact APK and matching unstripped `.so`/debug symbols from that build. Use Android NDK `ndk-stack` to symbolize a native tombstone; symbols from another build are not reliable.
- A normal process exit without a crash trace: include the `dumpsys activity exit-info` reason and check the activity lifecycle/termination logs before changing code.

`cargo-apk2` supports `build`, `run` (build/install/launch), and `gdb` (device debugging with symbols). Building this Java-backed Android app requires `ANDROID_NDK_ROOT` and a full JDK with `javac` and `jar`. This host has JDK 25 at `/usr/lib/jvm/java-25-openjdk`. The bundled D8 rejects Java 25 class files, so compile the Java sources as Java 17:

```sh
ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d \
JAVA_HOME=/usr/lib/jvm/java-25-openjdk \
JDK_JAVAC_OPTIONS="-source 17 -target 17" \
JAVA_SOURCE_VERSION=17 \
JAVA_TARGET_VERSION=17 \
cargo apk2 build --release -p notae-android
```

`[package.metadata.android] build_targets` pins `aarch64-linux-android`, so `cargo apk2 build` never probes the device's ABI over ADB (the probe hung when the device shell was unresponsive). For the emulator pass `--target x86_64-linux-android`.

Build artifacts and runtime behavior must come from the same source revision. Capture original failure logs before reinstalling or using `cargo apk2 run`, which can replace the state needed to reproduce the problem. For a native crash, retain the exact APK and matching unstripped `.so`/debug symbols; use `cargo apk2 gdb -p notae-android` only after saving evidence and confirming the device transport is responsive.

## Emulator setup

The API 35 Google APIs x86_64 image is installed with two tablet AVDs: `notae-api35-tablet` contains an existing library with 70 scores and must be preserved; `notae-test-api35` is the disposable scratch AVD for product testing. The scratch AVD uses a 2560×1600 display and KVM acceleration; in this session it ran as `emulator-5554`. SDK licenses have been accepted. Start only the scratch AVD for app interaction:

```sh
export ANDROID_SDK_ROOT=/home/mike/Android/Sdk
export PATH="$ANDROID_SDK_ROOT/emulator:$ANDROID_SDK_ROOT/platform-tools:$PATH"
emulator -avd notae-test-api35
```

On a fresh SDK, install the command-line tools, review and accept licenses interactively, then install the image and create the AVD:

```sh
sdkmanager --licenses
sdkmanager "system-images;android-35;google_apis;x86_64"
avdmanager create avd -n notae-api35-tablet \
  -k "system-images;android-35;google_apis;x86_64" -d pixel_tablet
```

Install and launch using an explicit serial; do not reset ADB:

```sh
adb -s emulator-5554 install -r target/release/apk/notae-android.apk
adb -s emulator-5554 shell am start -n org.notae.android/.NotaeActivity
```

Picker smoke check: tap **Choose score folder**, choose `Audiobooks` in the system picker, tap **Use this folder**, then **Allow**. Expected: Notae opens the library screen; an empty folder shows **A little space for your scores** with a **Scan library** button, and the header's Scan icon does the same. The crash buffer should stay empty.

Redesign smoke checks: in light and dark (`adb shell cmd uimode night yes|no`) check the welcome screen, scan a library and wait for thumbnails, switch sections (All, Recent, Needs details, Text pending, a composer), tap a score to open its details, use **Open score** and confirm it appears under Recent, edit details, open search from the icon and with Ctrl K, try each scope chip, choose a result, and open Settings. Check split screen (< 840 px): chips replace the side panel and details take the full width. While a sheet is open, Tab stays inside it and Escape dismisses it.

Build separately with `--target x86_64-linux-android` for the emulator and `--target aarch64-linux-android` for the Samsung tablet. Both builds overwrite `target/release/apk/notae-android.apk`; keep a separate copy before building the other architecture.

## v0.2.0 Task 1 startup evidence (#1)

Captured 2026-10-17 on the disposable `notae-test-api35` AVD (`emulator-5554`): Android API 35, x86_64, 2560×1600 physical at 320 dpi (1280×800 logical). The installed development APK cold-started into a populated Library with one non-sensitive fixture row; the recording shows the Android splash followed by Library and no Welcome or empty-library frame. Evidence is `target/emulator-evidence/task-1/cold-start.mp4` (ignored). ADB crash-buffer inspection found no Notae crash; an unrelated Nexus Launcher entry is not attributed to Notae.

The generated UI regression keeps the loading state visible and Welcome hidden while a no-root restore is unresolved, then asserts that resolving to no saved root shows Welcome. The startup-state target separately covers a saved root through loading and empty first rows, no saved root, a revoked grant, a failed initial query, and a storage error.

Issue #2 is separately reproduced on the scratch AVD: Search's first focus observed the field at y=48 logical px with height 640; after the IME request and viewport resize it settled at y=48, height 52. The caret glyph appeared near physical y=150 while the blue cursor handle was around y=800. This establishes a focus-before-final-layout condition for the emulator; a diagnosed fix and physical Samsung Keyboard acceptance are still outstanding. Issue #13 has not been tested with Samsung Keyboard or a hardware Polish layout and remains a physical gate. No ADB text injection is used as IME acceptance evidence. No real Galaxy Tab data or app state was accessed.

Task 4A (#1) is implemented and smoke-tested in the local v0.2.0 worktree on 2026-10-17. Scratch AVD evidence and still-open device gates are recorded below; the other redesign flows have not yet been qualified.

Use the full JDK above for APK builds. The Java target/source settings are needed for the bundled D8 version; SDK manager can use the same JDK.


## What to attach for diagnosis

- Full crash/tombstone block, including a short lead-in and the timestamp.
- Exact reproduction steps and the kind of folder/provider selected.
- Device model and Android version; target is Galaxy Tab S10 FE+ (`SM_X626B`). Record whether the test ran on the physical tablet or the API 35 emulator.
- Whether the app entered the library, returned to setup, showed an error, or disappeared, and whether it happens on every selection.
- APK/build revision and whether it was a debug or release build.

Do not infer a root cause from the symptom alone. In particular, folder selection crosses Android's picker, Java activity-result callback, JNI conversion, Rust future wakeup, SAF tree reopening, and local index initialization; the trace should identify which boundary failed.

## References

- [Android logcat command-line tool](https://developer.android.com/tools/logcat)
- [Android `dumpsys` diagnostics](https://developer.android.com/tools/dumpsys)
- [Android `DocumentsContract.Document` API](https://developer.android.com/reference/android/provider/DocumentsContract.Document)
- [Material 3 search guidance](https://m3.material.io/components/search/guidelines)
- [Material 3 list specifications](https://m3.material.io/components/lists/specs)
- [Material 3 list accessibility](https://m3.material.io/components/lists/accessibility)
- [Android adaptive canonical layouts](https://developer.android.com/develop/adaptive-apps/guides/canonical-layouts)
- [Slint widget styles](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/style/)
- [SQLite FTS5 query syntax](https://www.sqlite.org/fts5.html)
- [Android NDK native crash debugging](https://developer.android.com/ndk/guides/debug)
- [cargo-apk2 documentation](https://docs.rs/crate/cargo-apk2/latest)
