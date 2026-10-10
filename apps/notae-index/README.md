# `notae-index` — the Linux PDF indexer

`notae-index` is the Linux-side half of the Notae library. It walks a
folder of PDFs, extracts text (and OCRs the image-only ones), and writes
a portable sidecar the Android app imports without needing to redo any of
the work.

The indexer is the only Notae component that runs on the desktop. The
Android app reads the sidecar the indexer wrote and never asks the
desktop to do anything else.

## Quick start

```sh
# Build the indexer (see crates/notae-ocr/README.md for the dev-env setup)
cargo build -p notae-index-cli --release

# Index a folder of PDFs and write the sidecar
./target/release/notae-index /path/to/library

# Inspect the result
ls /path/to/library/.notae/
# → index.sqlite  manifest.json
```

The indexer writes two files inside `<library>/.notae/`:

* `index.sqlite` — the FTS5-indexed SQLite database the indexer uses to
  decide what changed between runs.
* `manifest.json` — the versioned, JSON-encoded description of the library
  the Android app reads. This is the cross-platform contract.

## Prerequisites

The binary is Linux-only because it links Tesseract and Leptonica
through `pkg-config`. The system requirements are:

* **Tesseract 5.x** with development headers (`tesseract-devel` on
  Fedora/RHEL, `libtesseract-dev` on Debian/Ubuntu). The C++ library
  ships as `libtesseract.so` (or `libtesseract.so.5.5.3` on systems
  without an unversioned symlink — the build script handles both).
* **Leptonica 1.8x** with development headers (`leptonica-devel` /
  `libleptonica-dev`).
* **Rust 1.85+** (edition 2024).
* **`pkg-config`** on `PATH`.

On a fresh Fedora 44 box without sudo this is a no-go because the
tesseract and leptonica packages cannot be installed. See
[`crates/notae-ocr/README.md`](../../crates/notae-ocr/README.md) for the
dev-env shim that makes the crate build on this kind of host.

## Local OCR language data

Tesseract does not ship language data; the indexer discovers every
`*.traineddata` file in the **tessdata directory** and uses whatever it
finds. The tessdata directory is chosen in this order:

1. `--tessdata-dir <path>` on the command line.
2. The `TESSDATA_PREFIX` environment variable.
3. `/usr/share/tesseract/tessdata` (the system default on most
   distributions).

```sh
# Install at least English
sudo dnf install tesseract-langpack-eng

# Optionally, Polish
sudo dnf install tesseract-langpack-pol

# Index a mixed English/Polish library
TESSDATA_PREFIX=/usr/share/tesseract/tessdata \
  notae-index /path/to/library
```

The indexer runs with an *automatic* language policy: every pack present
in the tessdata directory is loaded, and the language detector picks the
best one for each page. With only `eng.traineddata` installed every
recognized page is treated as English; with both `eng` and `pol`
installed, pages are recognized under both packs and Tesseract reports
the higher-confidence one.

If the tessdata directory is empty or missing, the indexer exits
non-zero with a typed error:

```text
notae-index: no tesseract language data at /path/to/tessdata;
install eng.traineddata or set TESSDATA_PREFIX
```

This is a hard failure, not a warning. A run that produced a sidecar
without any language data would not be the run the user asked for.

## Invocation

```text
notae-index [OPTIONS] <LIBRARY_ROOT>

Arguments:
  <LIBRARY_ROOT>              The folder whose PDFs are indexed

Options:
  -v, --verbose               Print per-file progress to stderr
      --tessdata-dir <DIR>    Tessdata directory (overrides
                              TESSDATA_PREFIX and the system default)
  -h, --help                  Print help
  -V, --version               Print version
```

The library root is validated exactly once, before anything is read. A
root that is missing, is a regular file, is unreadable, or is a
symbolic link however it is spelled, is refused with a non-zero exit
status and a clear message on stderr.

## Reruns

A second run against the same library is *incremental*. The SQLite
database remembers every record from the previous run, and the
reconciler classifies each catalogued file as one of:

* **new** — a file that was not in the index before; the indexer
  extracts text and writes a record.
* **unchanged** — a file whose SHA-256 matches the record already at
  the same path; the indexer reuses the existing row.
* **moved** — a file whose SHA-256 matches a record at a *different*
  path; the record is rewritten at the new path and not duplicated.
* **updated** — a file whose SHA-256 has changed since the last run;
  the old record is replaced.
* **removed** — a record whose file is no longer in the library; the
  row is deleted.

A rename is therefore a single record at the new path, never two
records for the same content. The rename test in
`tests/sidecar_output.rs` pins this contract.

The summary line lists what happened:

```text
indexed 12 of 13 PDFs under /path/to/library (1 PDF failures)
```

`readable / total` is the number of files the catalog could read; the
`(...)` extras surface what the run could not finish. The exit code is
zero — a run that produced a report, even one with per-file failures,
is a successful run.

## The sidecar format

The manifest is a single JSON file with three top-level fields:

* `schema_version` — the version of the wire format. The current
  version is `2`; an older or newer manifest is refused by the
  Android importer rather than guessed at.
* `generated_at` — seconds and nanoseconds since the epoch, the
  moment the sidecar was written.
* `tool_version` — the `CARGO_PKG_VERSION` of the build that wrote the
  sidecar.
* `entries` — one record per indexed PDF, with:
  * `relative_path` — the file's path relative to the library root,
    using forward slashes on every platform.
  * `fingerprint` — the SHA-256 of the file's bytes and the byte
    length. File identity is the content hash, so a renamed file
    carries the same hash.
  * `metadata` — the note ID, title, composer/artist, tags, and
    per-field provenance the indexer derived from the filename or
    left for the user to edit on the phone.
  * `status` — the indexer's verdict on this record (`discovered`,
    `invalid_pdf`, `inaccessible`, `duplicate_id_conflict`, …).
  * `ocr` — the recognized text and the language Tesseract detected,
    when the record has any OCR.

The manifest is sorted by path so a re-run over an unchanged library
produces the same entries in the same order. The only field that
changes between runs is `generated_at`; the importer treats it as
informational.

## Android import

The Android app reads the manifest through `SidecarImporter::preview`,
which compares each entry against the files the Android device has
under the same path and classifies the difference:

* **new** — the Android library does not have a record at this path.
* **unchanged** — the file is here, the SHA-256 matches, and the
  metadata is what the manifest says.
* **changed** — the file is here but the SHA-256 differs. The
  manifest's metadata is *proposed*; the user decides whether to
  overwrite the local edit.
* **absent** — the manifest names a file the Android library does not
  have. Nothing is deleted locally; the missing file is reported in
  the import preview.
* **hash mismatch** — the local file's bytes differ from the
  manifest's content hash.

The preview is read-only: it touches no row in the local index until
the user confirms. The integration test
`sidecar_importer_preview_accepts_the_run_output` in
`tests/sidecar_output.rs` exercises this against a fresh
`IndexStore` to prove the indexer produces a manifest the importer
can read.

## Exit codes

| Code | Meaning                                                                |
| ---- | ---------------------------------------------------------------------- |
| 0    | An informational request (`--help`, `--version`) or a run that produced a `RunReport`, even one with per-file failures. |
| non-zero | The library root is unusable, the tessdata directory is missing, or the sidecar could not be written. |

A run that cannot read one specific PDF is **not** a non-zero exit; the
failure is reported on stderr and counted in the summary line. The
dividing line is the library root itself: a root the process could not
see is a command failure; a PDF under a readable root is a per-file
problem.

## Where to look next

* The integration tests in `tests/sidecar_output.rs` are the contract.
* The pipeline tests in `tests/pipeline.rs` exercise the per-file
  classification.
* The CLI argument tests in `tests/cli_args.rs` and
  `tests/exit_codes.rs` cover the root-validation policy.
* The PDF handling tests in `tests/pdf_fixtures.rs` document the
  fixtures the integration tests use.
