# Fast Scan and PC Sidecar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A rescan reads no unchanged PDF, takes hashes/metadata/text/thumbnails from a PC-written `.notae/` sidecar when path and size match, and runs automatically when a library opens.

**Architecture:** Listing (Kotlin) stops hashing and reports size + modified time; the Rust rescan resolves each listed PDF to a fingerprint from (1) the index when path, size and time match, (2) the sidecar when path and size match, or (3) a per-file `hash` callback, then reconciles as today. Sidecar entries are applied with `PcImported` provenance and remembered per path by entry digest, so a re-run of the PC tool reaches the tablet. A new workspace binary, `apps/notae-sidecar`, writes the sidecar from the schola folder and `Indeks.csv`.

**Tech Stack:** Rust 1.98.1 (`notae-domain`, `notae-index` with rusqlite, `notae-app-core` with UniFFI 0.32.2), Kotlin/Robolectric, `csv` crate (PC tool only), `pdftoppm` (PC only).

**Spec:** `docs/superpowers/specs/2026-11-22-fast-scan-and-pc-sidecar-design.md`

## Global Constraints

- Toolchain: workspace-pinned Rust 1.98.1. Gates: `scripts/check.sh fast` while working, `scripts/check.sh full` before merging.
- No `unwrap`/`expect` in production code (tests may). `unsafe` stays forbidden in every crate.
- Only new dependency: `csv` in `apps/notae-sidecar`. Nothing new in the app.
- The library is read-only: the tablet never writes into the library folder.
- Sidecar location: `.notae/library.json` and `.notae/thumbnails/<sha256>.png` at the library root. `version: 1`.
- Trust rule: a sidecar entry applies when **path and byte size** match. Modified time is not part of the sidecar match.
- Unchanged rule: a listed file is reused without reading when path, size **and** modified time equal the index row; a missing modified time never matches.
- Provenance: `Manual` beats `PcImported` beats `FilenameDerived`.
- Sidecar text replaces stored text and sets the note `Ready`; `null`/empty text falls back to the PDF text layer.
- Thumbnail width 400 px (`THUMBNAIL_WIDTH_PX`).
- Tags = `Rodzaj` value + `Tagi` values split on `;`, trimmed, empty dropped, deduplicated.
- The tool writes nothing when it hits an error; PDFs without a CSV row are warnings.
- Conventional Commits, e.g. `feat(scan): skip unchanged files`.

## Review Focus

1. **Upgrading from 0.7.1:** every index row has `modified_secs = NULL`, so the first 0.8.0 scan must hash (or sidecar-match) each file once **and store the time**, and the second scan must read nothing. Pinned in Task 3 (`unchanged_record_takes_the_new_modified_time`) and Task 7 (`second_scan_reads_nothing`).
2. **A PDF replaced by a different file at the same path** (new size or new time): it must be hashed, not reused from the index. Pinned in Task 7 (`changed_size_or_time_is_hashed`).
3. **Re-scan after a sidecar title was applied:** the filename refresh must not flip a `PcImported` title back to the filename's. Pinned in Task 3 (`filename_refresh_keeps_pc_imported_fields`).
4. **A malformed or future-version `library.json`:** the scan completes without it and reports it. Pinned in Task 7 (`broken_sidecar_is_ignored_and_reported`).
5. **Sidecar entry whose size no longer matches** (PDF edited after the tool ran): hashed and indexed normally, sidecar metadata not applied. Pinned in Task 7 (`size_mismatch_ignores_the_entry`).

---

### Task 1: ADR 0004 and glossary

**Files:**
- Create: `docs/adr/0004-pc-sidecar-and-automatic-scans.md`
- Modify: `GLOSSARY.md:12` (the **Sidecar** entry)
- Modify: `docs/adr/0002-compose-ui-over-ffi.md` (status line: "item 8 partly superseded by ADR 0004")

- [ ] **Step 1: Write ADR 0004** in the format of ADR 0003 (Status Accepted 2026-11-22, Issues #55, Context / Decision / Options considered / Consequences). Decision points, verbatim from the spec: the PC sidecar returns as `.notae/library.json` + `.notae/thumbnails/`; trust path and size; `Manual` > `PcImported` > `FilenameDerived`; sidecar text wins; automatic rescans on folder pick and on app start with a saved library ("started by the user" in ADR 0001 now applies to OCR only); no network access added. Options: the three rows of the spec's Approaches table.
- [ ] **Step 2: Rewrite the glossary entry**: **Sidecar** — the `.notae/` folder the `notae-sidecar` PC tool writes into the library (`library.json`, `thumbnails/`); read by Notae from 0.8.0, never written by it. Keep one sentence on the 0.3.0 sidecar (`index.sqlite`, `manifest.json`): ignored.
- [ ] **Step 3: Commit**

```bash
git add docs/adr/0004-pc-sidecar-and-automatic-scans.md docs/adr/0002-compose-ui-over-ffi.md GLOSSARY.md
git commit -m "docs(adr): record the PC sidecar and automatic scans"
```

### Task 2: Sidecar format types and index schema v3

**Files:**
- Create: `crates/notae-domain/src/sidecar.rs`; Modify: `crates/notae-domain/src/lib.rs` (module + re-exports)
- Create: `crates/notae-index/migrations/0003_sidecar_applied.sql`
- Modify: `crates/notae-index/src/schema.rs` (`SCHEMA_VERSION`, `MIGRATIONS`, `SCHEMA_OBJECTS`), `crates/notae-index/src/store.rs`
- Test: `crates/notae-domain/src/sidecar.rs` (unit), `crates/notae-index/tests/store_and_search.rs`

**Interfaces:**
- Produces (domain): `pub const SIDECAR_VERSION: u32 = 1;` `pub const SIDECAR_FILE: &str = ".notae/library.json";` `pub const SIDECAR_THUMBNAILS: &str = ".notae/thumbnails";` `pub const SIDECAR_DIR: &str = ".notae";`
  `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct SidecarFile { pub version: u32, pub generator: String, pub scores: Vec<SidecarEntry> }`
  `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct SidecarEntry { pub path: String, pub size: u64, pub sha256: String, pub number: Option<u32>, pub title: Option<String>, pub composer: Option<String>, #[serde(default)] pub tags: Vec<String>, pub text: Option<String> }`
- Produces (index): `IndexStore::sidecar_digest(&self, path: &RelativePdfPath) -> Result<Option<String>, IndexError>`; `IndexStore::set_sidecar_digest(&self, path: &RelativePdfPath, digest: &str) -> Result<(), IndexError>`; `delete_missing` also deletes `sidecar_applied` rows for removed paths.

- [ ] **Step 1: Write failing tests**
  - domain: `sidecar_file_round_trips_the_spec_example` — deserialize the JSON example from spec §4 (with a real 64-hex sha) and assert `version == 1`, `scores[0].number == Some(1)`, `composer == None`, `tags == ["Z katalogu"]`; serialize back and deserialize equal.
  - index: `fresh_and_migrated_databases_are_at_version_3` — `IndexStore::open` on a new file and on a copy of `tests/fixtures/sidecar-provenance.sqlite` both report `schema_version() == 3`.
  - index: `sidecar_digest_is_stored_per_path_and_pruned_with_the_file` — set `"abc"` for `a.pdf`, read back `Some("abc")`, overwrite with `"def"`, read `Some("def")`; `delete_missing` with a set lacking `a.pdf` → `None`.
- [ ] **Step 2: Run, verify they fail**
  Run: `cargo test -p notae-domain sidecar && cargo test -p notae-index --test store_and_search sidecar_digest fresh_and_migrated`
  Expected: compile errors / FAIL (no `sidecar` module, version 2).
- [ ] **Step 3: Implement.** Migration SQL: `CREATE TABLE sidecar_applied (path TEXT PRIMARY KEY, entry_sha256 TEXT NOT NULL) STRICT;` (a separate table, so `notes` upserts can never drop it). `SCHEMA_VERSION = 3`, migration `{ version: 3, name: "sidecar_applied", sql: include_str!("../migrations/0003_sidecar_applied.sql") }`, add `("table", "sidecar_applied")` to `SCHEMA_OBJECTS` — `verify_objects` checks objects per version, so make the list version-aware (objects introduced at 3 are only required when `version >= 3`). `set_sidecar_digest` is `INSERT … ON CONFLICT(path) DO UPDATE`. Update the `SCHEMA_VERSION` doc comment.
- [ ] **Step 4: Run tests, verify pass**
  Run: `cargo test -p notae-domain && cargo test -p notae-index`
  Expected: PASS (including `sidecar_provenance.rs`, which asserts `SCHEMA_VERSION`).
- [ ] **Step 5: Commit** — `feat(index): store applied sidecar entries (schema 3)`

### Task 3: Provenance and reconcile rules

**Files:**
- Modify: `crates/notae-index/src/provenance.rs` (`refresh_from_parsed`, new `refresh_from_sidecar`), `crates/notae-index/src/lib.rs` (re-export)
- Modify: `crates/notae-index/src/reconcile.rs:1274-1290` (`classify`, same-hash same-path branch)
- Test: `crates/notae-index/tests/reconcile.rs`, unit tests in `provenance.rs`

**Interfaces:**
- Consumes: `notae_domain::SidecarEntry` (Task 2).
- Produces: `pub fn refresh_from_sidecar(existing: &NoteMetadata, entry: &SidecarEntry) -> Result<(NoteMetadata, RefreshOutcome), MetadataRefreshError>`.

- [ ] **Step 1: Write failing tests**
  - `filename_refresh_keeps_pc_imported_fields` — metadata with title `"Adeste Fideles (PC)"` origin `PcImported` and id 1 `PcImported`; `refresh_from_filename(&m, "001 - Adeste Fideles.pdf")` keeps title `"Adeste Fideles (PC)"` and origin `PcImported`.
  - `sidecar_overrides_filename_but_not_manual` — title `FilenameDerived`, composer `Manual "Bach"`; entry `title: Some("T")`, `composer: Some("Taizé")`, `tags: ["Wielkanoc"]`, `number: Some(7)` → title `"T"`/`PcImported`, composer `"Bach"`/`Manual`, tags `["Wielkanoc"]`/`PcImported`, id 7/`PcImported`.
  - `sidecar_none_keeps_number_and_title_but_clears_composer_and_tags` — entry `number: None, title: None, composer: None, tags: []` over `PcImported` composer `"X"` and tags `["a"]` → id and title unchanged, composer `None`, tags empty (both `PcImported`).
  - reconcile: `unchanged_record_takes_the_new_modified_time` — index a file with fingerprint `(sha, 10, None)`; plan the same path with `(sha, 10, Some(t))`; assert one `updates()` entry (not `unchanged()`), and after `apply` the stored fingerprint's modified is `Some(t)`.
- [ ] **Step 2: Run, verify fail**
  Run: `cargo test -p notae-index refresh_ sidecar_ unchanged_record_takes`
  Expected: FAIL.
- [ ] **Step 3: Implement.** In `refresh_from_parsed`, the filename overwrites a field only when its origin is `FilenameDerived` or `Unset` (today: anything but `Manual`); update the module and function docs. `refresh_from_sidecar`: skip any `Manual` field; `number`/`title` assign only when `Some` (title trimmed, non-empty); `composer` and `tags` always assigned (None/empty clears); all with `MetadataOrigin::PcImported`, via `assign_id`, `assign_title`, `assign_composer_or_artist`, `assign_tags`. In `classify`, add `&& record.fingerprint() == &fingerprint` to the `Unchanged` condition so a changed modified time becomes an `Update` (which keeps OCR text through the store, as other updates do — assert this in the test too).
- [ ] **Step 4: Run, verify pass** — `cargo test -p notae-index` → PASS.
- [ ] **Step 5: Commit** — `feat(index): apply sidecar metadata below manual edits`

### Task 4: Platform interface in Rust

**Files:**
- Modify: `crates/notae-app-core/src/platform.rs` (`DocumentEntry`, `PlatformLibrary`, `PlatformCatalog`)
- Modify: `crates/notae-app-core/src/storage.rs` (`ListedPdf`, `LibraryCatalog`, forwarding in `LibrarySource` and `SharedLibraryCatalog`)
- Modify: `crates/notae-app-core/tests/support/fake_platform.rs`, `crates/notae-app-core/tests/support/fs_catalog.rs`
- Test: `crates/notae-app-core/tests/platform.rs`

**Interfaces:**
- Produces (UniFFI): `pub struct DocumentEntry { pub path: String, pub display_name: String, pub size: u64, pub modified_millis: Option<i64> }` (the `content` field is removed);
  `fn hash(&self, handle: String, path: String) -> Result<DocumentContent, PlatformError>;`
  `fn read_file(&self, handle: String, path: String) -> Result<Option<Vec<u8>>, PlatformError>;` (`None` when absent).
- Produces (core): `pub struct ListedPdf { pub path: RelativePdfPath, pub filename: String, pub size: u64, pub modified: Option<EpochTimestamp> }` in `storage.rs`;
  `LibraryCatalog::list(&self) -> Result<Vec<ListedPdf>, CatalogError>`;
  `LibraryCatalog::hash(&self, path: &RelativePdfPath, size: u64, modified: Option<EpochTimestamp>) -> Result<FileCondition, CatalogError>` (`PermissionLost` is an `Err`; any other failure is `Ok(FileCondition::Unreadable)`);
  `LibraryCatalog::read_file(&self, path: &str) -> Result<Option<Vec<u8>>, CatalogError>`.
  Defaults on the trait (for `fs_catalog.rs`): `list` maps `enumerate()`; `hash` looks the path up in `enumerate()`; `read_file` returns `Ok(None)`.
- FakePlatform gains `set_hashes(HashMap<String, Result<DocumentContent, PlatformError>>)`, `set_files(HashMap<String, Vec<u8>>)`, and records `hash {handle} {path}` and `read {handle} {path}` in `calls()`.

- [ ] **Step 1: Write failing tests** in `tests/platform.rs`:
  - `list_converts_millis_and_hashes_nothing` — entry `modified_millis: Some(1_700_000_000_123)` → `ListedPdf.modified == EpochTimestamp::new(1_700_000_000, 123_000_000)`; `calls()` has no `hash`.
  - `hash_maps_content_to_condition` — `Sha256{hex}` → `Readable(PdfFingerprint::new(digest, size, modified))`; `Unreadable` and a bad hex → `Unreadable`; `InvalidPdf` → `InvalidPdf`; `PermissionLost` → `Err(CatalogError::PermissionLost)`.
  - `read_file_passes_bytes_and_absence` — `Some(bytes)` and `None`.
  - Adapt existing tests that built `DocumentEntry { content, .. }`.
- [ ] **Step 2: Run, verify fail** — `cargo test -p notae-app-core --test platform` → compile errors.
- [ ] **Step 3: Implement.** `PlatformCatalog::list` drops paths the domain rejects into `rejected` exactly as `enumerate` does today; `PdfCatalog::enumerate` for `PlatformCatalog` becomes `list` + `hash` per file (unchanged behaviour for any remaining caller). Millis → `EpochTimestamp::new(ms.div_euclid(1000), (ms.rem_euclid(1000) * 1_000_000) as u32)` with a checked conversion (no `as` truncation lint; use `u32::try_from`). Update the module doc ("lists the library, hashes one document on request, reads a sidecar file…").
- [ ] **Step 4: Run, verify pass** — `cargo test -p notae-app-core` → PASS (all binaries using `FakePlatform` compile).
- [ ] **Step 5: Commit** — `feat(platform): list without hashing and hash on request`

### Task 5: Kotlin `SafLibrary`

**Files:**
- Modify: `apps/notae-android/app/src/main/kotlin/com/c2yz/notae/platform/SafLibrary.kt`
- Test: `apps/notae-android/app/src/test/kotlin/com/c2yz/notae/platform/SafLibraryTest.kt` (and `FakeDocumentsProvider` in it)

**Interfaces:**
- Consumes: regenerated UniFFI bindings for Task 4 (`DocumentEntry(path, displayName, size, modifiedMillis)`, `hash`, `readFile`).
- Produces: `override fun enumerate(handle: String): List<DocumentEntry>`, `override fun hash(handle: String, path: String): DocumentContent`, `override fun readFile(handle: String, path: String): ByteArray?`.

- [ ] **Step 1: Write failing tests** (provider returns `COLUMN_SIZE` and `COLUMN_LAST_MODIFIED`; it counts `openDocument` calls):
  - `enumerate_lists_size_and_modified_without_opening` — two PDFs → sizes and `modifiedMillis` as served; `openCount == 0`.
  - `enumerate_skips_the_notae_folder` — root `.notae/` holding `x.pdf` is not listed.
  - `hash_reports_sha256_and_unreadable` — readable file → `DocumentContent.Sha256(sha256Hex(bytes))`; missing file → `Unreadable`.
  - `read_file_returns_bytes_or_null` — `.notae/library.json` bytes; absent path → `null`.
  - Adapt `enumerate_reports_unreadable_document` to `hash`.
- [ ] **Step 2: Run, verify fail**
  Run: `ANDROID_HOME=~/Android/Sdk apps/notae-android/gradlew -p apps/notae-android testDebugUnitTest --tests 'com.c2yz.notae.platform.SafLibraryTest'`
  Expected: FAIL / compile errors.
- [ ] **Step 3: Implement.** `children()` projection adds `COLUMN_SIZE`, `COLUMN_LAST_MODIFIED` (nullable → `null`); `enumerateChildren` skips a directory named `.notae` at the root; `readEntry` becomes `hash` (same stream + `sha256Hex`, same exception mapping, `SecurityException` rethrown); `readFile` resolves with `documentUri` and returns `null` on `FileNotFoundException` or a missing segment; all wrapped in `platformCall`.
- [ ] **Step 4: Run, verify pass** — same command → PASS.
- [ ] **Step 5: Commit** — `feat(android): list documents without reading them`

### Task 6: Sidecar parsing in the core

**Files:**
- Create: `crates/notae-app-core/src/sidecar.rs`; Modify: `crates/notae-app-core/src/lib.rs`
- Test: unit tests in `sidecar.rs`

**Interfaces:**
- Consumes: `SidecarFile`, `SidecarEntry`, `SIDECAR_VERSION` (Task 2).
- Produces: `pub struct Sidecar` with `pub fn parse(bytes: &[u8]) -> Result<Sidecar, SidecarError>`, `pub fn matching(&self, path: &str, size: u64) -> Option<&SidecarEntry>`; `pub fn entry_digest(entry: &SidecarEntry) -> String` (lowercase hex SHA-256 of `serde_json::to_vec(entry)`); `pub enum SidecarError { Malformed { detail: String }, UnsupportedVersion { found: u32 }, BadHash { path: String } }`.

- [ ] **Step 1: Write failing tests** — `parses_and_matches_on_path_and_size` (match on `("a.pdf", 10)`, none on `("a.pdf", 11)` or `("b.pdf", 10)`); `rejects_unknown_version` (`version: 2` → `UnsupportedVersion { found: 2 }`); `rejects_malformed_json`; `rejects_an_entry_with_a_bad_sha` (`BadHash`); `digest_changes_when_tags_change`.
- [ ] **Step 2: Run, verify fail** — `cargo test -p notae-app-core --lib sidecar` → FAIL.
- [ ] **Step 3: Implement** (entries in a `HashMap<String, SidecarEntry>` keyed by path; sha validated with `Sha256Digest::parse`).
- [ ] **Step 4: Run, verify pass** — same command → PASS.
- [ ] **Step 5: Commit** — `feat(scan): parse the PC sidecar`

### Task 7: Rescan resolves, skips and applies the sidecar

**Files:**
- Create: `crates/notae-app-core/src/operations/resolve.rs`; Modify: `crates/notae-app-core/src/operations/mod.rs`
- Modify: `crates/notae-app-core/src/operations/rescan.rs` (`run_rescan`, `apply_pdf_text_layer` caller), `crates/notae-app-core/src/operations/events.rs` (`RescanSummary`)
- Modify: Kotlin banner text for the new summary flag (`apps/notae-android/app/src/main/kotlin/com/c2yz/notae/ui/components/OperationBanner.kt`, `res/values/strings.xml`, `res/values-pl/strings.xml`)
- Test: create `crates/notae-app-core/tests/fast_scan.rs` (uses `support/fake_platform.rs`)

**Interfaces:**
- Consumes: `LibraryCatalog::{list, hash, read_file}` (Task 4), `Sidecar`, `entry_digest` (Task 6), `refresh_from_sidecar` (Task 3), `IndexStore::{sidecar_digest, set_sidecar_digest}` (Task 2), `ThumbnailCache::ensure`.
- Produces: `pub(crate) struct Resolved { pub files: Vec<PdfFile>, pub unchanged: HashSet<RelativePdfPath>, pub sidecar: HashMap<RelativePdfPath, SidecarEntry> }`; `pub(crate) fn resolve(local: &IndexStore, catalog: &dyn LibraryCatalog, listed: Vec<ListedPdf>, sidecar: Option<&Sidecar>) -> Result<Resolved, CatalogError>`; `RescanSummary.sidecar_ignored: bool`.

Resolution, per listed file, in this order:
1. Index row at the same path with equal `byte_len` and `modified == Some(..)` equal → `Readable(stored sha, size, modified)`, path into `unchanged`.
2. Else sidecar entry matching path + size → `Readable(entry sha, size, modified)`.
3. Else `catalog.hash(path, size, modified)`.
Any file whose sidecar entry matches (cases 1 or 2) goes into `sidecar` **only if** `entry_digest(entry) != local.sidecar_digest(path)`.

Per-file loop after `Reconciler::apply`:
- In `sidecar`: apply `refresh_from_sidecar` to the stored note and write it; if `text` is non-empty, store it with `set_ocr` and status `Ready`, else run `apply_pdf_text_layer`; thumbnail: if the cache lacks the key, `ensure(key, |out| …)` writes the bytes from `read_file(".notae/thumbnails/<sha>.png")`, falling back to `render_thumbnail` when absent; then `set_sidecar_digest`.
- Else in `unchanged`: no text extraction, no thumbnail call; report `FileSucceeded`.
- Else: today's path (`apply_pdf_text_layer`, `render_thumbnail`).
Sidecar read at scan start: `read_file(SIDECAR_FILE)`; `Ok(None)` → no sidecar; parse error → `sidecar_ignored = true` and continue; `Err(PermissionLost)` aborts like a failed `enumerate`.

- [ ] **Step 1: Write failing tests** in `tests/fast_scan.rs` (drive `OperationController` over `PlatformCatalog<FakePlatform>` as `text_layer_rescan.rs` does; count calls by prefix):
  - `first_scan_hashes_each_file_once` — 3 PDFs with times, no sidecar → 3 `hash`, 3 `text`, 3 `thumbnail`.
  - `second_scan_reads_nothing` — rescan the same listing → 0 `hash`, 0 `text`, 0 `thumbnail`.
  - `changed_size_or_time_is_hashed` — change size of one and time of another → exactly 2 `hash`.
  - `missing_time_is_always_hashed` — `modified_millis: None` → `hash` on every scan.
  - `sidecar_match_reads_no_pdf` — sidecar for 2 of 3 files (with thumbnail bytes) → 1 `hash`, 1 `text`, 1 `thumbnail`; search for a word only in sidecar text finds the file; row title/composer/tags from the entry; thumbnail file exists in the cache with the served bytes.
  - `size_mismatch_ignores_the_entry` — entry size 10, listed size 11 → hashed; title from the filename.
  - `changed_entry_is_reapplied_to_an_unchanged_file` — scan, then serve a sidecar whose entry adds tag `"Komunia"` → 0 `hash`; tags include `"Komunia"`.
  - `manual_edit_survives_a_changed_entry` — save metadata with composer `"Ja"`, then change the entry's composer → composer stays `"Ja"`.
  - `broken_sidecar_is_ignored_and_reported` — `library.json` = `b"{"` → scan `Completed` with `summary.sidecar_ignored == true` and files hashed.
  - `empty_sidecar_text_falls_back_to_the_text_layer` — entry `text: None` → 1 `text` call for that file.
- [ ] **Step 2: Run, verify fail** — `cargo test -p notae-app-core --test fast_scan` → FAIL.
- [ ] **Step 3: Implement** `resolve` and the loop changes above; `RescanSummary.sidecar_ignored` defaults `false`. Progress events still count every listed file (`total = listed.len()`).
- [ ] **Step 4: Run, verify pass** — `cargo test -p notae-app-core` → PASS (`text_layer_rescan.rs`, `operation_state.rs` unchanged in outcome).
- [ ] **Step 5: Kotlin banner** — when `sidecarIgnored`, the finished banner adds the string `scan_sidecar_ignored` = "The library's .notae file could not be read; scanned without it." / pl "Nie udało się odczytać pliku .notae biblioteki; przeskanowano bez niego." Add a case to the existing banner test.
  Run: `ANDROID_HOME=~/Android/Sdk apps/notae-android/gradlew -p apps/notae-android testDebugUnitTest --tests '*OperationBanner*' --tests '*OverlayTest*'` → PASS (adjust filter to the test that covers the banner).
- [ ] **Step 6: Commit** — `feat(scan): skip unchanged files and apply the PC sidecar`

### Task 8: Automatic scans on open and start

**Files:**
- Modify: `crates/notae-app-core/src/api.rs` (`open_root` success branch ~line 409; `Action::Rescan` arm ~line 579), `crates/notae-app-core/src/state.rs:229-232` (doc comment)
- Test: `crates/notae-app-core/tests/api.rs`, `crates/notae-app-core/tests/startup_state.rs`

**Interfaces:**
- Produces: `CoreState::begin_rescan(&mut self)` — the body of today's `Action::Rescan` arm, now called from that arm and from `open_root` after `state.operations` is set.

- [ ] **Step 1: Write failing tests**
  - `picking_a_root_scans_without_rescan` (api.rs) — pick a root with 2 PDFs, never send `Action::Rescan`, wait for a finished snapshot → 2 library rows.
  - `restarting_with_a_saved_root_scans` (api.rs) — start a core over a data dir with a saved root → finished snapshot with rows and `enumerate` in `calls()`.
  - Update `fresh_core_starts_unconfigured`/smoke expectations only where they asserted "no scan" after a pick; a fresh install with no root still starts no scan (keep that assertion).
- [ ] **Step 2: Run, verify fail** — `cargo test -p notae-app-core --test api picking_a_root_scans restarting_with_a_saved_root` → FAIL.
- [ ] **Step 3: Implement** `begin_rescan`; call it at the end of the `Ok((controller, source))` branch. Rewrite the `state.rs` comment: a fresh process starts no scan; opening a library (picked or saved) starts one.
- [ ] **Step 4: Run, verify pass** — `cargo test -p notae-app-core` → PASS. Also `ANDROID_HOME=~/Android/Sdk apps/notae-android/gradlew -p apps/notae-android testDebugUnitTest --tests 'com.c2yz.notae.CoreSmokeTest'` → PASS.
- [ ] **Step 5: Commit** — `feat(scan): scan when a library opens`

### Task 9: PC tool `apps/notae-sidecar`

**Files:**
- Modify: `Cargo.toml` (workspace `members` adds `"apps/notae-sidecar"`)
- Create: `apps/notae-sidecar/Cargo.toml`, `apps/notae-sidecar/src/main.rs`, `apps/notae-sidecar/src/lib.rs`, `apps/notae-sidecar/src/index_csv.rs`, `apps/notae-sidecar/src/build.rs`, `apps/notae-sidecar/src/thumbnails.rs`
- Test: `apps/notae-sidecar/tests/sidecar.rs`, fixture `apps/notae-sidecar/tests/fixtures/library/` (two tiny PDFs + `Indeks.csv`)

**Interfaces:**
- Consumes: `notae_domain::{SidecarFile, SidecarEntry, SIDECAR_VERSION, parse_filename}`.
- Produces:
  - `pub struct CsvRow { pub id: Option<u32>, pub title: Option<String>, pub author: Option<String>, pub kind: Option<String>, pub tags: Vec<String>, pub text: Option<String> }`; `pub fn read_index(path: &Path) -> Result<HashMap<String, CsvRow>, ToolError>` keyed by `Nazwa pliku` (UTF-8 BOM stripped; columns by header name: `ID`, `Tytuł`, `Autor`, `Rodzaj`, `Tagi` (optional column), `Tekst OCR`).
  - `pub trait Renderer { fn render(&self, pdf: &Path, out_png: &Path) -> Result<(), String>; }`; `pub struct Pdftoppm;` runs `pdftoppm -png -f 1 -l 1 -scale-to-x 400 -scale-to-y -1 -singlefile <pdf> <out-without-.png>`.
  - `pub struct Report { pub errors: Vec<String>, pub warnings: Vec<String> }`; `pub fn run(library: &Path, renderer: &dyn Renderer) -> Result<Report, ToolError>` — writes only when `errors` is empty.
- `Cargo.toml`: `publish = false`, `[lints] workspace = true`, deps `notae-domain` (path), `serde`, `serde_json`, `sha2 = "0.11"`, `csv = "1"`; dev-dep `tempfile = "3"`.

- [ ] **Step 1: Write failing tests** (a fake `Renderer` that writes `b"png"` and records calls):
  - `writes_library_json_from_pdfs_and_csv` — fixture → `.notae/library.json` with entries sorted by path; entry 1: `size` = file length, `sha256` = SHA-256 of bytes, `number` from `ID`, `title` from `Tytuł`, `composer` from `Autor`, `tags == ["Boże Narodzenie", "Komunia"]` for `Rodzaj = Boże Narodzenie`, `Tagi = "Komunia; ;Boże Narodzenie"`; `text` from `Tekst OCR`.
  - `quoted_multiline_csv_and_bom_parse` — a `Tekst OCR` value with commas, quotes and a newline round-trips.
  - `empty_id_gives_null_number` — `ID` empty and no number in the file name → `number: null`.
  - `pdf_without_row_is_a_warning` — file still written, entry has filename metadata, `warnings.len() == 1`.
  - `row_without_pdf_writes_nothing` — `errors.len() == 1`, `.notae/` not created.
  - `renderer_failure_writes_nothing` — renderer returns `Err` → error, no `library.json`.
  - `rerun_is_byte_identical_and_renders_only_missing` — second run: identical bytes, 0 renderer calls; removing a PDF deletes its thumbnail.
  - `skips_notae_and_sources_and_leaves_old_files` — PDFs under `.notae/` and `sources/` are not listed; an existing `.notae/index.sqlite` is untouched.
- [ ] **Step 2: Run, verify fail** — `cargo test -p notae-sidecar` → FAIL.
- [ ] **Step 3: Implement.** `main` takes exactly one argument (the library folder), prints warnings and errors to stderr, exits 1 on errors, 2 on usage. `library.json` is written pretty-printed via a temp file in `.notae/` then `rename`. Generator string `format!("notae-sidecar {}", env!("CARGO_PKG_VERSION"))`.
- [ ] **Step 4: Run, verify pass** — `cargo test -p notae-sidecar` → PASS; then on the real folder: `cargo run -p notae-sidecar -- schola` → exits 0, 176 entries, 176 PNGs (do not commit anything in `schola/`).
- [ ] **Step 5: Commit** — `feat(sidecar): add the PC tool that writes .notae/`

### Task 10: Documentation and full gate

**Files:**
- Modify: `CHANGELOG.md` (`## [Unreleased]`), `README.md` (Features, "Using Notae" line 206 about Rescan, a short "Preparing a library on a PC" section, Project layout lists `apps/notae-sidecar`), `docs/engineering-standards.md` only if it lists workspace members, `docs/device-test-checklist` equivalent if present (add: import schola with `.notae/`, time it).

- [ ] **Step 1: Write the docs.** CHANGELOG under Unreleased → Added: PC sidecar tool and reading `.notae/`; Changed: scans run automatically when a library opens, unchanged files are not read again. README "Using Notae": Notae scans when you pick a folder and when it starts; **Rescan** stays. New section with `cargo run -p notae-sidecar -- <folder>`, the `Indeks.csv` columns (including `Tagi`), that `pdftoppm` (poppler) is required, and to commit `.notae/` and `git rm` the 0.3.0 `index.sqlite`/`manifest.json` once.
- [ ] **Step 2: Run the full gate** — `scripts/check.sh full` → all gates pass (record new Roborazzi baselines only if the banner screenshot changed, and review the diff).
- [ ] **Step 3: Commit** — `docs: describe fast scans and the PC sidecar`

---

## Self-review notes

- Spec coverage: goals 1–3 → Tasks 4–8; §4 format → Tasks 2, 6, 9; §5 tool → Task 9; ADR → Task 1; testing list → Tasks 2–9; "left to the builder" items are decided here (names, `sidecar_applied` table, thumbnail copy through `read_file`).
- Deviation from the spec, deliberate: the index **already** stores `byte_len`, `modified_secs`, `modified_nanos` per note (`0001_initial.sql`); schema 3 therefore only adds `sidecar_applied`. Two code facts the spec did not mention are fixed in Task 3: the filename refresh overwrote `PcImported` fields, and an `Unchanged` classification never stored a newly reported modified time.
