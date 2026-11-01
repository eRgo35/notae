# Debugging Notae on Android

This guide covers the Jetpack Compose app (`com.c2yz.notae`) in `apps/notae-android`, built with Gradle, targeting the Samsung Galaxy Tab S10 FE+ (`SM_X626B`, ADB serial in `$TABLET_SERIAL`, from `adb devices -l`). The UI is Kotlin; the app logic is the Rust core (`libnotae_app_core.so`), which the app calls through UniFFI's generated bindings and JNA. The notes from debugging the 0.3.0 UI (window insets, the hardware-keyboard patch, the TalkBack bridge, issue evidence) are in this file's history, for example `git show v0.4.0:docs/android-debugging.md`.

The look is a fixed One UI palette, not wallpaper-derived; its values are `Tokens` in `app/src/main/kotlin/com/c2yz/notae/ui/theme/Tokens.kt`, checked by `TokensTest`. Screens, sheets and transitions use 120 ms and 220 ms animations, off when Android's animator scale is 0.

## Protect the ADB session

- The host has two ADB binaries: `/usr/bin/adb` (37.0.0) and the SDK binary at `$HOME/Android/Sdk/platform-tools/adb` (37.0.1). Use the SDK binary consistently for every command so troubleshooting does not alternate client versions. In the debugging terminal, put the SDK tools first:

  ```sh
  export PATH="$HOME/Android/Sdk/platform-tools:$PATH"
  adb version
  ```

- Do not run `adb kill-server`, `adb start-server`, `adb disconnect`, `adb reboot`, or USB-reset commands as a debugging shortcut.
- Do not clear log buffers (`adb logcat -c`) before collecting the failure.
- Run one ADB command at a time. If a command hangs, stop there; do not queue more ADB work or repeatedly retry it. Use Android Studio's Logcat window on the already connected device instead.
- `adb devices -l` only confirms that ADB lists the device; it does not prove that device shell/logcat requests are responsive.
- Starting a live `adb logcat` stream is read-only. Stop the host-side stream with Ctrl+C after reproducing; this does not restart ADB or alter the app/device.
- If both the tablet and emulator are attached, pass the exact serial with `adb -s <serial>` for every device command; do not rely on implicit device selection.

## Capture a failure

1. Connect the tablet as usual and leave the existing ADB server/device connection alone.
2. In a terminal, start a focused live crash-buffer capture:

   ```sh
   adb -s "$TABLET_SERIAL" logcat -b crash -v threadtime
   ```

3. Reproduce once while the capture is running. Record the approximate time and, for a folder problem, whether the folder was internal storage, an SD card, or another document provider.
4. Stop the capture with Ctrl+C and save/share the output around the failure.
5. If the crash buffer has no relevant entry, use Android Studio's **Logcat** window with the same device selected and reproduce once. If needed, a live all-buffer capture is:

   ```sh
   adb -s "$TABLET_SERIAL" logcat -b all -v threadtime
   ```

   This is noisier; stop it immediately after the failure and preserve the lines just before and after the stop. The app logs its own warnings (a folder picker or import that could not run, a failed search) with `Log.w`.

The `crash` buffer contains crash records; `threadtime` adds timestamps, process IDs, and thread IDs. A Kotlin failure usually includes `FATAL EXCEPTION` and a stack trace. A panic in the Rust core reaches Kotlin as an exception from the generated bindings. A native failure usually includes `Fatal signal`, `SIGSEGV`/another signal, or a tombstone/backtrace. Preserve the complete relevant block, not only the final line.

If ADB is responsive after the failure, Android 11+ may expose the app's recent process exit reason with this read-only query:

```sh
adb -s "$TABLET_SERIAL" shell dumpsys activity exit-info com.c2yz.notae
```

If it hangs or the service rejects the command, do not reset ADB; use Android Studio Logcat on the connected device or capture a bug report from the tablet's Developer options. Avoid sharing unrelated logs containing personal identifiers, document names, or `content://` URIs.

## Native crashes

- `FATAL EXCEPTION` / Kotlin stack: follow the cause chain; an exception thrown from `uniffi.notae_app_core` names the core call that failed.
- `Fatal signal` / tombstone: keep the exact APK and the matching unstripped library from the same build, `target/android/aarch64-linux-android/release/libnotae_app_core.so` (or `x86_64-linux-android` for the emulator), and symbolize with the NDK's `ndk-stack`. Symbols from another build are not reliable.
- A normal process exit without a crash trace: include the `dumpsys activity exit-info` reason and check the activity lifecycle logs before changing code.

Build artifacts and runtime behaviour must come from the same source revision. Capture the failure logs before reinstalling, which can replace the state needed to reproduce the problem.

## Emulator setup

The API 35 Google APIs x86_64 image is installed with two tablet AVDs: `notae-api35-tablet` contains an existing library with 70 scores and must be preserved; `notae-test-api35` is the disposable scratch AVD for product testing (2560×1600 at 320 dpi, KVM acceleration). Start only the scratch AVD for app interaction:

```sh
export ANDROID_HOME=$HOME/Android/Sdk
export PATH="$ANDROID_HOME/emulator:$ANDROID_HOME/platform-tools:$PATH"
emulator -avd notae-test-api35 -no-snapshot-save
```

On a fresh SDK, install the command-line tools, review and accept licenses interactively, then install the image and create the AVD:

```sh
sdkmanager --licenses
sdkmanager "system-images;android-35;google_apis;x86_64"
avdmanager create avd -n notae-test-api35 \
  -k "system-images;android-35;google_apis;x86_64" -d pixel_tablet
```

The debug build packages arm64-v8a and x86_64 (arm64 code aborts under the x86_64 emulator's translation); the release build is arm64 only. Build, install and launch with an explicit serial; do not reset ADB:

```sh
(cd apps/notae-android && ./gradlew assembleDebug)
adb -s emulator-5554 install -r apps/notae-android/app/build/outputs/apk/debug/app-debug.apk
adb -s emulator-5554 shell am start -n com.c2yz.notae/.MainActivity
```

## UI verification

- Every screen has Roborazzi screenshots, light and dark, compared with the baselines in `apps/notae-android/app/src/test/screenshots` by `./gradlew verifyRoborazziDebug`; a failed verify writes the differences under `app/build/outputs/roborazzi`. The Robolectric tests also check that every clickable element has an accessible name.
- [`compose-parity/README.md`](compose-parity/README.md) is the screen-by-screen checklist that compared the Compose app with 0.3.0 before the switch, with the captures of both.
- On the tablet, check by hand in landscape and portrait, light and dark (`adb shell cmd uimode night yes|no`): the first screen and choosing a folder, a scan with its banner, the library sections in grid and list, opening a score in the PDF viewer and returning with Back, the details beside the list (landscape) or full screen (portrait), editing details, Search (from its icon and with Ctrl K, every scope chip), Settings, Sets (create, edit, add pieces and text notes, reorder, delete), export and import, and TalkBack names on the main controls.

## What to attach for diagnosis

- Full crash/tombstone block, including a short lead-in and the timestamp.
- Exact reproduction steps and, for a folder problem, the kind of folder/provider selected.
- Device model and Android version; target is Galaxy Tab S10 FE+ (`SM_X626B`). Record whether the test ran on the physical tablet or the API 35 emulator.
- What the app showed: the library, a setup screen, an error, or nothing; and whether it happens every time.
- APK version and build (Settings shows the commit), and whether it was a debug or release build.

Do not infer a root cause from the symptom alone. Folder selection crosses Android's picker, the Kotlin activity-result callback, the core call, the Storage Access Framework tree and the local index; the trace should identify which boundary failed.

## References

- [Android logcat command-line tool](https://developer.android.com/tools/logcat)
- [Android `dumpsys` diagnostics](https://developer.android.com/tools/dumpsys)
- [Android `DocumentsContract.Document` API](https://developer.android.com/reference/android/provider/DocumentsContract.Document)
- [Android adaptive canonical layouts](https://developer.android.com/develop/adaptive-apps/guides/canonical-layouts)
- [Android NDK native crash debugging](https://developer.android.com/ndk/guides/debug)
- [Roborazzi](https://github.com/takahirom/roborazzi)
- [SQLite FTS5 query syntax](https://www.sqlite.org/fts5.html)
