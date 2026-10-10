//! Android-specific implementations behind `cfg(target_os = "android")`.
//!
//! The Storage Access Framework provider lives in [`saf`]. It is the
//! only place that talks to the Android platform: `ContentResolver`,
//! tree URIs, persisted read permission, and the `ACTION_OPEN_DOCUMENT_TREE`
//! intent that opens the system file picker.
//!
//! This module does *not* apply a crate-wide lint override:
//! [`saf`] opts back into `unsafe` locally with its own
//! `#![allow(unsafe_code)]` for the JNI helpers, the same pattern
//! `android_main.rs` uses, and the rest of the crate keeps
//! `#![forbid(unsafe_code)]`.

pub mod saf;
pub mod pdf_intent;