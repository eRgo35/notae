//! Android-specific implementations behind `cfg(target_os = "android")`.
//!
//! Storage and external PDF handoff live in [`saf`] and [`pdf_intent`].
//! [`pdf_pages`] uses the platform's local `PdfRenderer` API for page access
//! and API-35 text-layer extraction.
//!
//! This module does *not* apply a crate-wide lint override:
//! [`saf`] opts back into `unsafe` locally with its own
//! `#![allow(unsafe_code)]` for the JNI helpers, the same pattern
//! `android_main.rs` uses, and the rest of the crate keeps
//! `#![forbid(unsafe_code)]`.

pub mod saf;
pub mod pdf_intent;
pub mod pdf_pages;
pub mod theme;