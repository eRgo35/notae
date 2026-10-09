//! The grayscale page image this crate renders and the OCR provider consumes.
//!
//! # Why this is a re-export, not a type of its own
//!
//! OCR — Tesseract and any adapter that replaces it — takes 8-bit grayscale
//! pixels. `image::GrayImage` is already exactly that, so [`RenderedPage`] *is*
//! `image::GrayImage` rather than something that wraps it. A newtype here would
//! buy nothing and would cost a conversion at every call boundary: the OCR
//! crate would have to unwrap it, and any future change to the pixel layout
//! would have to be mirrored on both sides.
//!
//! Naming the alias [`RenderedPage`] anyway keeps the *intent* legible at the
//! call site — a rendered page is not a photograph and not a thumbnail — while
//! leaving the identity intact. Assigning a `RenderedPage` to an
//! `image::GrayImage`, or the reverse, compiles with no conversion, which is
//! what lets the OCR crate declare
//! `type OcrImage = image::GrayImage` and accept a rendered page directly.

use image::GrayImage;

/// A rendered PDF page as 8-bit grayscale, ready for OCR.
///
/// This is a re-export of [`image::GrayImage`]; see the [module
/// documentation](self) for why it is not a distinct type.
pub type RenderedPage = GrayImage;

/// Converts the renderer's premultiplied RGBA output into grayscale.
///
/// The renderer composites onto an opaque white background, so alpha is
/// discarded rather than composited: a pixel is either ink or paper, and
/// treating it as ink-over-paper would only reintroduce the background it was
/// already composited against. The weights are the usual luminance
/// coefficients — green dominates perceived brightness, so a black-and-white
/// scan stays a black-and-white scan rather than picking up a tint.
pub(crate) fn to_gray(rgba: &[u8]) -> Vec<u8> {
    let (pixels, remainder) = rgba.as_chunks::<4>();
    debug_assert!(
        remainder.is_empty(),
        "the renderer always emits whole RGBA pixels"
    );
    pixels
        .iter()
        .map(|[r, g, b, _]| {
            let luma = 299 * u32::from(*r) + 587 * u32::from(*g) + 114 * u32::from(*b);
            (luma / 1000) as u8
        })
        .collect()
}