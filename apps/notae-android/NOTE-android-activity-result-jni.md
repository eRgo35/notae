# Wiring Android Activity Results back to Rust

## Status (round 4)

**The seam this note used to describe is closed.** `cargo-apk` 0.10
does not compile Java sources (it runs neither `javac` nor
`gradle`), and it hard-codes the shipped manifest's
`android:name` to `android.app.NativeActivity` (the
`ndk-build` `default_activity_name` in
`/tmp/ndk-build-0.10.0/src/manifest.rs:330` is
`"android.app.NativeActivity"`, with no `[package.metadata.android]`
field to override it). As a result:

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
* `select_root` still launches `ACTION_OPEN_DOCUMENT_TREE` and the
  picker will open on the user's device. The activity result is
  delivered to `NativeActivity` — Rust has no hook into that
  path. **The picker future will hang** on the device until one of
  the two follow-on options below is taken.

The `notae-android` *contract* — `LibraryRootProvider` /
`LibrarySource` / `StorageState` — is host-testable today
(`cargo test -p notae-android`), and the waker-map bug from
round 3 is fixed: `SelectRootFuture` now registers through
`PickerState::register_waker`, the same map `dispatch` walks
(see `apps/notae-android/src/picker_fut.rs::poll_outcome` and
`tests/picker_fut.rs::poll_outcome_registers_waker_into_dispatchers_map_and_completes`).
The architecture is sound. The on-device bridge from "user picked
a folder" to "the picker future completes" is the missing seam,
and it cannot be closed on this host with `cargo-apk` 0.10.

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

## Two follow-on options (not implemented)

These are the two paths forward. Round 4 records them so the user
can pick; this round does not implement either.

### Option 1 — pure-Rust `ANativeActivity_callbacks` hook

`android-activity` 0.6 hands `AndroidApp::activity_as_ptr()` to
the Rust side; the returned pointer is an `ANativeActivity*`
whose `callbacks` field carries the `onActivityResult` function
pointer. The plan:

1. Read the `ANativeActivity_callbacks` struct from
   `AndroidApp::activity_as_ptr()`, obtain the `onActivityResult`
   slot, and the `callbacks` pointer we read it from.
2. Replace `onActivityResult` with a Rust trampoline that calls
   `PickerState::dispatch` with the bridged (requestCode,
   resultCode, data) tuple, then chains to the original
   `onActivityResult` so the rest of the platform lifecycle stays
   intact.
3. The trampoline is `unsafe` (it crosses a C-ABI function
   pointer table). The `apps/notae-android/NOTE-unsafe-android-main.md`
   exception currently covers `android_main.rs` and `saf.rs`;
   `saf.rs` is the natural home for the trampoline, so no
   additional user agreement is needed.
4. Build the `trampoline` signature against the `jni` 0.22
   `Env<'_>` shape (same one the round-3
   `activity_as_jobject` fix uses), thread the `JNIEnv*` the
   platform hands the trampoline through to a `PickerState::dispatch`
   call.

This closes the seam without a Gradle migration. Cost: a few
hundred lines of FFI against the `ANativeActivity_callbacks` v1
struct layout (Documented in `android/native_activity.h` from the
NDK headers) and a memory model decision (do we restore the
original `onActivityResult` on app shutdown, or leave the
trampoline installed for the process lifetime?).

### Option 2 — Gradle migration

Stop using `cargo-apk`. Move to a Gradle-based Android project
that:

1. Compiles Java sources through the standard Android Gradle
   plugin.
2. Calls into the Rust `cdylib` via the standard Android NDK
   mechanism (`externalNativeBuild` block in `build.gradle`).
3. Provides the `NotaeActivity` subclass properly (the seam closes
   exactly like the round-2 design intended).

Cost: a significantly larger refactor. We lose the
single-`Cargo.toml` build description; the `notae-android`
crate becomes a thin Rust library called into from a Gradle
project. `cargo-apk` would not be involved in the build any more.
The host tests still run on Linux (the storage-state, picker-fut,
and smoke suites are platform-independent); only the APK build
moves to Gradle.

The user has to pick one of these. Round 4 does not.

## What currently works on the host

* `cargo test -p notae-android` passes (29 tests: 5 lib + 9
  picker-fut + 7 smoke + 8 storage-state).
* `cargo test --workspace` passes.
* `cargo clippy --workspace --all-targets -- -D warnings` is
  clean on the host target.
* `cargo apk build --release -p notae-android` produces a signed,
  aligned, verified APK. The release `.so` carries every JNI
  helper except the dead `NotaeActivity` bridge.
* `tests/picker_fut.rs::poll_outcome_registers_waker_into_dispatchers_map_and_completes`
  proves the waker is registered into the dispatcher's map and is
  fired by the next dispatch. `SelectRootFuture::poll` (the
  Android-only future) calls the same helper, so the contract it
  enforces on Linux is the contract that ships on the device.

What does **not** work on the device: the picker future hangs
after the user picks a folder, because the OS's onActivityResult
has no Rust hook. Fixing that is Options 1 or 2.

## Files touched this round

| File | Change |
|------|--------|
| `apps/notae-android/src/picker_fut.rs` | Make `register_waker` `pub`; add `pub fn poll_outcome` shared by both futures; add `pub fn inner_pending_len` for tests. |
| `apps/notae-android/src/android/saf.rs` | Delete `PENDING_ROOT_ENTRIES` / `PendingRootEntries` and the static `NEXT_REQUEST_CODE`. `SelectRootFuture` stores an `Arc<AtomicWaker>` and calls `poll_outcome` (registers into the dispatcher's map). Delete `deliver_activity_result`, the `Java_org_notae_android_NotaeActivity_nativeOnActivityResult` JNI symbol, and `launch_picker_jni`'s self-allocation of the request code (the caller now allocates from `PickerState::next_request_code`). |
| `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java` | **Deleted**. |
| `apps/notae-android/android/src/**` (empty tree) | **Deleted**. |
| `apps/notae-android/android/AndroidManifest.xml` | Reference manifest now points at `android.app.NativeActivity` (matches cargo-apk 0.10's hard-coded default). |
| `apps/notae-android/tests/picker_fut.rs` | Add `poll_outcome_registers_waker_into_dispatchers_map_and_completes` and `poll_outcome_fast_paths_when_outcome_already_dispatched`. The pre-existing `dispatch_wakes_the_future_waker` covers `SelectRootHandshake` (the host-testable future); the two new tests cover `SelectRootFuture`'s polling helper directly. |
