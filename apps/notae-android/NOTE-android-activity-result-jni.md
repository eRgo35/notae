# Wiring Android Activity Results back to Rust

## Status (Task 2 implementation, 2026-10-10)

The approved `cargo-apk2` 1.4.0 path is implemented. The APK now
contains the Java `NotaeActivity` subclass and its Rust JNI result
handler. The result handler validates the selected URI/read grant,
persists the URI permission and root metadata, then dispatches the
picker outcome. `handle_picker_response` uses `slint::spawn_local`,
so it does not block the event loop while awaiting the Activity result.

Host tests, Android-target Rust compilation, release APK packaging,
manifest inspection, and APK payload inspection pass. Device behavior
is still unverified: `adb devices` reported no connected device. The
target-tablet picker/responsiveness/relaunch/revocation smoke remains
required before calling the SAF path end-to-end validated.

Recursive SAF enumeration is not implemented yet; it is Task 3 in
`docs/superpowers/plans/2026-10-09-notae-android-plan.md`.

## Why `cargo-apk` 0.10 could not close the result-delivery seam

In round 4, the implementation used `cargo-apk` 0.10. That packager
does not compile Java sources (it runs neither `javac` nor `gradle`)
and hard-codes the shipped manifest's `android:name` to
`android.app.NativeActivity` (the `ndk-build` `default_activity_name`
in `/tmp/ndk-build-0.10.0/src/manifest.rs:330`, with no
`[package.metadata.android]` override field). As a result:

* The `NotaeActivity` subclass and its JNI bridge
  (`Java_org_notae_android_NotaeActivity_nativeOnActivityResult`)
  that previous rounds shipped are **dead code** — the subclass is
  not compiled, and even if it were the OS would never instantiate
  it because the manifest points at the plain `NativeActivity`.
* `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java`
  is **deleted** (round 4 commit; the empty `android/src/` tree is
  gone too). The corresponding JNI symbol and `deliver_activity_result`
  in `apps/notae-android/src/android/saf.rs` are also deleted.
* `apps/notae-android/android/AndroidManifest.xml` now references
  `android.app.NativeActivity` (matches what cargo-apk 0.10 actually
  ships) instead of the deleted subclass.
* At that revision, `select_root` opened the picker but Rust had no
  result callback, so the picker future remained pending.

The `notae-android` *contract* — `LibraryRootProvider` /
`LibrarySource` / `StorageState` — is host-testable today
(`cargo test -p notae-android`), and the waker-map bug from
round 3 is fixed: `SelectRootFuture` now registers through
`PickerState::register_waker`, the same map `dispatch` walks
(see `apps/notae-android/src/picker_fut.rs::poll_outcome` and
`tests/picker_fut.rs::poll_outcome_registers_waker_into_dispatchers_map_and_completes`).
The bridge was missing for the `cargo-apk` 0.10 packaging path. The
approved `cargo-apk2` migration described above replaces that path;
it remains pending implementation.

## Why the previous Java subclass was deleted

* `cargo-apk` 0.10 does not invoke `javac`, `kotlinc`, or any Java
  build step. The Java file under `android/src/main/java/**/` is
  read by **nothing** in the build pipeline. The `cdylib` is the
  only payload it packs into the APK; `libnotae_android.so` gets
  found by `System.loadLibrary("notae_android")` via the
  `<meta-data android:name="android.app.lib_name">` entry.
* `cargo-apk`'s default activity name (passed straight through to
  the produced `AndroidManifest.xml`) is `android.app.NativeActivity`,
  not whatever the user names in `[package.metadata.android]`. There
  is no field exposed in `cargo-apk` 0.10's manifest schema to
  override it (`/tmp/ndk-build-0.10.0/src/manifest.rs:330`). The OS
  therefore instantiates the bare `NativeActivity`, never the
  `NotaeActivity` subclass. Even if Java did compile, the override
  would never fire.
* The `Java_org_notae_android_NotaeActivity_nativeOnActivityResult`
  JNI symbol was the bridge, and with the Java caller gone it has
  no caller. It is removed.

## Result-delivery constraint

The proposed pure-Rust `ANativeActivityCallbacks` hook is not
available. The NDK header
`$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include/android/native_activity.h`
defines callbacks for lifecycle, window, input, configuration, and
memory events; it defines no `onActivityResult` member. The
`ndk-sys` bindings used by this crate confirm the same ABI. Writing
or indexing a nonexistent callback slot would corrupt the callback
table, not intercept the picker result.

The round-4 callback proposal relied on the incorrect assumption that
this NDK callback exists. It cannot be implemented safely. At that
revision, the `cargo-apk` 0.10 build had no Java subclass in the APK
and no Rust activity-result callback, so the picker future remained
pending after a user selection.

The supported implementation path is `cargo-apk2`, which compiles
Java sources and packages a custom `NativeActivity` subclass without
introducing Gradle. The user approved this path on 2026-10-10. Do not
fake the callback by extending or casting the NDK struct.

## Current result-delivery implementation

`NotaeActivity extends NativeActivity` forwards Android's
`onActivityResult` request/result codes, URI string, and read-grant bit
through `Java_org_notae_android_NotaeActivity_nativeOnActivityResult`.
Rust validates the result with `decode_activity_result`, persists the
read permission through the Activity's `ContentResolver`, stores the
selected root URI/name/ID in `SharedPreferences`, and dispatches the
result through `PickerState`.

`handle_picker_response` schedules `LibraryRootProvider::select_root`
with `slint::spawn_local`; it does not block the main event loop.
`cargo apk2 check -p notae-android --target aarch64-linux-android`
passes. `cargo test -p notae-android` passes (33 tests). The release
APK is signed and aligned; its manifest declares `.NotaeActivity`,
`android.app.lib_name=notae_android`, and the launcher intent filter,
and its payload contains `classes.dex` and
`lib/arm64-v8a/libnotae_android.so`.

These checks do not prove Android runtime behavior. No device is
attached, so actual Activity result delivery, permission persistence
across relaunch, permission revocation handling, and event-loop
responsiveness remain to be smoke-tested on the Galaxy Tab S10 FE+.

## Files touched in round 4

| File | Change |
|------|--------|
| `apps/notae-android/src/picker_fut.rs` | Make `register_waker` `pub`; add `pub fn poll_outcome` shared by both futures; add `pub fn inner_pending_len` for tests. |
| `apps/notae-android/src/android/saf.rs` | Delete `PENDING_ROOT_ENTRIES` / `PendingRootEntries` and the static `NEXT_REQUEST_CODE`. `SelectRootFuture` stores an `Arc<AtomicWaker>` and calls `poll_outcome` (registers into the dispatcher's map). Delete `deliver_activity_result`, the `Java_org_notae_android_NotaeActivity_nativeOnActivityResult` JNI symbol, and `launch_picker_jni`'s self-allocation of the request code (the caller now allocates from `PickerState::next_request_code`). |
| `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java` | **Deleted**. |
| `apps/notae-android/android/src/**` (empty tree) | **Deleted**. |
| `apps/notae-android/android/AndroidManifest.xml` | Reference manifest now points at `android.app.NativeActivity` (matches cargo-apk 0.10's hard-coded default). |
| `apps/notae-android/tests/picker_fut.rs` | Add `poll_outcome_registers_waker_into_dispatchers_map_and_completes` and `poll_outcome_fast_paths_when_outcome_already_dispatched`. The pre-existing `dispatch_wakes_the_future_waker` covers `SelectRootHandshake` (the host-testable future); the two new tests cover `SelectRootFuture`'s polling helper directly. |
