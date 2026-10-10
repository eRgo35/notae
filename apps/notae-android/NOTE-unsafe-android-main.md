# Why the Android crate needs `unsafe`

## Decision
`apps/notae-android` relaxes `unsafe_code` from the workspace's `forbid` down to `deny`
(`apps/notae-android/Cargo.toml`) and uses file-scoped `#![allow(unsafe_code)]` only on
`src/android_main.rs`. Every other module (`state.rs`, `ui.rs`) re-asserts
`#![forbid(unsafe_code)]`.

## Why an `unsafe` surface is unavoidable
- `cargo-apk` packages this crate as a `cdylib` (`Cargo.toml` `[lib] crate-type`) loaded
  by `android.app.NativeActivity`.
- `NativeActivity` calls `android_main` by **literal C symbol name** when the activity
  starts. Rust mangles names by default; without `#[unsafe(no_mangle)]` the runtime
  cannot find the entry point and the app fails to launch.
- `extern "C"` selects the platform's C calling convention — the OS ABI, not Rust's.
- `#[unsafe(no_mangle)]` (the current spelling; `#[no_mangle]` is deprecated) is the
  mechanism Rust provides for that export.

## Why this scoping is correct, not a smell
- Workspace `forbid` stays in force everywhere else in the repo (`Cargo.toml`
  `[workspace.lints.rust]`).
- The allowed surface is **one file**, **one symbol**. The body of `android_main` is
  itself safe Rust (slint platform glue + safe `AppController` / `UiModel` calls).
- The state machine (`state.rs`) and Slint binding (`ui.rs`) are deliberately
  `#[cfg]`-independent and free of FFI, so `cargo test` on Linux exercises the same
  code that runs on device via the crate's dual `cdylib` + `rlib` output.
- `#[cfg(target_os = "android")]` on the symbol keeps it out of host builds.

## Notes for future bumps
- `#![allow(improper_ctypes_definitions)]`: justified today because `slint`'s own
  examples declare the same shape against `slint::android::AndroidApp` (a
  `Arc`-shaped wrapper that is not `#[repr(C)]`). If slint changes the binding,
  re-verify before re-applying the allow.
- If `notae-index` (rusqlite + bundled SQLite) or `notae-ocr` (Tesseract C++) is
  ever linked into this crate, expect more `extern "C"` boundaries. Same scoping
  rules apply: one file per boundary, `#![forbid(unsafe_code)]` everywhere else.
