# Notae

Notae is a personal music-library manager. The library lives on a
desktop (a Linux box that runs the indexer); the user reads,
annotates, and searches it on a tablet (a Galaxy Tab S10 FE+ running
Android).

The desktop and the tablet do not share a file system. They share
the **sidecar**: a versioned JSON manifest the indexer writes
beside its SQLite payload, and the Android app reads before
importing anything.

This repository contains the complete Notae workspace: the Linux indexer,
OCR layer, SQLite store, portable sidecar format, and Android tablet app.
The desktop writes the sidecar; Android imports it without requiring the
devices to share a file system.

## Workspace layout

```text
crates/
  notae-domain/     Filename convention, note metadata, file identity.
                    The types every other crate imports.
  notae-index/      SQLite persistence, FTS5 search, the sidecar
                    format, and the importer the Android app uses.
  notae-ocr/        Tesseract-backed OCR provider. See
                    crates/notae-ocr/README.md for the dev-env setup.
apps/
  notae-index/      The `notae-index` binary. See apps/notae-index/README.md.
  notae-android/    Android application and tablet UI. See
                    docs/android-debugging.md for build and device checks.
```


## Build and quality gates

The Rust workspace is pinned to 1.98.1 in `rust-toolchain.toml`. On Linux,
install `pkg-config`, Tesseract development headers, and Leptonica development
headers first. Fedora/RHEL hosts without system pkg-config files can follow
[`crates/notae-ocr/README.md`](crates/notae-ocr/README.md) and set
`PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig` for Cargo commands.

The exact local gates run in CI; commands and Android target setup are listed
in [`docs/engineering-standards.md`](docs/engineering-standards.md):

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
cargo llvm-cov --workspace --summary-only --fail-under-lines 84
cargo run -p notae-index-cli -- --help
```

`notae-ocr` is the only crate that requires Tesseract and Leptonica headers.
The Android arm64 target has a separate CI compile gate; installing and
visually checking the app on the Galaxy Tab S10 FE+ remains a manual step.

## Run

```sh
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig \
  cargo run -p notae-index-cli -- /path/to/library
```

The binary writes a SQLite index and a sidecar manifest under
`<library>/.notae/`. See [`apps/notae-index/README.md`](apps/notae-index/README.md)
for the full usage.

## Where to read more

* [`apps/notae-index/README.md`](apps/notae-index/README.md) —
  Linux indexer user guide, prerequisites, invocation, sidecar
  format, and the Android import contract.
* [`crates/notae-ocr/README.md`](crates/notae-ocr/README.md) —
  the dev-env setup for building against a system Tesseract and
  Leptonica on Fedora / RHEL.
* [`docs/android-debugging.md`](docs/android-debugging.md) — safe crash-log
  capture and native debugging for the Samsung Android app.
* [`docs/engineering-standards.md`](docs/engineering-standards.md) — local
  quality commands, Android target gate, and standards audit.
## Target device

The Android side targets the **Samsung Galaxy Tab S10 FE+** with
the latest Samsung keyboard cover. The desktop side targets a
Linux box the user keeps near the printer; it does not have to
be the same hardware across users.
