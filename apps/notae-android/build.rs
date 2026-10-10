//! Slint build script for `notae-android`.
//!
//! Translates `ui/app.slint` into a `slint` module placed in `$OUT_DIR`.
//! The actual `include!` happens from `src/lib.rs` so the generated
//! bindings are visible there.

fn main() {
    slint_build::compile("ui/app.slint").expect("failed to compile ui/app.slint");
}
