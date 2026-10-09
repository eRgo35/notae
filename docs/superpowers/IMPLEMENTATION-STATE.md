# Notae Implementation State

**As of:** 2026-10-09
**Branch:** `main`
**Last committed commit:** `5d3de0b feat: extract and render Linux PDFs`
**Remote:** `git@github.com:eRgo35/notae.git`

This document records where the project actually stands so work can resume without re-deriving context. It is a status record, not a design document.

## Documents

| Document | Path | Status |
|---|---|---|
| Product and architecture spec | `docs/superpowers/specs/2026-10-09-notae-design.md` | Approved |
| Core plan (Tasks 1-5) | `docs/superpowers/plans/2026-10-09-notae-core-index-plan.md` | Complete |
| Linux indexer plan (Tasks 1-5) | `docs/superpowers/plans/2026-10-09-notae-linux-indexer-plan.md` | Task 1 complete, Task 2 in progress |
| Android app plan (Tasks 1-7) | `docs/superpowers/plans/2026-10-09-notae-android-plan.md` | Not started |

## Product decisions already settled

These came from user decisions during brainstorming. Do not reopen them without asking.

- PDFs live in a user-selected folder chosen through the Storage Access Framework; Notae never copies, renames, or moves them.
- Automatic splitting of a NAPS2 batch is **deferred**. The user divides and names scans manually.
- One active library root. Filesystem folders are storage locations, not collections.
- Numeric ID parsed from a leading `<ID> - <Title>.pdf` convention. The user's ranges `1-299` and `500-599` are conventions, not validation rules.
- One PDF per numeric ID, globally unique. Duplicates are surfaced as conflicts and never auto-resolved.
- Default order: numeric ID ascending, then unnumbered by title.
- Editable metadata: ID, title, composer/artist, tags. Filenames are never changed to match.
- Manual edits are authoritative over imported metadata, tracked per field.
- Search covers ID, title, composer/artist, tags, filename, and OCR text.
- OCR is printed text only, with automatic language detection. Handwriting is excluded.
- Linux CLI writes a portable `.notae/index.sqlite` plus `manifest.json`; Android shows an import preview and never applies it automatically.
- PDF viewing is delegated to Samsung Notes, falling back to the Android chooser.
- Target device: Samsung Galaxy Tab S10 FE+.
- Fully local: no cloud, no telemetry, no network at runtime.

## Workspace layout

```
crates/notae-domain    # pure domain model, no I/O dependencies
crates/notae-index     # SQLite + FTS5 store, reconciliation, sidecar import
apps/notae-index-cli   # Linux CLI (package name notae-index-cli)
apps/notae-android     # NOT CREATED YET
```

## Completed: core plan

Every task below passed implementation, review, and fix rounds. The core is the foundation for both remaining plans.

### Task 1 - domain model (`d8f82aa`, `c40b937`, `52eab35`)

Exports `NoteId`, `NoteMetadata`, `PdfFingerprint`, `Sha256Digest`, `EpochTimestamp`, `RelativePdfPath`, `MetadataField`, `FieldProvenance`, `MetadataOrigin`, `IndexStatus`, `DomainError`.

Key invariants established during review:
- Deserialization routes through validation. `RelativePdfPath` and `NoteMetadata` use manual `Deserialize`; a sidecar cannot inject an escaping path, a blank title, or unnormalized tags.
- Provenance is per field, not per record. `set_*` marks `Manual` even when clearing a value; `assign_*(value, origin)` sets an explicit origin for filename parsing and imports; `set_origin` and `mark_manual` change provenance without changing a value. `is_manual(field)` is the predicate reconciliation and sidecar import use.
- `NoteMetadata` deserialization requires provenance, with an explicit migration from the legacy scalar `origin` field.
- `EpochTimestamp { secs: i64, nanos: u32 }` replaces `SystemTime` in the serde path so pre-epoch instants round-trip; range handling uses checked arithmetic.

### Task 2 - filenames, ordering, IDs (`4f5b677`, `0741c0c`, `6bde356`)

- `parse_filename` never rejects; `is_note_filename` and `parse_note_filename` plus `FilenameError::NotPdf` give callers an explicit signal so a non-PDF cannot become an indexed note.
- ID requires a leading integer, a `-` separator, and a non-empty title. Embedded numbers are never extracted.
- `compare_library_order` is a documented total order across every `IndexedNote` field, so `sort_by` is deterministic even with duplicate records.
- `IndexedNote` carries `IndexStatus`, default `Discovered`.
- `validate_unique_ids` reports `DuplicateId { id, existing_path, conflicting_path }`.

### Task 3 - SQLite and FTS5 (`1e7043d`, `6931371`)

`notes` table is STRICT with unique `path` and unique `sha256`, a partial unique index on non-null `note_id`, four provenance columns, a status column, and `ocr_text`/`ocr_language`. A derived `notes_fts` FTS5 table indexes `note_id`, `title`, `composer`, `tags`, `filename`, `ocr_text` with `unicode61 remove_diacritics 2`. `schema_migrations` tracks versions.

The `note_id` FTS column is a deliberate deviation from the plan's five columns: without it, a record whose ID came from metadata rather than its filename would be unreachable by typing the number.

`ReindexOutcome { Inserted, Unchanged, Moved { from }, Replaced { previous, duplicate_at } }` distinguishes same-hash moves from same-path content changes. Schema validation rejects a database whose declared version does not match its actual objects. Migrations run in one transaction including the bookkeeping table.

### Task 4 - reconciliation (`d97d1a8`, `1749165`, `c395d7e`, `cf25b3f`)

`PdfCatalog` is object-safe with no filesystem coupling, so Android can supply a SAF-tree implementation. `PdfFile` carries `FileCondition { Readable(PdfFingerprint), Unreadable, InvalidPdf }`. `Reconciler::plan` is read-only and deterministic; `Reconciler::apply` writes and reports.

Semantics that later work must respect:
- Identity is SHA-256, consulted before path.
- Duplicate-ID ownership comes from a pre-plan `held` map, so an already-indexed record keeps its ID against a smaller-path newcomer, and the rule is stable across repeated identical rescans.
- A manual ID edit always wins; manual IDs are never withheld.
- Content hashes are reserved only by files that produced an indexable record.
- A record that is both condition-failed and contested keeps its condition in `status`; the collision is a per-rescan fact in `conflicts()`. **Any preview must join `conflicts()` and `failures()` by path** rather than collapsing them into one chip.
- `FileFailureReason` covers `Unreadable`, `InvalidPdf`, `NotAPdf`, `Metadata`, `DuplicateContent`, and `Store`.

### Task 5 - sidecar (`95a2ef8`, `73d7e04`, `1ff30e0`)

`SidecarManifest { schema_version, generated_at, tool_version, entries }` with `SidecarEntry` carrying path, fingerprint, metadata, `IndexStatus`, and optional OCR. `SidecarImporter::preview(local, manifest, catalog)` takes an object-safe `PdfCatalog` so hashes are verified against real files without filesystem coupling. `preview` is strictly read-only; `apply` follows explicit user confirmation.

Nested wire values deserialize through private `deny_unknown_fields` structs, while the domain types stay lenient for the legacy `origin` migration. Missing `schema_version` is rejected.

`ImportPreview` exposes `conflicts()`, `conflict_for(path)`, `contested_records()`, `absent_entries()`, and `unreadable_entries()`. A withheld duplicate ID is identifiable by `status() == DuplicateIdConflict` together with `metadata().id() == None`.

## Completed: Linux Task 1 - CLI and catalog (`e7c153d`, `21636f4`, `4bdb442`, `2abc720`, `c70b0b0`)

`notae-index <library-root>` with `--verbose`. Recursive `.pdf`/`.PDF` discovery, non-PDF filtering, root-relative forward-slash paths, real SHA-256 with byte length and modification time.

Contracts established across four review rounds:
- Symbolic links are never followed, in the tree or as the root. No bytes outside the root are read or reported.
- A name the domain type rejects (`C:.pdf`) becomes a recorded failure, never a panic.
- The root is validated exactly once by `Cli::validate`, which returns a `ValidatedRoot` carrying an open handle plus that handle's device and inode. `FilesystemCatalog::new` takes it and is infallible. Every collected entry carries its identity through descent and fingerprinting.
- `named_root` normalizes trailing separators, `.`, and doubled separators purely lexically, so `link/`, `link/.`, and `link//` are all refused.
- Exit codes: `0` for `--help`, `--version`, and any completed run including one with isolated per-file failures; non-zero only when the command cannot produce a report.
- Documented residual window: the stat checks are not atomic with the operations they guard. This is a defence against silent redirection, not a privilege boundary, appropriate for a user indexing their own library.

## In progress: Linux Task 2 - PDF adapter

Committed as `5d3de0b`. Implementation chose `hayro` 0.8 (pure Rust parser and rasterizer) plus `image` 0.25, after establishing that the plan's `pdfium-render` route cannot work here (no `libpdfium`, loaded at runtime) and that `pdf-extract` panicked on 48 of 7,385 damaged PDFs in a fuzz sweep versus 0 for hayro.

Established: `extract_text` returns `Ok(None)` for no usable text layer and `Ok(Some(text))` otherwise, with normalization matching `OcrText::new`; unmappable glyphs are dropped rather than guessed. `RenderedPage` is `pub type RenderedPage = image::GrayImage`, so Task 3's `type OcrImage = image::GrayImage` accepts a rendered page with zero conversion. Default DPI 300, chosen from measurements (150 DPI 11 ms/page, 300 DPI 28 ms/page, 400 DPI 45 ms/page with no measurable accuracy gain).

### Open findings not yet fixed

A review raised four issues. **None are fixed. Work on them was interrupted twice and the working tree is mid-edit.**

1. **Unbounded DPI reaches the rasterizer.** Any nonzero `u32` is passed to hayro as an `f32` scale. Scaled dimensions convert to `u16`, so an absurd DPI saturates near 65,535 and attempts a multi-gigabyte allocation instead of returning `PdfError`, risking an OOM abort. Needs a maximum dimension and pixel budget validated before `hayro::render`.
2. **Damaged page streams are silently accepted.** hayro turns a failed content-stream decode into `None` and supplies an empty operation iterator, so a PDF with a valid page tree but a truncated `/Contents` filter is accepted, `extract_text` returns `Ok(None)`, and `render_page` returns a blank image. That routes a damaged file into OCR. Needs stream validation surfaced as `InvalidPdf`, without rejecting valid unusual filters, and keeping encrypted PDFs distinguishable from corrupt ones.
3. **No PDF header check.** hayro accepts a missing version and its xref fallback scans without requiring a header, so a non-PDF blob containing PDF-like objects can be accepted. Needs an explicit header check; the spec allows the header within the first 1024 bytes and real files carry leading junk.
4. **Word boundaries encoded by `TJ` positioning are lost.** Same-baseline glyphs are concatenated directly. `[(Notae) 250 (searchable)] TJ` becomes `Notaesearchable`, which directly breaks search for titles and composers. Needs a position or advance based separator with a justified threshold, while retaining explicit spaces.

### Uncommitted working tree

```
 M apps/notae-index/tests/fixtures/generate.py
 M apps/notae-index/tests/pdf_fixtures.rs
?? apps/notae-index/tests/fixtures/damaged-contents.pdf
?? apps/notae-index/tests/fixtures/hex-contents.pdf
?? apps/notae-index/tests/fixtures/leading-junk.pdf
?? apps/notae-index/tests/fixtures/no-header.pdf
?? apps/notae-index/tests/fixtures/word-spacing.pdf
```

Tests and fixtures for all four findings exist in the working tree. `pdf.rs` is unmodified, so **the implementation side of the fix is absent while the tests are present**, meaning `cargo test -p notae-index-cli --test pdf_fixtures` is expected to fail on those four. Verify before continuing.

The next step is to finish the `pdf.rs` implementation for these four findings, confirm each test fails without its fix, then commit and pass review.

## Not started

- Linux Task 3: `notae-ocr` crate, multilingual automatic OCR.
- Linux Task 4: incremental sidecar pipeline.
- Linux Task 5: sidecar output wiring and CLI documentation.
- The entire Android plan, including the `notae-android` crate.

## Environment notes

- Tesseract 5.5.3 is installed at `/data/data/com.termux/files/usr/bin/tesseract`.
- Tessdata lives at `/data/data/com.termux/files/usr/share/tessdata` and contains **only `eng`**. Polish and Latin packs are not installed. Task 3 needs them; install before claiming multilingual OCR works.
- No `pdfium` binary and no `libpdfium` in the linker cache. `pdfium-render` cannot be used.
- Workspace forbids `unsafe`. No crate in this repository uses it, and introducing it needs explicit agreement.
- `rustc` and `cargo` are 1.99.0. The environment is aarch64 Android/Termux.

## Verification commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p notae-index-cli -- --help
```

For Linux Task 1 behavior, the binary must be exercised directly against real fixture trees; several contracts are only observable through exit codes and filesystem conditions.

## Working method

Work proceeds by subagent-driven development: a fresh implementer per task, then a fresh reviewer, then a fix round, then a closeout review. Each task in this project has needed two to four fix rounds. Findings have consistently been real rather than stylistic, so the review gate is doing work; do not bypass it.

Two implementation rounds were interrupted by process exits mid-task. After any interruption, check `git status` and `git diff` before re-dispatching, because an interrupted agent may have left partial edits or reverted work in place.