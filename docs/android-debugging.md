# Debugging Notae on Android

This guide covers the Rust/Slint Android app (`org.notae.android`) built with `cargo-apk2`, targeting the Samsung Galaxy Tab S10 FE+ (`SM_X626B`, ADB serial `R5GL61JXE8F`). The original report was that Notae stopped after choosing a folder.

The original failure was a JNI `UnsatisfiedLinkError` when Android returned from the SAF picker. `NotaeActivity` now loads `notae_android` statically; the API 35 emulator returned from the picker without a crash. That exposed a second bug: the SAF cursor lookup used the wrong display-name column. Android's `DocumentsContract.Document.COLUMN_DISPLAY_NAME` is `_display_name`; after correcting it, selecting the emulator's `Audiobooks` folder opened Notae's library screen. The emulator crash buffer was empty. This verifies the emulator path, not yet the physical Samsung tablet.

The UI follows a fixed One UI palette (not wallpaper-derived): background is `#F3F3F5` / `#000000`, cards `#FFFFFF` / `#17171A`, fields `#EBEBEF` / `#26262B`, and accent `#2F6FEB` / `#5B9BFF` (light / dark). Wide layouts show library sections and score cards; compact layouts adapt the library and details. Search opens from its icon or Ctrl K, with scope chips and lyric-match snippets. Metadata and import reviews use modal sheets. The earlier emulator and tablet observations recorded below relate to the previous UI, not this redesign; they are not visual verification of the new design.

The launcher icon is the blue folded score page with a music note (source `apps/notae-android/android/icon-source.png`): a light-blue (`#EAF1FF`) adaptive background, a transparent foreground PNG sized for the safe zone, a note-and-sheet monochrome mask for themed icons, and the same art as the in-app logo (`ui/assets/notae.png`). `android/res` contains legacy density PNGs, API-26 adaptive resources, and an API-33 monochrome themed icon. Cargo's Android resource metadata, not the reference manifest alone, packages them.

The redesigned UI has no animated transitions or state effects. Controls respond immediately to input, modal sheets open and close immediately, and progress changes only when worker events report new progress.

## Window insets and the "app offset downward" report

Two causes were found by reading the code, not by running the tablet (the tablet's adb shell was unresponsive, so neither fix has been seen on a device):

1. `app.slint` placed a 56 px Scan row (plus 20 px spacing) above every configured screen. That pushed the Library header, side panel, search field and settings title down by 76 px and let Search and Settings, which size themselves to the safe viewport, overrun the bottom. Each screen now owns its header; the Scan icon is part of it (`ScanButton` in `components.slint`) and `tests/details_ui.rs` fails if a scan row returns to `app.slint`.
2. Slint's Android backend always reports the window manager's system-bar and display-cutout insets as `safe-area-insets` and never makes the window edge-to-edge itself, while `NotaeActivity` ran with the default framework theme (action bar, decor fitting system windows). The UI was therefore inset by the status and navigation bars twice. `NotaeActivity.onCreate` now removes the title/action bar, calls `setDecorFitsSystemWindows(false)` (API 30+, legacy layout flags before), makes the bars transparent and chooses light or dark bar icons from the night mode. `app.slint` pads the layout by `safe-area-insets`, and `window-width`/`available-height` come from the window, never from child layouts (deriving them from children caused Slint binding loops).

Check on the tablet in landscape and portrait: the title starts just below the status bar with no empty band, the bottom row ends above the gesture bar, nothing is clipped, and system-bar icons are legible in light and dark.

## Implemented screen layout

- **Breakpoints** (window width minus side insets): `< 840 px` compact (chip row instead of side panel, Settings icon in the Library header, details take the full width); `>= 840 px` side panel; `>= 1280 px` list plus details side by side. Score details stack the preview above the title below 560 px of pane width and scroll vertically.
- **Headers:** Library: title and count left; Search, Settings (compact only), Scan right. Search: Back, search field, Scan. Settings: Back, title, Scan. The Scan icon is always the right-most action.
- **Status banner:** running operation or its outcome (Cancel while scanning, Dismiss after an outcome; a failure uses the error colour and warning icon). Idle shows nothing.
- **Empty states:** Library (per section), Search with no query, and Search with no results each explain what to do. Text on neutral surfaces that is accent-coloured uses `Theme.accent-text` (4.5:1 or better in both themes).

## Follow-up UI verification

The following checks cover the post-install usability fixes. They remain unverified in this run; installing an APK on the connected tablet is still deferred pending explicit user approval.

- Start a scan, switch among Library, Search, and Settings, and confirm the same phase, progress, and outcome banner remains visible until dismissed. An idle library shows no banner. The latest scan result and pending-text count live in Settings > Library maintenance; confirm pending text is described as an index state, not Android OCR running.
- Confirm the Scan icon is the right-most header action on Library (after Search, and after Settings in compact layouts), on Search (right of the search field), and on Settings (right of the title); each starts the existing scan, and it dims while another operation is active. There is no separate bar above the screens.
- In Settings, verify the selected folder name and Change folder action, latest available scan result/conflicts/failures, pending-text count, local-only storage copy, and app version.
- Open Search with the on-screen keyboard. Confirm scope chips scroll horizontally and results use the remaining viewport and scroll independently.
- Open score details and verify the icon-only Edit details action has the accessible name “Edit details” and opens the editor.
- Confirm screen changes, state layers, modal sheets, and progress have no animations.


SAF readiness also synchronizes the controller's Library screen. Previously, selecting a folder opened the visible library but left the controller in Setup, so Settings was rejected. A host regression verifies that selecting ready storage enables Settings without starting a scan.

Emulator checks (2560x1600 AVD, `-gpu host`; `swiftshader_indirect` segfaults here) confirmed: saved folder restored after force-stop and emulator reboot, edge-to-edge layout with no offset, Scan in every header, Settings/Search scroll and stretch, Android Back closes Settings, Search and details instead of leaving the app, corrupt thumbnails self-heal on the next scan, and dark and compact (1000x1600) layouts.

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

The API 35 Google APIs x86_64 system image and `notae-api35-tablet` AVD are installed under `/home/mike/Android/Sdk`; the AVD uses a 2560×1600 tablet display and KVM acceleration. SDK licenses have been accepted. The current emulator is `emulator-5554`. To start the existing AVD:

```sh
export ANDROID_SDK_ROOT=/home/mike/Android/Sdk
export PATH="$ANDROID_SDK_ROOT/emulator:$ANDROID_SDK_ROOT/platform-tools:$PATH"
emulator -avd notae-api35-tablet
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

Prior debugging notes: the following emulator/tablet observations predate the current One UI redesign and must not be taken as its verification. They record earlier picker, search, metadata, and Settings behavior only. No current redesign emulator or tablet visual checks have been performed; both remain deferred pending user go-ahead.

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
