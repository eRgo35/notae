/*
 * NotaeActivity — Notae's NativeActivity subclass.
 *
 * The android-activity 0.6 runtime does not surface Android Activity
 * Results through its Rust `poll_events` loop (the MainEvent enum
 * only covers the standard NativeActivity lifecycle set: InitWindow,
 * Resume, Pause, Destroy, ConfigChanged, LowMemory, … — no
 * ActivityResult variant). The Storage Access Framework picker is a
 * `startActivityForResult` call, so without a Java-side seam the
 * result would never reach our Rust future.
 *
 * The seam is this subclass: we override `onActivityResult` and
 * forward the call back into Rust through the JNI symbol
 * `Java_org_notae_android_NotaeActivity_nativeOnActivityResult`,
 * which `apps/notae-android/src/android/saf.rs` declares. That symbol
 * routes the result into `PickerState::dispatch`, which wakes the
 * future the picker callback registered under the same request code.
 *
 * `System.loadLibrary("notae_android")` finds
 * `libnotae_android.so`, the `cdylib` `cargo-apk` packages. The
 * `AndroidManifest.xml`'s `<activity android:name="…">` entry
 * points at this subclass so the override is invoked.
 *
 * See `apps/notae-android/NOTE-android-activity-result-jni.md` for
 * the full rationale and risk list.
 */
package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;

public class NotaeActivity extends NativeActivity {
    static {
        System.loadLibrary("notae_android");
    }

    /**
     * Forwards the platform's Activity Result back into Rust. The
     * JNI symbol name is the `jni` 0.22 "mangled" form of
     * `nativeOnActivityResult(JNIEnv*, jobject, jint, jint, jobject)`.
     * `Intent data` may be null when `resultCode != RESULT_OK`; the
     * Rust bridge handles null and maps it to a "user dismissed"
     * picker outcome.
     */
    private static native void nativeOnActivityResult(
        int requestCode, int resultCode, Intent data);

    @Override
    public void onActivityResult(int requestCode, int resultCode, Intent data) {
        nativeOnActivityResult(requestCode, resultCode, data);
        super.onActivityResult(requestCode, resultCode, data);
    }
}