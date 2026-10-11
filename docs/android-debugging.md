# Debugging Notae on Android

This guide covers the Rust/Slint Android app (`org.notae.android`) built with `cargo-apk2`, targeting the Samsung Galaxy Tab S10 FE+ (`SM_X626B`, ADB serial `<tablet-serial>`). The original report was that Notae stopped after choosing a folder.

The original failure was a JNI `UnsatisfiedLinkError` when Android returned from the SAF picker. `NotaeActivity` now loads `notae_android` statically; the API 35 emulator returned from the picker without a crash. That exposed a second bug: the SAF cursor lookup used the wrong display-name column. Android's `DocumentsContract.Document.COLUMN_DISPLAY_NAME` is `_display_name`; after correcting it, selecting the emulator's `Audiobooks` folder opened Notae's library screen. The emulator crash buffer was empty. This verifies the emulator path, not yet the physical Samsung tablet.

The UI uses custom touch-first Material controls rather than Slint's stock buttons and LineEdit. Android 12+ supplies wallpaper-derived dark color roles; older releases use Notae's lavender baseline. Wide windows show a navigation rail and score/composer/status columns; compact windows use a single-column score list. Search matches token prefixes (for example, `Allel` returns `Alleluja` titles), and its trailing clear action resets the query. Metadata and desktop import reviews overlay the library in modal sheets.

The launcher icon combines a folded score page with a musical N. `android/res` contains legacy density PNGs, API-26 adaptive resources, and an API-33 monochrome themed icon. Cargo's Android resource metadata, not the reference manifest alone, packages them.

Controls animate state layers over 120–180 ms, sheets enter over 240 ms, and scan progress animates over 200 ms. **Settings → Reduce animations** disables this motion for the session. Android's disabled animator setting is read at launch. Android wallpaper colors are also read at launch; restart the app after changing the wallpaper.

SAF readiness also synchronizes the controller's Library screen. Previously, selecting a folder opened the visible library but left the controller in Setup, so Settings was rejected. A host regression verifies that selecting ready storage enables Settings without starting a scan.

## Protect the ADB session

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
   adb -s <tablet-serial> logcat -b crash -v threadtime
   ```

3. Choose the folder in Notae while the capture is running. Record the approximate time and whether the folder was internal storage, an SD card, or another document provider.
4. Stop the capture with Ctrl+C and save/share the output around the failure.
5. If the crash buffer has no relevant entry, use Android Studio's **Logcat** window with the same device selected and reproduce once. If needed, a live all-buffer capture is:

   ```sh
   adb -s <tablet-serial> logcat -b all -v threadtime
   ```

   This is noisier; stop it immediately after the failure and preserve the lines just before and after the stop.

The `crash` buffer contains crash records; `threadtime` adds timestamps, process IDs, and thread IDs. A Java failure usually includes `FATAL EXCEPTION` and a Java stack trace. A native failure usually includes `Fatal signal`, `SIGSEGV`/another signal, or a tombstone/backtrace. Preserve the complete relevant block, not only the final line.

If ADB is responsive after the failure, Android 11+ may expose the app's recent process exit reason with this read-only query:

```sh
adb -s <tablet-serial> shell dumpsys activity exit-info org.notae.android
```

If it hangs or the service rejects the command, do not reset ADB; use Android Studio Logcat on the connected device or capture a bug report from the tablet's Developer options. Avoid sharing unrelated logs containing personal identifiers, document names, or `content://` URIs.

## Rust/native follow-up

Notae's UI, SAF flow, and storage state run in a Rust shared library called by the Android activity/JNI adapter. The crash evidence determines which layer to inspect:

- `FATAL EXCEPTION` / Java stack: follow the Java or JNI exception path and include the full cause chain.
- `Fatal signal` / tombstone: retain the exact APK and matching unstripped `.so`/debug symbols from that build. Use Android NDK `ndk-stack` to symbolize a native tombstone; symbols from another build are not reliable.
- A normal process exit without a crash trace: include the `dumpsys activity exit-info` reason and check the activity lifecycle/termination logs before changing code.

`cargo-apk2` supports `build`, `run` (build/install/launch), and `gdb` (device debugging with symbols). Building this Java-backed Android app requires `ANDROID_NDK_ROOT` and a full JDK with `javac` and `jar`. This host has JDK 25 at `/usr/lib/jvm/java-25-openjdk`. The bundled D8 rejects Java 25 class files, so compile the Java sources as Java 17:

```sh
ANDROID_NDK_ROOT=$HOME/Android/Sdk/ndk/android-ndk-r26d \
JAVA_HOME=/usr/lib/jvm/java-25-openjdk \
JDK_JAVAC_OPTIONS="-source 17 -target 17" \
JAVA_SOURCE_VERSION=17 \
JAVA_TARGET_VERSION=17 \
cargo apk2 build --release -p notae-android
```

Build artifacts and runtime behavior must come from the same source revision. Capture original failure logs before reinstalling or using `cargo apk2 run`, which can replace the state needed to reproduce the problem. For a native crash, retain the exact APK and matching unstripped `.so`/debug symbols; use `cargo apk2 gdb -p notae-android` only after saving evidence and confirming the device transport is responsive.

## Emulator setup

The API 35 Google APIs x86_64 system image and `notae-api35-tablet` AVD are installed under `$HOME/Android/Sdk`; the AVD uses a 2560×1600 tablet display and KVM acceleration. SDK licenses have been accepted. The current emulator is `emulator-5554`. To start the existing AVD:

```sh
export ANDROID_SDK_ROOT=$HOME/Android/Sdk
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

Picker smoke check: tap **Choose library folder**, choose `Audiobooks` in the system picker, tap **Use this folder**, then **Allow**. Expected: Notae opens the library screen; an empty folder shows **A little space for your scores** with a **Scan library** action. The crash buffer should stay empty.

Redesign smoke checks: enter a title prefix, clear search, tap a score to open its PDF, use its pencil action to edit details, save/cancel, and navigate between Library and Settings. While a metadata/import sheet is open, Tab must stay in its controls and Escape must dismiss it without activating the library behind it. Check narrow windows as well as tablet landscape, including the on-screen keyboard.

Build separately with `--target x86_64-linux-android` for the emulator and `--target aarch64-linux-android` for the Samsung tablet. Both builds overwrite `target/release/apk/notae-android.apk`; keep a separate copy before building the other architecture.

The redesign was exercised on the API 35 emulator with 70 indexed scores: `Allel` filtered to seven results, clearing restored the library, a whole-row tap opened the PDF viewer, metadata saved in tablet and compact layouts, and Settings navigation worked. Keyboard focus stayed in the metadata sheet and Escape dismissed it; the scan completed with **No changes**. The Android host suite passed 71 tests. The ARM64 release APK was installed and launched on the Samsung tablet, preserving app data. Further physical-device, TalkBack, import-review, and display-class transition verification was deferred at the user's request; emulator keyboard checks used its floating keyboard.

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
