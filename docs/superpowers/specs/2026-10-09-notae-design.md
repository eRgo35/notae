# Notae Local Music-Notes Library Design

**Status:** Approved design; cargo-apk2 bridge approved 2026-10-10; nonblocking picker execution clarification pending review
**Target device:** Samsung Galaxy Tab S10 FE+
**Implementation language:** Rust-first with a thin Java NativeActivity result adapter
**Original date:** 2026-10-09

## Goal

Build a tablet-first Android application for a fully local library of music-note PDFs. Notae indexes manually prepared PDFs, searches filenames, metadata, and printed OCR text, and delegates PDF viewing to Samsung Notes when available.

## Product boundaries

### MVP includes

- One user-selected PDF library root.
- Recursive PDF discovery under that root.
- Manual rescan only; no launch rescan and no background watcher.
- Numeric ID parsing from filenames.
- Editable title, ID, composer/artist, and tags.
- Globally unique optional numeric IDs.
- Numeric-ID-first browsing order.
- Full-text search over indexed metadata and printed OCR text.
- Android extracts embedded PDF text when the platform API supports it. Image-only PDFs remain OCR-pending until OCR text is imported from the Linux sidecar.
- Linux PC indexing with a portable sidecar index.
- Sidecar import preview and explicit confirmation.
- Samsung Notes handoff with Android PDF chooser fallback.
- Local-only operation with no cloud service, network requirement, or telemetry.

### Deferred

- Automatic splitting of a NAPS2 batch PDF.
- Automatic organization, renaming, or moving of PDFs.
- In-app PDF rendering, thumbnails, zoom, annotation, or page navigation.
- Handwriting recognition.
- Multiple library roots or library profiles.
- Background monitoring.
- Freeform notes, source/provenance fields, and custom metadata fields.
- Cloud synchronization.

The user manually scans, divides, names, and places individual PDFs before Notae indexes them.

## User-visible behavior

### Library source of truth

The user selects one root folder through Android's Storage Access Framework. PDFs remain in that folder and are never copied, renamed, or moved by Notae.

Subdirectories are storage locations only. They are not collections, categories, or search facets.

Notae persists the selected folder permission and uses the root for explicit manual rescans. The exact Android/One UI version on the target tablet is validated during implementation; the product does not promise broad Android compatibility in the first release.

### Filename convention

The preferred filename format is:

```text
<ID> - <Title>.pdf
```

Examples:

```text
12 - Autumn Leaves.pdf
507 - My Practice Arrangement.pdf
Autumn Leaves.pdf
```

The parser extracts an ID only when a leading integer is followed by a separator and title. A filename without that pattern has no ID. The remaining filename stem initializes the title.

The numeric ranges currently used by the user (`1-299` for the main catalogue and `500-599` for custom notes) are conventions, not hard-coded validation rules. Any non-negative integer is valid if it is unique.

### ID invariant

At most one indexed PDF may have a given numeric ID across the whole library. A duplicate ID is an import conflict. Notae MUST NOT silently overwrite, renumber, or hide either record.

A duplicate-ID conflict requires an explicit resolution:

- keep the existing record and reject the new record;
- replace the existing record after confirmation; or
- edit the new record's ID before applying it.

### Metadata

The MVP metadata model is:

- `id`: optional globally unique integer;
- `title`: required, initialized from the filename;
- `composer_or_artist`: optional text;
- `tags`: zero or more user-defined labels.

Metadata is editable inside Notae and is stored separately from the PDF filename. Manual metadata edits are authoritative and are preserved across later PC index imports unless the user explicitly selects replacement in an import preview.

### Browsing order

The default library order is:

1. records with IDs, by numeric ID ascending;
2. records without IDs, alphabetically by title.

Filesystem path and import time do not affect the default order.

### Search

The default search query matches:

- numeric ID;
- title;
- composer/artist;
- tags;
- filename;
- OCR text.

Exact ID and title matches rank above broad OCR-only matches. Search must still return a PDF when its only match is printed text extracted by OCR.

### Main screen

The tablet-first library screen contains:

- a prominent search field;
- the ordered result list;
- result rows showing ID, title, composer/artist, tags, and index/OCR status;
- an explicit **Rescan** action;
- Settings;
- empty, permission, and error states.

The visual language is minimal and Material-3-inspired: clear hierarchy, generous tablet spacing, readable typography, restrained color, touch-sized controls, and no decorative navigation that competes with search.

### Opening PDFs

Notae renders only the first page of each PDF, once, into a private thumbnail cache for the library grid. Viewing a score is still delegated to an installed PDF app.

When the user opens a result, the Android bridge:

1. obtains a readable content URI;
2. prefers Samsung Notes when it can handle the PDF intent;
3. grants temporary read permission;
4. falls back to the Android PDF chooser if Samsung Notes is unavailable;
5. reports a clear error if no compatible viewer exists.

### Manual rescan

Notae performs no automatic scan on launch and no background file monitoring. The user starts every rescan explicitly.

A rescan:

1. enumerates PDFs under the selected root;
2. reuses records whose content fingerprint is unchanged;
3. detects additions, deletions, path moves, and changed content;
4. parses filename metadata for new files;
5. reads existing PDF text when available;
6. marks image-only or otherwise unsearchable PDFs OCR-pending for Linux OCR and sidecar import;
7. reports duplicate IDs and per-file failures;
8. detects `.notae/index.sqlite` for optional PC-index import.

One invalid or failed PDF MUST NOT abort the entire rescan.

### PC sidecar import

The Linux indexer writes a self-contained sidecar under the selected root:

```text
Music Notes/
  12 - Autumn Leaves.pdf
  507 - My Practice Arrangement.pdf
  .notae/
    index.sqlite
    manifest.json
```

Notae detects the sidecar during a manual rescan but never applies it automatically. It shows an import preview containing:

- new entries;
- changed entries;
- unchanged entries;
- missing entries;
- hash mismatches;
- OCR availability;
- duplicate-ID conflicts;
- metadata that will be preserved locally;
- metadata proposed by the sidecar.

The user explicitly confirms before applying the import.

File identity is based on SHA-256 content hash. A path or filename change with the same hash updates the stored path without creating a new note.

Imported OCR may replace missing or stale local OCR. Manually edited metadata remains authoritative by default. Imported metadata may be applied only when the corresponding local field has not been manually edited or the user explicitly chooses replacement.

### Indexing status

Each PDF can expose a status such as:

- discovered;
- indexing;
- ready;
- OCR pending;
- OCR incomplete;
- stale;
- duplicate-ID conflict;
- inaccessible;
- invalid PDF;
- import mismatch.

Statuses are visible in the rescan/import UI and do not prevent other valid records from being used.

## Technical architecture

The repository becomes a Rust workspace with focused crates:

```text
notae/
  crates/
    notae-domain/    # IDs, metadata, fingerprints, invariants
    notae-index/     # SQLite schema, FTS search, reconciliation
    notae-ocr/       # OCR provider interface and job orchestration
    notae-pdf/       # PDF enumeration and text/page extraction boundary
  apps/
    notae-android/   # Slint UI and Android bridge
    notae-index/     # Linux PC indexing CLI
```

### UI and Android boundary

The UI is implemented in Slint and driven by Rust application state. A small Android bridge handles only platform concerns:

- Storage Access Framework folder selection and persisted permission;
- conversion of SAF document handles into readable content URIs;
- Samsung Notes preference and PDF intent fallback;
- Android lifecycle and platform-specific OCR integration if required by the selected backend.

Domain logic, indexing, metadata validation, search, import reconciliation, and CLI behavior remain in Rust crates independent of the UI.

#### Android activity-result bridge

The Android APK is packaged with `cargo-apk2`, which compiles the
small Java boundary class into the APK and allows the manifest to
name it as the app's activity. The launcher activity is
`NotaeActivity`, a Java subclass of `android.app.NativeActivity`;
the inherited native startup continues to load `libnotae_android`
and call Rust's `android_main` entrypoint.

`NotaeActivity.onActivityResult` calls the superclass implementation,
then forwards the SAF request code, result code, selected URI string,
and granted read-permission flags (or cancellation) to an instance
JNI entrypoint in Rust; the Activity receiver supplies the context
needed for `ContentResolver` calls. Rust dispatches that outcome
through the shared `PickerState`; the Android SAF adapter takes
persistable read permission using the flags returned by the picker
and stores the opaque tree URI before completing root selection.
The UI MUST keep the Slint/Android main event loop responsive while
the picker future is pending. It MUST schedule completion
asynchronously (for example, with `slint::spawn_local`) rather than
blocking the thread that must deliver `onActivityResult`.
Java owns only Android Activity lifecycle/result delivery; indexing,
storage state, and picker outcome handling remain Rust.

Do not modify `ANativeActivityCallbacks` or infer an activity-result
slot: the NDK ABI has no such callback. Keep this boundary narrow and
validate the generated APK activity declaration and result delivery
on the target tablet.

References:
- [`cargo-apk2` README](https://github.com/mzdk100/cargo-apk2) documents JVM source compilation and manifest-declared activities.
- [Android `NativeActivity` API](https://developer.android.com/reference/android/app/NativeActivity) and [NDK callbacks reference](https://developer.android.com/ndk/reference/struct/a-native-activity-callbacks).
- [Storage Access Framework guide](https://developer.android.com/training/data-storage/shared/documents-files).

### Storage

The Android app maintains a local application-owned SQLite database for the active root. The Linux CLI produces the portable `.notae/index.sqlite` using the same logical schema and serialization contract.

The index stores, at minimum:

- schema version;
- root-relative PDF path;
- SHA-256 content hash;
- byte size and observed modification metadata;
- optional numeric ID;
- title;
- composer/artist;
- tags;
- normalized OCR text;
- OCR status and detected-language metadata;
- metadata provenance and manual-edit markers;
- indexing timestamps and tool version.

SQLite FTS5 is the search index. The canonical metadata fields remain ordinary structured columns so ID uniqueness, sorting, and field editing do not depend on tokenization.

The Android app does not directly treat the SAF-backed sidecar file as its live database. It validates and imports sidecar records into its local database. Sidecar generation and import are explicit boundaries.

### OCR

OCR is printed-text-only. Handwritten text and music notation are not searchable in the MVP.

The Rust OCR layer exposes a provider abstraction so the index and UI do not depend on one engine:

```rust
pub trait OcrProvider {
    fn recognize(
        &self,
        page: &Image,
        policy: &LanguagePolicy,
    ) -> Result<OcrText, OcrError>;
}
```

The Linux OCR backend uses Tesseract/Leptonica through Rust because the product requires multilingual language data, including English, Polish, and Latin, plus extensibility to additional installed language packs. Automatic language/script detection is the default behavior; it should select among locally available packs and record uncertainty or failure instead of silently returning no text.

The Android app does not run OCR. It renders first-page thumbnails with `PdfRenderer` during a rescan. On Android API 35 and later, it reads an existing PDF text layer through `PdfRenderer.Page.getTextContents`; on older APIs or image-only documents it preserves imported sidecar OCR or leaves the record OCR-pending. Linux OCR models and language data are local assets, and runtime indexing MUST NOT require a network connection.

**Status (2026-10-18):** [ADR 0001](../../adr/0001-on-device-ocr.md) (Proposed, pending maintainer approval) proposes to supersede "The Android app does not run OCR" for image-only PDFs with local Tesseract recognition on the device (issue #28). Until the ADR is accepted and implemented, the paragraph above remains the shipped behaviour; the evidence and phased plan are in [`2026-10-18-on-device-ocr-design.md`](2026-10-18-on-device-ocr-design.md).

Android visual design follows the [approved One UI canvas](https://claude.ai/artifact/3bQ38pa1r5L3hVXF1M8N6Z) and the fixed light/dark palette in the One UI redesign plan's Global Constraints.

Android layout rules: the window draws edge-to-edge and the UI pads itself by the system-bar insets (no framework action bar). Every configured screen owns its header, and the Scan icon is that header's right-most action on Library, Search and Settings; there is no global bar above the screens. Layout is compact below 840 px of window width (section chips, Settings in the Library header), shows the section side panel from 840 px, and shows list plus details side by side from 1280 px. The status banner reports only the running operation or its outcome; the latest scan result and pending-text count are in Settings. Touch targets are at least 48 px.

### Linux CLI

The PC tool is Linux-only for the first release:

```text
notae-index <library-root>
```

It reuses `notae-domain`, `notae-pdf`, `notae-ocr`, and `notae-index`, processes PDFs locally, and writes `.notae/index.sqlite` plus `manifest.json`. It does not rename, move, or split PDFs.

## Error and privacy requirements

Notae is fully local:

- no cloud OCR;
- no network dependency at runtime;
- no telemetry;
- no upload of PDFs, metadata, or OCR text.

Errors MUST remain attached to the affected file or operation. Required error classes include:

- lost or denied folder permission;
- missing, unreadable, or invalid PDF;
- PDF hash mismatch;
- duplicate ID;
- OCR model or language-pack failure;
- OCR processing failure;
- malformed sidecar schema;
- Samsung Notes unavailable;
- no compatible external PDF viewer.

Error messages must state the next actionable step and must not expose raw internal paths when a user-facing filename is sufficient.

## Verification requirements

The implementation plan must include tests and smoke scenarios covering:

- leading-ID filename parsing and title extraction;
- filenames without IDs;
- numeric sorting, including `2` before `10`;
- duplicate-ID rejection;
- editable metadata surviving a rescan;
- path rename with unchanged SHA-256 preserving one record;
- changed content creating a reindex requirement;
- FTS matches from title, tags, and OCR text;
- sidecar hash validation and import preview;
- preservation of manual metadata over PC-generated metadata;
- one corrupt PDF not aborting a rescan;
- missing Samsung Notes falling back to the Android PDF chooser;
- no-network operation after OCR assets are available.

The first device smoke run must use the Samsung Galaxy Tab S10 FE+ and verify folder selection, explicit rescan, OCR progress, metadata editing, sidecar import preview, and PDF handoff.

## Acceptance criteria

The MVP is complete when a user can:

1. select one local folder on the Galaxy Tab S10 FE+;
2. manually rescan that folder;
3. see individual PDFs ordered by numeric ID;
4. search metadata and printed text from scanned PDFs;
5. edit ID, title, composer/artist, and tags without renaming files;
6. detect and resolve duplicate IDs explicitly;
7. import a Linux-generated `.notae/index.sqlite` after reviewing its changes;
8. open a selected PDF in Samsung Notes or the Android chooser;
9. use the app without network access;
10. continue using valid library records when individual files or OCR jobs fail.
