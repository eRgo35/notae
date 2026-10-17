# Notae Implementation State

**As of:** 2026-10-17
**Branch:** `feat/v0.2.0-work` (`/home/mike/Work/notae/.worktrees/notae-v0.2.0-work`)
**Implementation baseline:** `d69f73f` (local worktree setup based on the release branch); documentation baseline `13869a2`.
**Remote:** `https://github.com/eRgo35/notae.git`

This document records where the project actually stands so work can resume without re-deriving context. It is a status record, not a design document.

## Current release cycle: v0.2.0

- [Milestone v0.2.0](https://github.com/eRgo35/notae/milestone/1) contains the twenty tablet-feedback issues (#1–#20) plus [CI repair #21](https://github.com/eRgo35/notae/issues/21).
- [Release implementation plan](plans/2026-10-15-notae-v0.2.0-release.md) defines sequencing, shared interfaces, all issue acceptance gates and publication checks.
- Product implementation is in progress on the isolated feature branch. Issue #1 startup work is implemented and locally verified; no version bump, tag or published release is claimed.
- #1: commit `7c8cd0e` implements startup state and passes focused state/UI/storage/smoke tests (33 total) plus x86_64/arm64 APK builds. Cold-start evidence and the no-root Welcome regression are recorded in `docs/android-debugging.md`.
- #2: AVD geometry evidence selected stale pre-layout focus, not a backend transform. A local candidate now focuses once after the Search transition and preferred-height settle, cancels on close/background, and passes the clean AVD first-entry screenshot without a second tap; the instrumented event log and host harness limitation are in `docs/android-debugging.md`. No Samsung Keyboard/caret test or physical interaction has occurred; #2 remains open.
- #10: Shared 120/220 ms motion, Android animator-scale disablement, sheet exit protection and state-layer transitions are locally implemented; `motion_ui` passes 6 tests and scratch AVD scale 0/1 Search smokes were captured. Tablet/dark-theme visual acceptance remains open.
- #17 requires actual TalkBack acceptance; accessible labels/host tests do not establish operability on Android.
- CI repair #21 remains under its existing status below; no issue closure or release action is claimed.
- New issue requirements supersede earlier “no animations”, “Scan in every header”, split-preview and Settings pending/privacy instructions. See the release plan for final contracts.

## Documents

| Document | Path | Status |
|---|---|---|
| Product and architecture spec | `docs/superpowers/specs/2026-10-09-notae-design.md` | Approved |
| Core plan (Tasks 1-5) | `docs/superpowers/plans/2026-10-09-notae-core-index-plan.md` | Complete |
| Linux indexer plan (Tasks 1-5) | `docs/superpowers/plans/2026-10-09-notae-linux-indexer-plan.md` | Complete |
| Android app plan (Tasks 1-7) | `docs/superpowers/plans/2026-10-09-notae-android-plan.md` | Complete |
| v0.2.0 tablet-feedback release plan | `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md` | In progress; #1 locally complete; remaining issues and device/release gates open |

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
apps/notae-android     # Rust + Slint Android app; implementation present, visual verification pending
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

## Complete: Linux Task 2 - PDF adapter

Committed as `5d3de0b`. Implementation chose `hayro` 0.8 (pure Rust parser and rasterizer) plus `image` 0.25, after establishing that the plan's `pdfium-render` route cannot work here (no `libpdfium`, loaded at runtime) and that `pdf-extract` panicked on 48 of 7,385 damaged PDFs in a fuzz sweep versus 0 for hayro.

Established: `extract_text` returns `Ok(None)` for no usable text layer and `Ok(Some(text))` otherwise, with normalization matching `OcrText::new`; unmappable glyphs are dropped rather than guessed. `RenderedPage` is `pub type RenderedPage = image::GrayImage`, so Task 3's `type OcrImage = image::GrayImage` accepts a rendered page with zero conversion. Default DPI 300, chosen from measurements (150 DPI 11 ms/page, 300 DPI 28 ms/page, 400 DPI 45 ms/page with no measurable accuracy gain).

### Review findings, now fixed (`b8a4cc4`)

A review raised four issues. All four are fixed and covered by committed fixtures and tests (`cd1d3e0` satisfies clippy).

1. **Unbounded DPI reached the rasterizer.** Fixed with `MAX_RENDER_DIMENSION = 16_384` (larger than A0 at about 495 DPI) and `MAX_RENDER_PIXELS = 40_000_000` (about A1 at 300 DPI, keeping the RGBA pixmap under roughly 160 MB). Both are validated before `hayro::render`; an over-budget request returns `PdfError::UnusableDpi`, the same condition a zero DPI reports.
2. **Damaged page streams were silently accepted.** Page content streams are now decoded and validated, surfaced as `PdfError::InvalidPdf` rather than an empty page. A fixture using the ordinary `ASCIIHexDecode` filter confirms valid unusual filters are still accepted, so detection is based on decoding rather than pattern-matching filters.
3. **No PDF header check.** An explicit `%PDF-` header check now runs before parsing. The header is accepted anywhere in the first 1024 bytes, so real files carrying leading junk are still read, and a fixture covers both the missing-header and leading-junk cases.
4. **Word boundaries encoded by `TJ` positioning were lost.** Glyph advances in page space now drive word separation using `WORD_GAP_AS_ADVANCE_FRACTION = 0.2`, above intra-word kerning and below a typesetter's word space, with `MAX_BASELINE_JITTER = 1.0` point keeping superscript and subscript on one line. A fixture using `[(Notae) 250 (searchable)] TJ` pins that `Notaesearchable` is now `Notae searchable`, which matters directly because titles and composers must land in the search index as separate words.

### Verification status

`cargo test --workspace` passes 241 tests across 15 suites with zero failures. `cargo clippy --workspace --all-targets -- -D warnings` is clean. Task 2 passed its fixture review round and the four findings are fixed and covered by committed tests. A separate closeout review pass on 2026-10-10 returned no findings.

## Completed: Linux Task 3 - OCR provider (`74c7dd6`, `34a3684`, `1efadef`)

New crate `crates/notae-ocr` with the `OcrProvider` trait, `LanguagePolicy::Automatic` and `Fixed`, `InstalledLanguages`, `DetectedLanguage`, `LanguageDetector::detect`, `TesseractProvider::new`, and `OcrText { text, detected_languages, confidence, warnings }`. `OcrImage` is `pub type OcrImage = image::GrayImage`, identical to `RenderedPage`, so a rendered page flows in with zero conversion.

The Rust Tesseract binding chosen is `tesseract = "0.15.2"`. Link is direct against `/usr/lib64/libtesseract.so.5.5.3`; a custom `build.rs` creates an `OUT_DIR/tesseract-link-shim/` with the unversioned `.so` symlinks the system lacks. The build script does NOT create `.pc` files or touch `PKG_CONFIG_PATH`; that part of the dev-env is user-side and is documented in `crates/notae-ocr/README.md`.

The `notae-ocr` crate depends on `notae-domain` (for `normalize_whitespace` — extracted as a shared helper after Task 3 review found four-way duplication) and `image = "0.25"`. Tests are committed against vendored `eng`/`pol`/`lat`/`osd` tessdata under `crates/notae-ocr/tests/tessdata/`. The 15 OCR tests verify: automatic mode selects available packs; Latin-script content is searchable; confidence and warnings are recorded; missing packs produce a typed warning; OCR errors do not panic; and the `pol` pack keeps `Zażółć gęślą jaźń` intact (the contrast against `eng` is what makes the multilingual backend non-decorative).

Documented limitations that the Android adapter should expect:

- `LanguageDetector::detect` runs Tesseract once per installed pack and picks by mean confidence, because `forbid(unsafe_code)` blocks the safe-bindings wrapper from calling `TessBaseAPIDetectOrientationScript`. For a 100-page PDF with `[eng, pol, lat]`, automatic detection performs 300 OCR invocations. A safe OSD story is a future work item.
- `OcrWarning::UnsupportedScript` is reachable only through the OSD path and is not produced by the current provider. The variant is reserved for the future.

## Completed: Linux Task 4 - pipeline (`4e13f08`, `12f63bf`, `e92e59a`, `32820d5`)

`IndexPipeline::run(catalog, store, ocr, policy, output) -> Result<RunReport, PipelineError>`. `output` is `mpsc::Sender<ProgressEvent>`; the binary drains it on a worker thread so the UI thread is not blocked. `RunReport` carries the brief's seven counters (`processed`, `reused`, `moved`, `deleted`, `ocr_failed`, `pdf_failed`, `conflicts`) plus the catalog-shaped fields the existing CLI tests rely on.

Per-file flow: catalog condition (`Unreadable`/`InvalidPdf`) emits a failure AND a status-correction write to the store, so a `Readable → Unreadable` flip updates the record's `IndexStatus` to `Inaccessible` rather than silently keeping the old status. Text-layer files skip OCR entirely; image-only files render and OCR page by page. The OCR text is bridged to `notae_index::OcrText::with_language(text, primary_language)` via `into_index_ocr` so the store and sidecar wire form both see the same shape. OCR errors increment both `pdf_failed` (per the brief's item 6 prose) and `ocr_failed` (per the brief's struct definition) so the two counters tell distinct stories.

`ProgressEvent::{Started, FileStarted, PageProgress, FileCompleted, FileFailed, Finished}`. `FileCompleted` is gated on `apply_note`/`apply_with_ocr` returning success, so a `DuplicateNoteId` rejection produces a single `FileFailed` line, not a `FileFailed` followed by a `FileCompleted`. The test `a_file_that_becomes_unreadable_updates_its_stored_status` pins the Readable→Unreadable flip.

## Completed: Linux Task 5 - sidecar output + docs (`29ccba3`, `4feed4b`, `ccd78d2`, `adf0e9e`, `66d0cec`)

The binary's `NoOcr` stub is replaced with `TesseractProvider::new(TesseractConfig::discover(...)?)`. Tessdata resolution: `--tessdata-dir` flag → `TESSDATA_PREFIX` env var → `/usr/share/tesseract/tessdata` system default. The `clap` `env` feature is enabled so the env-var fallback is part of the typed CLI surface. A missing or empty tessdata directory yields a typed CLI error and exit code 1.

After the pipeline runs, the binary calls `SidecarWriter::new(env!("CARGO_PKG_VERSION"), EpochTimestamp::now()).write(&store, manifest_path)`. Both `.notae/index.sqlite` (the live store, written by `IndexStore::open`) and `.notae/manifest.json` (the sidecar wire form) are produced. A sidecar-write failure (`EISDIR` on the manifest path, or any other `io::Error`) exits non-zero with a clear stderr message; the index.sqlite is preserved so a retry without the bad path can succeed.

The 18 sidecar integration tests in `apps/notae-index/tests/sidecar_output.rs` drive the built binary as a subprocess. They cover: both sidecar files written; schema version = `SIDECAR_SCHEMA_VERSION`; root-relative forward-slash paths; SHA-256s re-hashed independently from the file bytes; OCR status present on entries that have OCR text; detected languages round-trip through `SidecarManifest::read`; tool version = `env!("CARGO_PKG_VERSION")`; stable output across two runs; missing root, regular-file root, missing tessdata, and empty tessdata all exit non-zero; the sidecar-write-failure path (`manifest.json` pre-created as a directory) exits non-zero while leaving `index.sqlite` on disk; same-hash rename moves the record; duplicate-ID keeps both records; and `SidecarImporter::preview` accepts the produced manifest against a fresh `IndexStore` (the Android import contract the next plan will exercise).

READMEs:
- `README.md` (repo root) — workspace orientation, build commands, target device, plan/spec pointers.
- `apps/notae-index/README.md` — Linux indexer user guide: prerequisites, tessdata install, `notae-index <library-root>` invocation, rerun semantics, sidecar format, exit codes, Android import.
- `crates/notae-ocr/README.md` — the dev-env shim recipe for Fedora 44 / RHEL; build-script contract; OCR-failure semantics.

Final whole-branch review of the Linux plan (`1ff30e0..66d0cec`, 23 commits, 426 KB diff) approved for merge on 2026-10-10: all spec items the Linux plan owns are covered, no regressions in core invariants, no `unsafe`, no network at build or runtime, all five Review Focus items pinned by automated tests, the Android import smoke (`SidecarImporter::preview` against a fresh `IndexStore`) is in `apps/notae-index/tests/sidecar_output.rs`.
The Android plan and implementation are no longer unstarted; the One UI redesign record below is the current Android status.
## One UI redesign (2026-10-14)

Tasks 1–11 are implemented and reviewed. Commits by task:
- Task 1: `dc02b67` — One UI palette and system light/dark.
- Task 2: `7e09e31` — shared UI components.
- Task 3: `5800fb8` — library sections and composer counts.
- Task 4: `e52c0f2` — remember recently opened scores.
- Task 5: `a2509ab` — details/import refinements.
- Task 6: `2dffa6e` — search screen navigation.
- Task 7: `57bcc65` — render first-page thumbnails during rescan.
- Task 8: `b3b9691` — cache decoded thumbnails and skip unchanged library rows.
- Task 9: `1513f44` — responsive score details pane.
- Task 10: `4632cec` — search screen with scopes and Ctrl K.
- Task 11: `4fe85be` — first run, settings, and dialogs, including duplicate-chip-label fix.

Verification recorded in the task reports: host tests/build/lint and the aarch64 `cargo apk2 check` were exercised during implementation, with reports documenting per-task outcomes. No emulator or tablet visual checks were performed in this work session; both are unverified and deferred under the ledger ruling pending user go-ahead for device installation. Page numbers for lyrics matches remain deferred.



## Environment notes (current host: Fedora 44 x86_64, 2026-10-10)

This host is **not** the same as the one the original Task 1–2 work was committed on. The original Termux/Android environment (aarch64, tesseract at `/data/data/com.termux/files/usr/bin/tesseract`) is no longer reachable. The current host reconciles as follows:

- Tesseract C library: `/usr/lib64/libtesseract.so.5.5.3`. No `.pc` file in the system pkgconfig path.
- Tesseract 5.5.0 binary: `/usr/lib/naps2/_linux/tesseract` (not on `PATH`; useful for ad-hoc CLI probes only).
- System tessdata: `/usr/share/tesseract/tessdata/eng.traineddata` (root-owned, read-only). No `pol` or `lat` system pack.
- Vendored tessdata for tests: `crates/notae-ocr/tests/tessdata/{eng,pol,lat,osd}.traineddata` — committed, so the OCR test suite is self-contained. The `pol` and `lat` files were fetched from `tesseract-ocr/tessdata_fast`.
- The dev-env shim for compiling `notae-ocr` against the system Tesseract on Fedora 44: `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig` plus the `crates/notae-ocr/build.rs` shim. The recipe is in `crates/notae-ocr/README.md`. Without it, `cargo build -p notae-ocr` fails because `pkg-config` cannot find `tesseract` or `lept`.
- No `libpdfium` on the host. `pdfium-render` cannot be used; the PDF adapter is built on `hayro` 0.8.
- Workspace forbids `unsafe` (`workspace.lints.rust.unsafe_code = "forbid"` at the root `Cargo.toml`). No crate uses `unsafe`.
- `rustc` 1.98.1, `cargo` 1.98.1, host x86_64 GNU/Linux Fedora 44. No sudo; system packages cannot be installed from this account.

## Verification commands

```bash
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo test --workspace
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo clippy --workspace --all-targets -- -D warnings
PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig cargo run -p notae-index-cli -- --help
```

The `PKG_CONFIG_PATH` is required on Fedora 44 / RHEL-family hosts because the system ships `libtesseract.so.5.5.3` and `libleptonica.so.6.0.0` without the unversioned `.so` symlinks that `pkg-config`-driven Rust bindings look up. `crates/notae-ocr/README.md` documents the recipe; the dev-env shim was deferred from Task 3 to Task 5. Without `PKG_CONFIG_PATH` set, `cargo test -p notae-ocr` fails before the linker step. Other crates (domain, index, index-cli) build without it.

A real CLI smoke run against a fixture library of text-layer / multi-page / image-only / corrupt / duplicate-ID / same-hash-rename PDFs produces both `.notae/index.sqlite` and `.notae/manifest.json`, the manifest imports through `SidecarImporter::preview` against a fresh `IndexStore`, and the binary exits `0` for completed runs and non-zero only for an unusable root, a missing/empty tessdata directory, or a sidecar-write failure.

## Working method

Work proceeds by subagent-driven development: a fresh implementer per task, then a fresh reviewer, then a fix round, then a closeout review. Each task in this project has needed two to four fix rounds. Findings have consistently been real rather than stylistic, so the review gate is doing work; do not bypass it.

Two implementation rounds were interrupted by process exits mid-task. One left its work in a stash and appeared to have reverted `pdf.rs`; the other left tests and fixtures committed while the implementation was absent. After any interruption, check `git status`, `git diff`, **and `git stash list`** before re-dispatching, because an interrupted agent may have stashed, partially edited, or reverted work in place.