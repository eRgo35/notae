//! Slint + android-activity UI for the Notae score library.
//!
//! The state machine, stores and view models live in `notae-app-core`
//! and are host-testable on Linux. This crate holds the Slint glue — the
//! generated bindings, the row projections in [`rows`], and the English and
//! Polish text for the core's typed values in [`texts`] — and the
//! `android_main` entry point, which only has to compile for Android.
//!
//! * On Android, `android_main` is the entry symbol the
//!   `android-activity` runtime calls. It initialises the platform,
//!   builds the Slint window, and runs the UI loop.
//! * On Linux, no `main` exists — there is nothing to run, only the
//!   Slint UI tests link against the generated bindings.
//!
//! `unsafe_code` policy is set per module:
//!
//! * [`rows`] declares `#![forbid(unsafe_code)]` so the Slint glue
//!   remains strictly safe; the core crate forbids it crate-wide.
//! * `android_main` declares `#![allow(unsafe_code)]` because it
//!   owns the FFI shim (`#[unsafe(no_mangle)] fn android_main`) that
//!   the `android-activity` runtime requires. The exception is
//!   scoped to that one file.
//!
//! The crate root intentionally does *not* apply a crate-wide lint
//! override — `forbid` cannot be locally downgraded later, so leaving
//! it off here keeps the per-module policies in control.

pub mod accessibility;
pub mod language;
pub mod picker_fut;
pub mod saf_tree;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod android_main;

pub mod rows;
pub mod texts;

pub mod motion;
pub mod voice_search;

// Pull in the Slint-generated bindings for `app.slint`. The build script
// in `build.rs` writes this file as `$OUT_DIR/app.rs`; the path uses
// `env!("OUT_DIR")` so the include works on Android *and* on Linux
// (where the Linux-side smoke test indirectly exercises the same build
// pipeline).
// Slint's generated bindings intentionally use unwrap/expect internally;
// keep the production panic policy strict everywhere else by containing
// this lint exception in a private module around generated source only.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/app.rs"));
}

pub use generated::*;
