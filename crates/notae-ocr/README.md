# `notae-ocr` — local multilingual OCR

`notae-ocr` is the Tesseract-backed OCR provider Notae uses on Linux.
The Android app has its own adapter (Tesseract is not the right
runtime there) and the two sides communicate through the
`OcrProvider` trait in this crate; nothing about the language model
or the engine leaks across the boundary.

The crate is **not** a re-implementation of Tesseract's API. It is a
thin wrapper that hides the engine's `unsafe` surface, exposes a
typed error model, and lets the indexer run with an automatic
language policy without ever loading a pack it did not ask for.

## What it provides

* `OcrProvider` — the trait every concrete engine implements.
* `TesseractProvider` — the Tesseract implementation.
* `TesseractConfig` — the tessdata directory and the discovered pack
  list. Build it by hand from a known list, or use
  `TesseractConfig::discover(dir)` to scan a directory.
* `LanguagePolicy::Automatic` / `Fixed` — the per-call language
  selection.
* `InstalledLanguages` — the deduplicated pack list a tessdata
  directory yielded.
* `LanguageDetector` — a "what language is this page?" helper that
  runs every installed text pack and picks the highest-confidence
  match. Carries the script name Tesseract detected when
  `osd.traineddata` is on disk.
* `OcrText` — the recognized text, the language(s) the engine ran
  against, the mean confidence, and any non-fatal warnings.
* `OcrWarning` — the things the engine saw but did not fail on
  (a missing pack, an empty result).
* `OcrError` — the typed failure cases (`TessdataDirectoryUnavailable`,
  `EngineInitializationFailed`, `RecognitionFailed`, `InvalidImage`).

## Linux build setup

The hard part of building this crate is not Rust — it is finding the
Tesseract and Leptonica shared libraries on a Linux distribution that
ships them under names the Rust binding does not expect.

### What the upstream `tesseract` crate expects

The `tesseract` crate's `tesseract-sys` build script runs:

```sh
pkg-config --libs --cflags tesseract
pkg-config --libs --cflags lept
```

Those lookups need `tesseract.pc` and `lept.pc` on the search path.
On a stock Fedora 44 box they are not present, because `tesseract`
itself is installed but the `-devel` package (which provides the
`.pc` files) is not. `libtesseract.so.5.5.3` and
`libleptonica.so.6.0.0` are present in `/usr/lib64`, but there is no
`libtesseract.so` (unversioned) symlink for the linker to follow.

### What this crate's `build.rs` does

The build script in this crate runs in front of `tesseract-sys` and:

1. **Materialises a `pkg-config` search directory** at
   `$PKG_CONFIG_PATH` (when it is set) with the `.pc` files the
   upstream build script needs. See "Reproducing the dev-env shim"
   below.
2. **Materialises an `OUT_DIR/tesseract-link-shim/` directory** with
   `libtesseract.so` and `libleptonica.so` symlinks pointing at the
   actual versioned shared objects, and emits a `rustc-link-search`
   directive so the linker can find them.

When `/usr/lib64/libtesseract.so` and `/usr/lib64/libleptonica.so`
are present unversioned, the script does nothing — a clean
distribution just works.

### Reproducing the dev-env shim

The shim is built by extracting the `.pc` files from Fedora's
`tesseract-devel`, `tesseract-libs`, `leptonica-devel`,
`libjpeg-turbo-devel`, `libtiff-devel`, and `libwebp-devel` RPMs
into a single directory:

```sh
# 1. Create a working directory
mkdir -p /tmp/tesseract-pkgconfig
cd /tmp/tesseract-pkgconfig

# 2. Extract the .pc files from the RPMs. The exact command depends
#    on how the RPMs are obtained; on Fedora 44 the .pc files live
#    under /usr/lib64/pkgconfig/ and the .so symlinks under
#    /usr/lib64/. Either install the -devel packages (when sudo is
#    available) or extract the RPM contents:
for rpm in \
  tesseract \
  tesseract-devel \
  leptonica \
  leptonica-devel \
  libjpeg-turbo-devel \
  libtiff-devel \
  libwebp-devel
do
  rpm2cpio "/path/to/${rpm}.rpm" | cpio -idm
done

# 3. Copy the .pc files into the search directory
cp ./usr/lib64/pkgconfig/*.pc /tmp/tesseract-pkgconfig/

# 4. Edit tesseract.pc and lept.pc so the libdir is /usr/lib64
#    (or wherever the actual .so files live) and the libs list
#    points to the system shared objects.
```

The exact contents of the rewritten `.pc` files are not
reproduced here because they vary across Fedora / RHEL minor
versions. The shape is:

```text
prefix=/usr
exec_prefix=${prefix}
libdir=/usr/lib64
includedir=${prefix}/include

Name: tesseract
Description: ...
Version: 5.5.0
Libs: -L${libdir} -ltesseract
Cflags: -I${includedir}
```

with `lept.pc`, `libjpeg.pc`, `libtiff-4.pc`, `libwebp.pc`,
`libwebpmux.pc`, plus a stub `Lerc.pc` for the Lerc library the
bindings depend on.

### Running the build

With the shim in place:

```sh
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig \
  cargo build -p notae-ocr
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig \
  cargo test -p notae-ocr
```

The build script in this crate will create the OUT_DIR symlinks
transparently; nothing under `/tmp` outside of `OUT_DIR` is touched.

### Tesseract language data

The `notae-ocr` tests use a vendored `tests/tessdata/` directory
that carries:

* `eng.traineddata` (English)
* `pol.traineddata` (Polish)
* `lat.traineddata` (Latin)
* `osd.traineddata` (orientation/script detection)

These are committed because not every Linux box has the system
tessdata populated, and the integration tests need to know exactly
which packs are available.

For a production install, install the language packs you actually
need through the package manager:

```sh
# Fedora / RHEL
sudo dnf install tesseract-langpack-eng tesseract-langpack-pol

# Debian / Ubuntu
sudo apt install tesseract-ocr-eng tesseract-ocr-pol
```

## Linux dev-env on Fedora 44 (no sudo)

A second-developer-friendly recipe for a host that does not have
sudo and cannot install `-devel` packages:

1. **Make sure `pkg-config` is on `PATH`.** On Fedora 44 it lives
   at `/usr/bin/pkg-config` and is installed by default.
2. **Build the shim as described above** into a directory you own
   (e.g. `/tmp/tesseract-pkgconfig`).
3. **Export the path before building:**
   ```sh
   export PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig
   ```
4. **Build the crate:**
   ```sh
   cargo build -p notae-ocr
   cargo test  -p notae-ocr
   ```
5. The build script's `OUT_DIR/tesseract-link-shim/` directory is
   recreated on every `cargo` invocation. Nothing outside
   `OUT_DIR` is touched.

The same recipe works for any Linux distribution: extract the
`.pc` files, rewrite the `libdir` lines, point `PKG_CONFIG_PATH`
at the result, and the build script handles the unversioned `.so`
symlinks.
