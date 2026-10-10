# Why the Android crate needs `unsafe`

## Decision
`apps/notae-android` relaxes `unsafe_code` from the workspace's `forbid` down to `deny`
(`apps/notae-android/Cargo.toml`) and uses file-scoped `#![allow(unsafe_code)]` on
`src/android_main.rs` *and* `src/android/saf.rs`. Every other module (`state.rs`,
`ui.rs`, `picker_fut.rs`) re-asserts `#![forbid(unsafe_code)]`.

## Why an `unsafe` surface is unavoidable
- `cargo-apk` packages this crate as a `cdylib` (`Cargo.toml` `[lib] crate-type`) loaded
  by `android.app.NativeActivity`.
- `NativeActivity` calls `android_main` by **literal C symbol name** when the activity
  starts. Rust mangles names by default; without `#[unsafe(no_mangle)]` the runtime
  cannot find the entry point and the app fails to launch.
- `extern "C"` selects the platform's C calling convention — the OS ABI, not Rust's.
- `#[unsafe(no_mangle)]` (the current spelling; `#[no_mangle]` is deprecated) is the
  mechanism Rust provides for that export.

## Two `unsafe` boundaries, scoped per boundary

The crate has two distinct FFI surfaces; both are scoped to the file they live in
and the `unsafe` blocks inside are minimal:

1. **`src/android_main.rs`** — the `#[unsafe(no_mangle)] pub extern "C" fn
   android_main` entry point the `android-activity` runtime calls when the activity
   starts. The body itself is safe Rust (Slint platform glue + safe `AppController`
   / `UiModel` calls).

2. **`src/android/saf.rs`** — the JNI bridge the Storage Access Framework provider
   uses to (a) launch the `ACTION_OPEN_DOCUMENT_TREE` intent, (b) read the
   persisted tree URI from `SharedPreferences`, (c) walk the tree via
   `ContentResolver.query` / `openInputStream`. Each JNI call is wrapped in a
   `with_env` helper that attaches the calling thread to the JVM and returns a
   fresh `Env`. The `unsafe` blocks are localised to the helper functions; the
   `LibraryRootProvider` impl, the catalog, and the permission storage are safe
   Rust.

## Why this scoping is correct, not a smell
- Workspace `forbid` stays in force everywhere else in the repo (`Cargo.toml`
  `[workspace.lints.rust]`).
- Each allowed surface is **one file**, scoped to a single boundary.
- The state machine (`state.rs`), Slint binding (`ui.rs`), and picker handshake
  (`picker_fut.rs`) are deliberately `#[cfg]`-independent and free of FFI, so
  `cargo test` on Linux exercises the same code that runs on device via the
  crate's dual `cdylib` + `rlib` output.
- `#[cfg(target_os = "android")]` on the entry point and on the entire
  `crate::android` module tree keeps the FFI surfaces out of host builds.

## Notes for future bumps
- `#![allow(improper_ctypes_definitions)]` on `android_main.rs`: justified today
  because `slint`'s own examples declare the same shape against
  `slint::android::AndroidApp` (a `Arc`-shaped wrapper that is not `#[repr(C)]`).
  If slint changes the binding, re-verify before re-applying the allow.
- `#![allow(deprecated)]` on `saf.rs`: the `jni` 0.22 surface includes a couple of
  deprecated APIs (e.g. `JObject::from_raw`); the review notes that the upgrade to
  `jni` 0.23 / `jni-sys` 0.4 lifts them. Until that lands, the allow keeps the
  build log clean. Re-verify on every `jni` bump.
- If `notae-index` (rusqlite + bundled SQLite) or `notae-ocr` (Tesseract C++) is
  ever linked into this crate, expect more `extern "C"` boundaries. Same scoping
  rules apply: one file per boundary, `#![forbid(unsafe_code)]` everywhere else.
