# Notae

Notae is a personal music-library manager. The library lives on a
desktop (a Linux box that runs the indexer); the user reads,
annotates, and searches it on a tablet (a Galaxy Tab S10 FE+ running
Android).

The desktop and the tablet do not share a file system. They share
the **sidecar**: a versioned JSON manifest the indexer writes
beside its SQLite payload, and the Android app reads before
importing anything.

This repository holds the desktop half of the system — the
indexer, the OCR layer, the SQLite store, the sidecar format, and
the test fixtures the integration tests pin the contract on. The
Android app lives in a separate plan and a separate repository.

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
```

## Build

```sh
# Build everything that is not the OCR crate
cargo build --workspace --exclude notae-ocr

# Build the OCR crate (requires the Tesseract/Leptonica dev shim)
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo build --workspace

# Run the test suite
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo test --workspace

# Lint
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo clippy --workspace --all-targets -- -D warnings
```

`notae-ocr` is the only crate that needs Tesseract and Leptonica
on the build host. The other crates are pure Rust and build
without any system dependencies beyond `pkg-config` itself
(through `rusqlite`'s bundled SQLite).

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
* `docs/` — the spec and the Linux-plan documents that drove
  the implementation.

## Target device

The Android side targets the **Samsung Galaxy Tab S10 FE+** with
the latest Samsung keyboard cover. The desktop side targets a
Linux box the user keeps near the printer; it does not have to
be the same hardware across users.
