//! The grayscale page image this crate's [`OcrProvider`] consumes.
//!
//! # Why this is a re-export, not a type of its own
//!
//! OCR — Tesseract and any adapter that replaces it — takes 8-bit grayscale
//! pixels. `image::GrayImage` is already exactly that, so [`OcrImage`] *is*
//! `image::GrayImage` rather than something that wraps it. A newtype here
//! would buy nothing and would cost a conversion at every call boundary: the
//! OCR crate would have to unwrap it, and any future change to the pixel
//! layout would have to be mirrored on both sides.
//!
//! The same reasoning is documented on
//! [`notae_index_cli::image::RenderedPage`]; the renderer in the CLI emits
//! `image::GrayImage` for the same reason, so a rendered page flows in with
//! zero conversion. The Android adapter can either keep emitting this
//! concrete type or define its own image layout — the trait takes a
//! borrowed [`OcrImage`], so the platform is free to produce whichever
//! representation is cheapest to feed into its own OCR engine, and our
//! tests still load fixtures through the same `image::GrayImage` they
//! already speak.

use image::GrayImage;

/// A rendered PDF page as 8-bit grayscale, ready for OCR.
///
/// This is a re-export of [`image::GrayImage`]; see the module-level
/// explanation for why it is not a distinct type.
pub type OcrImage = GrayImage;
