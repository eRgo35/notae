//! Build script for `notae-ocr`.
//!
//! This crate links the system Tesseract and Leptonica shared libraries through
//! `tesseract = "0.15.2"`, whose `tesseract-sys` and `leptonica-sys` build
//! scripts locate them by `pkg-config tesseract` and `pkg-config lept`. Those
//! `.pc` files do not exist on every Linux distribution, and on the ones that
//! ship them under `prefix=/usr` the actual shared object is only available as
//! the versioned `libtesseract.so.5.5.3`, with no `libtesseract.so` symlink for
//! the linker to follow.
//!
//! This script handles both gaps so the rest of the crate can stay portable:
//!
//! * When `pkg-config tesseract` and `pkg-config lept` succeed, nothing extra
//!   is emitted; the link search paths come from the `.pc` files and the
//!   linker finds the libraries through them.
//! * When the system ships a `libtesseract.so` (unversioned) and
//!   `libleptonica.so` (unversioned) in `/usr/lib64`, nothing extra is emitted
//!   because `tesseract-sys` already emits that directory and `cc`'s default
//!   search paths will resolve the unversioned names.
//! * Otherwise, the script materialises a directory under `OUT_DIR` whose
//!   unversioned symlinks point at the actual shared objects, and emits a
//!   `rustc-link-search` directive so the linker finds them. The directory
//!   lives inside `OUT_DIR` because the build is the only thing that needs
//!   it; nothing under `/tmp` or `/etc` is touched.

use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// Candidate paths for the Tesseract shared library, newest versioned first.
///
/// Distributions vary in which suffix they ship. Newer Fedora releases use
/// `.so.5.5.3`, Debian-derived systems use `.so.5`, macOS Homebrew uses `.5.dylib`.
/// The list is walked in order and the first hit wins.
const TESSERACT_CANDIDATES: &[&str] = &[
    "/usr/lib64/libtesseract.so.5.5.3",
    "/usr/lib64/libtesseract.so.5",
    "/usr/lib64/libtesseract.so.5.5",
    "/usr/lib64/libtesseract.so",
    "/usr/lib/libtesseract.so.5.5.3",
    "/usr/lib/libtesseract.so.5",
    "/usr/lib/libtesseract.so",
    "/usr/local/lib/libtesseract.so.5.5.3",
    "/usr/local/lib/libtesseract.so.5",
    "/usr/local/lib/libtesseract.so",
];

/// Candidate paths for the Leptonica shared library.
const LEPTONICA_CANDIDATES: &[&str] = &[
    "/usr/lib64/libleptonica.so.6",
    "/usr/lib64/libleptonica.so.6.0.0",
    "/usr/lib64/libleptonica.so.5",
    "/usr/lib64/libleptonica.so",
    "/usr/lib/libleptonica.so.6",
    "/usr/lib/libleptonica.so.6.0.0",
    "/usr/lib/libleptonica.so",
    "/usr/local/lib/libleptonica.so.6",
    "/usr/local/lib/libleptonica.so.6.0.0",
    "/usr/local/lib/libleptonica.so",
];

/// Returns the first existing path in `candidates`, or `None` if none match.
fn first_existing(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(Path::new)
        .find(|path| path.exists())
        .map(Path::to_path_buf)
}

/// Emits a `rustc-link-search` for a directory containing `libfoo.so`
/// symlinks, creating them if necessary, when the system does not already
/// provide unversioned shared objects.
fn link_unversioned_shim_if_needed() {
    let needs_shim = !Path::new("/usr/lib64/libtesseract.so").exists()
        || !Path::new("/usr/lib64/libleptonica.so").exists();

    if !needs_shim {
        return;
    }

    let out_dir = match env::var_os("OUT_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => return,
    };

    let shim = out_dir.join("tesseract-link-shim");
    if let Err(error) = fs::create_dir_all(&shim) {
        eprintln!("notae-ocr build script: failed to create shim dir: {error}");
        return;
    }

    if let Some(tesseract) = first_existing(TESSERACT_CANDIDATES) {
        let link = shim.join("libtesseract.so");
        let _ = fs::remove_file(&link);
        if let Err(error) = symlink(&tesseract, &link) {
            eprintln!(
                "notae-ocr build script: failed to symlink {} -> {}: {error}",
                link.display(),
                tesseract.display(),
            );
        }
    }

    if let Some(leptonica) = first_existing(LEPTONICA_CANDIDATES) {
        let link = shim.join("libleptonica.so");
        let _ = fs::remove_file(&link);
        if let Err(error) = symlink(&leptonica, &link) {
            eprintln!(
                "notae-ocr build script: failed to symlink {} -> {}: {error}",
                link.display(),
                leptonica.display(),
            );
        }
    }

    println!("cargo:rustc-link-search=native={}", shim.display());
    println!("cargo:rerun-if-changed={}", shim.display());
}

fn main() {
    link_unversioned_shim_if_needed();
}
