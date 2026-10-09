# Notae Core and Index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Rust domain, SQLite/FTS5 index, reconciliation engine, and portable sidecar contract shared by Android and Linux.

**Architecture:** Keep domain types and file-independent invariants in `notae-domain`; keep SQLite persistence, FTS5 search, reconciliation, and sidecar validation in `notae-index`. Platform frontends supply PDF enumeration, fingerprints, OCR, and progress through explicit traits. Android owns its application database; Linux writes a portable sidecar using the same schema contract.

**Tech Stack:** Rust 2024, `serde`, `thiserror`, `sha2`, `rusqlite` with bundled SQLite/FTS5, `tracing`, and Rust unit/integration tests.

**Spec:** `docs/superpowers/specs/2026-10-09-notae-design.md`

## Global Constraints

- PDFs remain in the user-selected root; core code MUST NOT rename, move, copy, or delete them.
- One active library root; filesystem folders are not collections.
- IDs are optional non-negative integers and globally unique.
- Filename parsing accepts the leading `<ID> - <Title>.pdf` convention only.
- Default ordering is numeric ID ascending, then unnumbered title ascending.
- Manual metadata edits are authoritative over imported metadata.
- Runtime behavior is local-only and MUST NOT require network access.
- A single bad PDF or OCR failure MUST NOT abort a full rescan.
- All persisted schemas require explicit versioning and migration tests.

## Review Focus

- `2` sorts before `10`, and an ID conflict cannot be silently hidden.
- A renamed file with the same SHA-256 remains one record.
- A changed file is reindexed without destroying manual metadata.
- FTS searches OCR text as well as structured metadata and ranks exact ID/title matches first.
- Sidecar imports reject malformed or mismatched data before mutating the local database.

---

### Task 1: Establish the Rust workspace and domain types

**Files:**
- Modify: `Cargo.toml`
- Delete: `src/main.rs`
- Create: `crates/notae-domain/Cargo.toml`
- Create: `crates/notae-domain/src/lib.rs`
- Create: `crates/notae-domain/src/metadata.rs`
- Create: `crates/notae-domain/src/identity.rs`
- Create: `crates/notae-domain/src/status.rs`
- Create: `crates/notae-domain/tests/domain_types.rs`

**Interfaces:**
- Produces `NoteId(u32)`, `NoteMetadata`, `PdfFingerprint`, `MetadataOrigin`, `IndexStatus`, `RelativePdfPath`, and typed domain errors.
- `NoteMetadata` contains `id: Option<NoteId>`, required `title`, optional `composer_or_artist`, and a normalized tag set.
- `PdfFingerprint` contains SHA-256, byte length, and observed modification metadata.
- `MetadataOrigin` distinguishes filename-derived, PC-imported, and manually edited fields.

- [ ] **Step 1: Write failing domain tests**

  Test construction, serialization, equality, non-negative ID validation, required title validation, tag normalization, and status serialization.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-domain`

  Expected: FAIL because the workspace and domain types do not exist.

- [ ] **Step 3: Implement the workspace and domain types**

  Configure a Cargo workspace with shared lints. Keep domain types independent of SQLite, Android, Slint, PDF, and OCR crates. Use stable serde representations because the types cross the sidecar boundary.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-domain`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml crates/notae-domain
  git commit -m "feat: add Notae domain model"
  ```

### Task 2: Implement filename parsing, ordering, and ID invariants

**Files:**
- Modify: `crates/notae-domain/src/metadata.rs`
- Create: `crates/notae-domain/src/filename.rs`
- Create: `crates/notae-domain/src/ordering.rs`
- Create: `crates/notae-domain/tests/filename_and_ordering.rs`

**Interfaces:**
 `parse_filename(filename: &str) -> ParsedFilename`
- `ParsedFilename { id: Option<NoteId>, title: String }`
- `compare_library_order(left: &IndexedNote, right: &IndexedNote) -> Ordering`
- `validate_unique_ids(notes: impl Iterator<Item = &IndexedNote>) -> Result<(), DuplicateId>`

- [ ] **Step 1: Write failing parser and ordering tests**

  Cover `12 - Autumn Leaves.pdf`, `507 - My Practice Arrangement.pdf`, unnumbered files, whitespace, uppercase `.PDF`, empty titles, `2` before `10`, and duplicate IDs.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-domain --test filename_and_ordering`

  Expected: FAIL because parsing and ordering are not implemented.

- [ ] **Step 3: Implement the strict leading-ID parser and total ordering**

  Accept a leading integer followed by a hyphen separator and non-empty title. Do not extract arbitrary numbers embedded in titles. Use numeric comparison for IDs and normalized case-folded title comparison for unnumbered records.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-domain --test filename_and_ordering`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/notae-domain
  git commit -m "feat: parse note filenames and enforce ID ordering"
  ```

### Task 3: Add the SQLite schema and FTS5 repository

**Files:**
- Create: `crates/notae-index/Cargo.toml`
- Create: `crates/notae-index/src/lib.rs`
- Create: `crates/notae-index/src/schema.rs`
- Create: `crates/notae-index/src/store.rs`
- Create: `crates/notae-index/src/search.rs`
- Create: `crates/notae-index/migrations/0001_initial.sql`
- Create: `crates/notae-index/tests/store_and_search.rs`

**Interfaces:**
- `IndexStore::open(path: impl AsRef<Path>) -> Result<Self, IndexError>`
- `IndexStore::upsert_note(note: &IndexedNote) -> Result<(), IndexError>`
- `IndexStore::get_by_hash(hash: &Sha256Digest) -> Result<Option<IndexedNote>, IndexError>`
- `IndexStore::get_by_id(id: NoteId) -> Result<Option<IndexedNote>, IndexError>`
- `IndexStore::search(query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError>`
- `IndexStore::delete_missing(paths: &HashSet<RelativePdfPath>) -> Result<(), IndexError>`
- `SearchHit { note: IndexedNote, rank: f64 }`

- [ ] **Step 1: Write failing schema, CRUD, and FTS tests**

  Assert schema creation, migration versioning, insert/update behavior, unique ID enforcement, hash lookup, path update, metadata persistence, FTS matches from title/tags/OCR, and exact ID/title ranking over OCR-only matches.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index --test store_and_search`

  Expected: FAIL because the crate and schema do not exist.

- [ ] **Step 3: Implement the SQLite repository**

  Use bundled SQLite with FTS5. Store structured metadata in ordinary tables and maintain an FTS5 table containing title, composer/artist, tags, filename, and normalized OCR text. Enforce the unique non-null ID constraint in SQLite and convert constraint errors into typed duplicate-ID errors. Apply migrations inside a transaction.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-index --test store_and_search`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/notae-index
  git commit -m "feat: add SQLite FTS index"
  ```

### Task 4: Implement fingerprint reconciliation and metadata provenance

**Files:**
- Modify: `crates/notae-index/src/lib.rs`
- Create: `crates/notae-index/src/reconcile.rs`
- Create: `crates/notae-index/src/provenance.rs`
- Create: `crates/notae-index/tests/reconcile.rs`

**Interfaces:**
- `trait PdfCatalog { fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError>; }`
- `PdfFile { path: RelativePdfPath, fingerprint: PdfFingerprint, filename: String }`
- `Reconciler::plan(existing: &IndexStore, files: &[PdfFile]) -> Result<RescanPlan, ReconcileError>`
- `RescanPlan { additions, updates, moves, deletions, conflicts, unchanged }`
- `Reconciler::apply(plan: RescanPlan, store: &mut IndexStore) -> Result<RescanReport, ReconcileError>`

- [ ] **Step 1: Write failing reconciliation tests**

  Cover a new PDF, unchanged PDF reuse, same-hash path move, changed content, deletion, duplicate IDs, OCR status retention, and manually edited metadata surviving filename-derived updates.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index --test reconcile`

  Expected: FAIL because reconciliation is not implemented.

- [ ] **Step 3: Implement plan/apply reconciliation**

  Match by SHA-256 first, then path for diagnostics. Treat same-hash path changes as moves. Preserve manual field provenance. Do not mutate the store until the plan is complete; collect per-file failures in the report.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-index --test reconcile`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/notae-index
  git commit -m "feat: reconcile PDF fingerprints and metadata"
  ```

### Task 5: Implement the portable sidecar manifest and import preview

**Files:**
- Create: `crates/notae-index/src/sidecar.rs`
- Create: `crates/notae-index/src/import.rs`
- Create: `crates/notae-index/tests/sidecar.rs`

**Interfaces:**
- `SidecarManifest { schema_version, generated_at, tool_version, entries }`
- `SidecarEntry { relative_path, fingerprint, metadata, ocr, provenance }`
- `SidecarWriter::write(root, destination) -> Result<(), SidecarError>`
- `SidecarImporter::preview(local, sidecar) -> Result<ImportPreview, SidecarError>`
- `SidecarImporter::apply(preview, local) -> Result<ImportReport, SidecarError>`

- [ ] **Step 1: Write failing sidecar tests**

  Cover round-trip serialization, malformed schema rejection, hash mismatch reporting, new/changed/missing classification, duplicate-ID conflicts, preservation of manual metadata, and OCR replacement when local OCR is absent or stale.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index --test sidecar`

  Expected: FAIL because the sidecar contract is not implemented.

- [ ] **Step 3: Implement versioned sidecar export/import**

  Keep the manifest JSON separate from the SQLite payload. Validate schema version and every referenced PDF hash before applying changes. Produce a complete preview without mutating the local store; apply only after explicit caller confirmation.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-index --test sidecar`

  Expected: PASS.

- [ ] **Step 5: Run the core suite**

  Run: `cargo test --workspace`

  Expected: PASS.

- [ ] **Step 6: Commit**

  ```bash
  git add crates/notae-index
  git commit -m "feat: add portable Notae sidecar import"
  ```

## Core-plan completion check

Run:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all tests pass and no warnings remain before Android or Linux frontend work begins.
