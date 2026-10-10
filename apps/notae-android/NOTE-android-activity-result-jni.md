# Wiring Android Activity Results back to Rust

## Why this note exists

`android-activity` 0.6 (the runtime `cargo-apk` packages for this
crate) does **not** surface Activity Results through its
`AndroidApp::poll_events` loop. The `PollEvent` enum is `{ Wake,
Timeout, Main(MainEvent) }` and `MainEvent` is the standard
`NativeActivity` lifecycle set (`InitWindow`, `Resume`, `Pause`,
`Destroy`, `ConfigChanged`, `LowMemory`, …). There is no
`MainEvent::ActivityResult`. (Verified against
`android-activity-0.6.1/src/lib.rs:455-547` in the registry.)

That gap is the seam this note documents. The Storage Access
Framework picker is a `startActivityForResult` call, so without a
seam somewhere in the build, the user would tap "Select folder", the
OS picker would open, the user would pick a folder, and the Rust
future would never see the result. The runtime does not hand it to
us.

## How the seam is wired

The fix is a Java `NativeActivity` subclass —
`apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java`
— that overrides `Activity.onActivityResult(int requestCode, int
resultCode, Intent data)` and routes the call back into Rust through
a JNI symbol the `jni` crate resolves by name:

```java
package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;
import android.os.Bundle;

public class NotaeActivity extends NativeActivity {
    static {
        System.loadLibrary("notae_android");
    }

    private static native void nativeOnActivityResult(
        int requestCode, int resultCode, Intent data);

    @Override
    public void onActivityResult(int requestCode, int resultCode, Intent data) {
        nativeOnActivityResult(requestCode, resultCode, data);
        super.onActivityResult(requestCode, resultCode, data);
    }
}
```

The Rust side declares the matching `extern "system" fn
Java_org_notae_android_NotaeActivity_nativeOnActivityResult` in
`apps/notae-android/src/android/saf.rs`. The symbol's name is the
`jni` 0.22 "mangled" form of `nativeOnActivityResult(JNIEnv*, jobject,
jint, jint, jobject)` — the `jni_mangle!` proc-macro emits it. The
function body attaches to the JVM, parses `data.getData()` into the
`tree:` URI string, and forwards it to
[`PickerState::dispatch`](crate::picker_fut::PickerState::dispatch)
under the same `request_code` the picker future registered.

The `AndroidManifest.xml`'s `<activity android:name="…">` entry has
to point at the subclass (`org.notae.android.NotaeActivity`) rather
than `android.app.NativeActivity` so the override is hit. The
`lib_name` meta-data stays `notae_android` so `System.loadLibrary`
finds `libnotae_android.so`.

## Why the alternative is rejected

The brief noted two options:

* **Option A (this note, the implemented path):** a Java
  `NativeActivity` subclass + a JNI symbol.
* **Option B:** poll `ANativeActivity_callbacks::onActivityResult`
  directly from Rust through a raw-pointer shim. `android-activity`
  0.6 does not expose the field and 0.5/0.6 do not push
  `MainEvent::ActivityResult`, so the only path that does not involve
  a fork of `android-activity` is the Java subclass.

`cargo-apk` accepts the Java file under
`android/src/main/java/org/notae/android/` because the standard
Android Gradle plugin picks up `src/main/java/**/*.java` into the
`classes.jar` it bundles. We do not need to enable the Gradle
plugin's full features; the Java source is compiled to a class and
slammed into the APK alongside the `cdylib` Rust produces.

## Risks / things to watch

1. **`request_code` collisions.** The picker future reserves
   request codes through `PickerState::next_request_code`, which
   starts at `1` and increments. The picker can launch while another
   picker is in flight only if the user is faster than the OS, which
   is the same scenario the rest of the handshake is built for. If
   a second picker ever ships, the bridge has to also handle
   `Activity.RESULT_CANCELED` as "first picker was dismissed" and
   not as "second picker was dismissed" — a future task, not
   this one.
2. **`Intent` may be `null`.** When `resultCode != RESULT_OK` the
   platform hands us a `null` `Intent`. The JNI bridge has to handle
   the null without crashing; a `null` data is the "user
   dismissed" signal and maps to
   [`StorageError::Cancelled`](crate::storage::StorageError::Cancelled).
3. **`data.getData()` is a `Uri`, not a `String`.** The JNI bridge
   calls `Uri.toString()` to get the canonical tree URI; the result
   is what the future yields as
   [`PickerOutcome::Ok::tree_uri`](crate::picker_fut::PickerOutcome::Ok::tree_uri).
4. **Compile-time only.** The host tests (`tests/picker_fut.rs`)
   exercise the handshake directly and do not need a device. The
   device-side check is a manual run on the Galaxy Tab S10 FE+.

## Files

| File                                                              | Role                                                  |
|-------------------------------------------------------------------|-------------------------------------------------------|
| `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java` | The `NativeActivity` subclass that overrides `onActivityResult`. |
| `apps/notae-android/android/AndroidManifest.xml`                  | `<activity android:name>` points at `NotaeActivity`.   |
| `apps/notae-android/src/android/saf.rs`                           | Declares `Java_org_notae_android_NotaeActivity_nativeOnActivityResult` and forwards to `PickerState::dispatch`. |
| `apps/notae-android/src/android_main.rs`                          | Wires the SAF handles (`SafHandles`) into the Slint callbacks; does not touch JNI directly. |
| `apps/notae-android/tests/picker_fut.rs`                          | Host tests that cover the handshake contract; do not need a device. |