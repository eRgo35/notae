//! Slint build script for `notae-android`.
//!
//! Translates `ui/app.slint` into a `slint` module placed in `$OUT_DIR`.
//! The actual `include!` happens from `src/lib.rs` so the generated
//! bindings are visible there.

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("material".into());
    slint_build::compile_with_config("ui/app.slint", config)
        .expect("failed to compile ui/app.slint");
}
