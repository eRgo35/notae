//! Slint + android-activity UI for the Notae portable sidecar.
//!
//! The crate is structured so the state machine in [`state`] is
//! host-testable on Linux, while the Slint glue in [`ui`] and the
//! `android_main` entry point only have to compile for Android targets.
//!
//! * On Android, `android_main` is the entry symbol the
//!   `android-activity` runtime calls. It initialises the platform,
//!   builds the Slint window, and runs the UI loop.
//! * On Linux, no `main` exists — there is nothing to run, only the
//!   `cargo test --test smoke` harness links against the `state` module.
//!
//! `unsafe_code` policy is set per module:
//!
//! * [`state`] and [`ui`] declare `#![forbid(unsafe_code)]` so the
//!   host-testable core and the Slint glue remain strictly safe.
//! * `android_main` declares `#![allow(unsafe_code)]` because it
//!   owns the FFI shim (`#[unsafe(no_mangle)] fn android_main`) that
//!   the `android-activity` runtime requires. The exception is
//!   scoped to that one file.
//!
//! The crate root intentionally does *not* apply a crate-wide lint
//! override — `forbid` cannot be locally downgraded later, so leaving
//! it off here keeps the per-module policies in control.

pub mod accessibility;
pub mod favorites;
pub mod language;
pub mod picker_fut;
pub mod recent;
pub mod saf_tree;
pub mod state;
pub mod storage;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod android_main;

pub mod ui;
pub mod ui_prefs;

pub mod motion;
pub mod operations;
pub mod pdf_intent;
pub mod pdf_pages;
pub mod thumbnails;
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
