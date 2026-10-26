//! Slint build script for `notae-android`.
//!
//! Translates `ui/app.slint` into a `slint` module placed in `$OUT_DIR`.
//! The actual `include!` happens from `src/lib.rs` so the generated
//! bindings are visible there.

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=ui/setup.slint");
    println!("cargo:rerun-if-changed=translations");
    let build_identity = git_build_identity();
    println!("cargo:rustc-env=NOTAE_BUILD_ID={build_identity}");
    // Every string is translated once, whichever component shows it; see translations/.
    let mut config = slint_build::CompilerConfiguration::new()
        .with_style("material".into())
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        config = config.with_debug_info(true);
    }
    slint_build::compile_with_config("ui/app.slint", config)?;
    Ok(())
}

fn git_build_identity() -> String {
    let head_path = Command::new("git")
        .args(["rev-parse", "--git-path", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|path| path.trim().to_owned());
    if let Some(path) = &head_path {
        println!("cargo:rerun-if-changed={path}");
    }
    let packed_refs = Command::new("git")
        .args(["rev-parse", "--git-path", "packed-refs"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|path| path.trim().to_owned());
    if let Some(path) = packed_refs {
        println!("cargo:rerun-if-changed={path}");
    }
    let reference = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    if let Some(reference) = reference {
        let path = Command::new("git")
            .args(["rev-parse", "--git-path", &reference])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|path| path.trim().to_owned());
        if let Some(path) = path {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_owned())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}
