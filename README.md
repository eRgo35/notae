<div align="center">

<img src="docs/images/notae-banner.svg" alt="Notae: local-first sheet-music library for Android tablets" width="720">

**Index your sheet-music PDFs on a Linux desktop, then browse and search them
on an Android tablet. No cloud, no account, no network.**

[![CI](https://github.com/eRgo35/notae/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/eRgo35/notae/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/eRgo35/notae)](https://github.com/eRgo35/notae/releases/latest)
[![MIT license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.98.1](https://img.shields.io/badge/Rust-1.98.1-orange?logo=rust)](rust-toolchain.toml)
[![Android 6.0+ (API 23), arm64](https://img.shields.io/badge/Android-6.0%2B%20%7C%20arm64-3DDC84?logo=android&logoColor=white)](#install)

[Download](https://github.com/eRgo35/notae/releases/latest) ·
[Quick start](#quick-start) ·
[Documentation](#documentation) ·
[Issues](https://github.com/eRgo35/notae/issues) ·
[Contributing](CONTRIBUTING.md)

<img src="docs/images/screenshot-library.png"
     alt="Notae Library in light theme: a collapsible sidebar and a grid of four score cards with first-page thumbnails"
     width="860">

<sub>Android emulator capture with a non-sensitive fixture library.</sub>

</div>

## Why Notae

Musicians who keep a few hundred scanned or exported scores as plain PDFs
quickly run into the limits of a file manager: names are the only index, and
nothing can search the printed text inside a scan. Notae adds a fast, private
catalogue on top of your existing folder, and leaves the PDFs exactly where
they are.

- **Your files stay yours.** PDFs are never copied, moved, renamed or edited.
  The index lives elsewhere: a `.notae/` sidecar folder written by the desktop
  indexer, and the Android app's private storage.
- **Fully local.** The Android app declares no permissions, including no
  network access, and contains no telemetry or accounts. See
  [Privacy & security](#privacy--security).
- **Read and annotate in the app you already use.** Notae does not render or
  annotate scores itself; it hands the PDF to your PDF viewer, preferring
  Samsung Notes when installed.

## Features

**Desktop indexer (`notae-index`, Linux)**

- Walks a folder of PDFs, parses `<number> - <title>.pdf` file names, and
  extracts the embedded text layer.
- Runs Tesseract OCR on image-only (scanned) pages, using every installed
  Tesseract language pack and picking the best match per page, so mixed
  English/Polish/Latin libraries work.
- Writes a portable, versioned **sidecar** (`.notae/index.sqlite` and
  `manifest.json`) and re-indexes incrementally by SHA-256 content hash, so a
  renamed file stays one record.

**Android app (tablet-first)**

- Browse the library as a **card grid** or a dense **list**, with a collapsible
  sidebar, a **Recent** section and a **full-width preview** with
  previous/next browsing. Light and dark themes follow the system setting.
- **Search** titles, composers, tags and lyrics (the printed text), scoped with
  chips, with highlighted lyric snippets. `Ctrl K` focuses search when a
  hardware keyboard is attached; a microphone action appears when Android
  provides a speech recognizer.
- **Edit details** (number, title, composer, tags) inside Notae; manual edits
  are preserved when you re-import a sidecar.
- **Import preview** for sidecars: see what is new, changed or missing before
  anything is applied. Notae never imports automatically.
- **Open scores in your PDF viewer** (Samsung Notes first, then the Android PDF
  chooser).
- Per-file failures never abort a scan; duplicate numbers are reported, not
  silently overwritten.

**Polish letters and search.** The search index is SQLite FTS5 with the
`unicode61` tokenizer, which folds combining diacritics: `ą ć ę ń ó ś ź ż`
match their plain counterparts. `ł` has no Unicode decomposition and does
**not** match `l`; type `ł` (or the full word) to find titles that contain it.

## Screenshots

<table>
  <tr>
    <td width="50%">
      <img src="docs/images/screenshot-search.png"
           alt="Search with the query Hex: the result shows the matching lyric text highlighted, with scope chips All, Titles, Composers, Lyrics and Tags">
      <br><sub><b>Search</b> across titles, composers, tags and lyrics.</sub>
    </td>
    <td width="50%">
      <img src="docs/images/screenshot-preview.png"
           alt="Full-width score preview showing the first page, the title, number and file name, and an Open score button">
      <br><sub><b>Preview</b> with details and an <i>Open score</i> action.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/images/screenshot-library-dark.png"
           alt="Notae Library in dark theme with the sidebar and four score cards">
      <br><sub><b>Dark theme</b> (captured before the 0.2.1 UI changes).</sub>
    </td>
    <td width="50%">
      <img src="docs/images/screenshot-library.png"
           alt="Notae Library in light theme with the sidebar and four score cards">
      <br><sub><b>Library</b> grid in the light theme.</sub>
    </td>
  </tr>
</table>

<sub>All screenshots are real captures from an Android 15 tablet **emulator**
(2560×1600) using non-sensitive fixture scores, not from a physical tablet.</sub>

## How it works

The desktop and the tablet never need to share a filesystem or a network. They
exchange a versioned sidecar that lives in the library folder.

```mermaid
flowchart LR
    subgraph desktop["Linux desktop"]
        pdfs[("PDF library")]
        idx["notae-index<br/>catalog, text layer, Tesseract OCR"]
        pdfs --> idx
    end

    idx -->|writes| sidecar[".notae/<br/>index.sqlite + manifest.json"]
    pdfs -. "copy folder (USB, cloud drive, ...)" .-> tablet_files
    sidecar -. "copy with the PDFs" .-> tablet_files

    subgraph tablet["Android tablet"]
        tablet_files[("Library folder<br/>PDFs + .notae/")]
        app["Notae<br/>scan, import preview,<br/>browse, search, edit details"]
        viewer["Your PDF viewer<br/>(Samsung Notes, ...)"]
        tablet_files -->|"folder grant (SAF)"| app
        app -->|"open score"| viewer
    end
```

1. `notae-index` catalogs the folder and writes `.notae/` into it.
2. You move the folder (PDFs and `.notae/`) to the tablet however you like.
   Notae itself never syncs or uploads anything.
3. On the tablet you grant Notae access to that folder and scan it. Notae
   builds its own private index, detects the sidecar, and shows an import
   preview. Nothing is applied until you confirm.

On Android 15 (API 35) and later Notae can also read a PDF's existing text
layer by itself; scanned PDFs get searchable text from the desktop sidecar.
Notae does not run OCR on the tablet yet ([#28](https://github.com/eRgo35/notae/issues/28)
proposes it).

## Install

Notae ships one Android artifact: a signed **arm64** APK (Android 6.0 / API 23
or newer). The desktop indexer is built from source; see
[Build from source](#build-from-source).

1. Download `notae-<version>-arm64.apk` and its `.sha256` from the
   [latest release](https://github.com/eRgo35/notae/releases/latest).
2. Verify the download:

   ```sh
   sha256sum -c notae-0.2.0-arm64.apk.sha256        # replace with your version
   apksigner verify --print-certs notae-0.2.0-arm64.apk
   ```

   `apksigner` ships with the Android SDK build tools. The line
   `Signer #1 certificate SHA-256 digest` must equal the Notae release key
   fingerprint (published in
   [`docs/android-release.md`](docs/android-release.md) and pinned in the
   release workflow):

   ```text
   BE:27:CD:78:17:66:C5:C2:8A:9D:B1:06:C0:7B:05:1C:EE:E4:E0:00:31:05:96:45:95:E2:A8:48:F3:71:F6:01
   ```

   `apksigner` prints the digest without colons, in lowercase hex; compare it
   ignoring case and separators.
3. Copy the APK to the tablet and open it (allow installs from your file
   manager or browser when Android asks), or use
   `adb install notae-0.2.0-arm64.apk`.

> [!WARNING]
> **Debug-key installs cannot be updated in place.** Android refuses to update
> an app signed with a different key. If you installed a development (debug)
> build, uninstall it first. Uninstalling **deletes Notae's private data**
> (preferences, the folder grant, the local index and any metadata edits). The
> PDFs are untouched and the index is rebuilt by scanning again. Releases are
> signed with one release key, so upgrades between releases work in place.

The v0.2.0 release has been verified on an Android emulator and with host
tests, but not yet accepted on a physical tablet (see the
[release notes](docs/release-notes/v0.2.0.md)): please report anything odd.

## Quick start

**1. Index a folder on your Linux desktop.** Name PDFs `<number> - <title>.pdf`
(for example `12 - Autumn Leaves.pdf`) to get an ordering number and title for
free; other names still work and simply have no number.

```sh
cargo build --release -p notae-index-cli       # see "Build from source" for prerequisites
./target/release/notae-index /path/to/library  # writes /path/to/library/.notae/
```

Install at least one Tesseract language pack (for example
`tesseract-langpack-eng`, plus `tesseract-langpack-pol` for Polish). Re-run the
command whenever the library changes; it is incremental. Details, options and
the sidecar format are in [`apps/notae-index/README.md`](apps/notae-index/README.md).

**2. Copy the folder to the tablet.** Copy the PDFs together with the
`.notae/` directory (USB, a file-sharing app, adb push, ...). Keep the same
relative paths.

**3. Open Notae and choose the folder.** Tap **Choose score folder**, pick the
folder in the Android picker and allow access, then start a scan from the
Library. Notae indexes the PDFs and, when it finds `.notae/manifest.json`, shows
an import preview; confirm to import the OCR text and metadata.

## Using Notae

- **Browse:** All scores and Recent live in the sidebar (or the chip row on a
  narrow window). Switch between grid and list with the header button; hide the
  sidebar with the panel button. Both choices are remembered, the sidebar
  separately for portrait and landscape.
- **Open:** tap a score to open it in your PDF viewer or to see its details; the
  preview shows the first page and metadata, with previous/next buttons.
- **Search:** tap the search icon (or `Ctrl K`), type, and narrow by scope
  chip. Lyric matches show a snippet.
- **Edit details:** change number, title, composer and tags from a score's
  details. Edits live in Notae's private index only; they never rename the
  PDF.
- **Rescan:** use the Scan action in the Library header after adding files.
  Notae never scans in the background.

## Build from source

<details>
<summary><b>Prerequisites</b></summary>

- **Rust 1.98.1**, pinned by [`rust-toolchain.toml`](rust-toolchain.toml)
  (rustup selects it automatically, including `rustfmt` and `clippy`).
- **Linux native libraries** for the indexer's OCR and PDF stack: `pkg-config`,
  Tesseract and Leptonica headers, Fontconfig headers and an English
  Tesseract language pack. On Ubuntu:

  ```sh
  sudo apt-get install pkg-config libtesseract-dev libleptonica-dev \
      libfontconfig1-dev tesseract-ocr-eng
  ```

  On Fedora/RHEL hosts without system `pkg-config` files for Tesseract, follow
  [`crates/notae-ocr/README.md`](crates/notae-ocr/README.md) and set
  `PKG_CONFIG_PATH`.
- **For the Android app:** JDK 17 (a newer JDK works if you compile the Java
  adapter as Java 17; see [`docs/android-debugging.md`](docs/android-debugging.md)),
  Android SDK platform 34 with build-tools 34.0.0, **NDK r26d**
  (`26.3.11579264`), the `aarch64-linux-android` Rust target and
  `cargo-apk2` 1.4.0.

</details>

<details>
<summary><b>Build and run the indexer</b></summary>

```sh
cargo build --release -p notae-index-cli
./target/release/notae-index --help
```

</details>

<details>
<summary><b>Build the Android app</b></summary>

```sh
rustup target add aarch64-linux-android
cargo install --locked --version 1.4.0 cargo-apk2
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/26.3.11579264"

cargo apk2 check -p notae-android --target aarch64-linux-android   # compile gate used in CI
cargo apk2 build -p notae-android --target aarch64-linux-android   # debug-signed APK
```

Debug builds are signed with your Android SDK debug key, so they are **not**
update-compatible with the official release APK. Release builds need the
maintainer's signing key and are produced only by the
[Release workflow](docs/android-release.md). For an emulator, build with
`--target x86_64-linux-android` and see
[`docs/android-debugging.md`](docs/android-debugging.md) for AVD setup.

</details>

<details>
<summary><b>Quality gates</b></summary>

The same commands run in CI (details in
[`docs/engineering-standards.md`](docs/engineering-standards.md)):

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

</details>

## Project layout

```text
crates/
  notae-domain/     Filename convention, note metadata, file identity.
  notae-index/      SQLite persistence, FTS5 search, the sidecar format and
                    the importer the Android app uses.
  notae-ocr/        Tesseract-backed OCR provider (Linux).
apps/
  notae-index/      The `notae-index` command-line indexer.
  notae-android/    The Android app: Slint UI, Storage Access Framework
                    adapter and a thin Java activity shim.
docs/               Release procedure, engineering standards, debugging guide,
                    design spec and plans.
```

The app is written in Rust with a [Slint](https://slint.dev) UI, packaged with
`cargo-apk2`. Domain logic, indexing, search and import live in the host-tested
crates; the Android crate only adds platform glue. The product scope and
architecture are in the
[design spec](docs/superpowers/specs/2026-10-09-notae-design.md).

## Privacy & security

- **No network.** The Android manifest requests no permissions at all, so the
  app has no `INTERNET` permission, and none of the crates in the Android
  build's dependency graph is an HTTP or socket library. There are no accounts
  and no telemetry.
- **No broad storage access.** Notae reads only the folder you pick through
  Android's Storage Access Framework, and only reads from it.
- **PDFs are never modified.** They are not copied, moved, renamed or
  rewritten. The index and thumbnails live in the app's private storage.
- **Handing a score to a viewer** grants that app temporary read access to that
  one PDF.
- **Voice search** is optional: it only launches Android's installed speech
  recognizer. That recognizer is not part of Notae and may use network services
  independently; Notae requests no microphone permission.
- **Signed releases.** APKs are built in CI from version tags and signed with a
  dedicated key whose certificate fingerprint is published (see
  [Install](#install)).

Found a vulnerability? Please follow [`SECURITY.md`](SECURITY.md) and report it
privately.

## Documentation

- [`apps/notae-index/README.md`](apps/notae-index/README.md): indexer guide,
  sidecar format, Android import contract.
- [`crates/notae-ocr/README.md`](crates/notae-ocr/README.md): building against
  system Tesseract and Leptonica.
- [`docs/android-release.md`](docs/android-release.md): release workflow,
  signing and verification.
- [`docs/android-debugging.md`](docs/android-debugging.md): emulator setup,
  crash-log capture and the UI verification checklist.
- [`docs/engineering-standards.md`](docs/engineering-standards.md): quality
  gates and standards audit.
- [`docs/release-notes/`](docs/release-notes/) and
  [`CHANGELOG.md`](CHANGELOG.md): what changed in each release.

## Contributing

Bug reports, ideas and pull requests are welcome. Start with
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the development setup, quality gates,
commit conventions and what must never be committed.

## Roadmap and known limitations

Work is tracked in [GitHub issues](https://github.com/eRgo35/notae/issues) and
milestones; the next one is
[v0.2.1](https://github.com/eRgo35/notae/milestone/2).

- **Not yet accepted on a physical tablet.** The target device is the Samsung
  Galaxy Tab S10 FE+. So far, everything has been verified with host tests and
  an Android emulator; Samsung Keyboard and Gboard input, the launcher icon and
  motion on the real device still need confirmation.
- **No TalkBack support yet.** Slint's Android backend does not publish an
  accessibility tree, so TalkBack cannot operate item actions and menus
  ([#17](https://github.com/eRgo35/notae/issues/17)).
- **No on-device OCR.** Scanned PDFs get searchable text from the desktop
  indexer; an automatic, local on-device option is proposed in
  [#28](https://github.com/eRgo35/notae/issues/28).
- **Physical-keyboard Polish input** is being fixed in
  [#32](https://github.com/eRgo35/notae/issues/32).
- **Platforms:** the Android release is arm64 only (minSdk 23); the indexer is
  Linux only. There is no iOS, Windows or macOS build.
- **No in-app PDF viewing or annotation**, and no cloud sync, by design.
- Preview thumbnails are 400 px wide, so the full-width preview can look soft
  on a large tablet.

## License

Notae is released under the [MIT License](LICENSE). Third-party dependencies,
including the [Slint](https://slint.dev) UI toolkit, keep their own licenses;
the accepted set is enforced by `cargo deny` ([`deny.toml`](deny.toml)).
