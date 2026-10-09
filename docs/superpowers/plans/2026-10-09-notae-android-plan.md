# Notae Android App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Rust/Slint tablet app for Samsung Galaxy Tab S10 FE+, including folder selection, manual rescans, search, metadata editing, sidecar preview, OCR orchestration, and Samsung Notes handoff.

**Architecture:** `notae-android` consumes the shared domain/index crates and owns only UI state, Android lifecycle, Storage Access Framework integration, external intents, and Android OCR/PDF adapters. Slint renders the search-first interface; long-running work runs off the UI thread and reports typed progress events.

**Tech Stack:** Rust 2024, Slint, `android-activity`, `cargo-apk`, Android Storage Access Framework, Android PDF intents, shared `notae-domain`/`notae-index` crates, and the shared `notae-ocr` provider contract.

**Spec:** `docs/superpowers/specs/2026-10-09-notae-design.md`

## Global Constraints

- Target device: Samsung Galaxy Tab S10 FE+.
- One persisted SAF tree permission and one active library root.
- No automatic launch rescan and no background watcher.
- PDFs are never copied, renamed, moved, or rendered by the app.
- Samsung Notes is preferred; Android chooser is the fallback.
- UI work MUST remain responsive while scanning, hashing, importing, or OCR runs.
- No network calls, telemetry, cloud OCR, or remote model downloads at runtime.
- Metadata edits update the local index and preserve manual provenance.
- The first smoke run must use the target tablet.

## Review Focus

- SAF permission loss produces recovery UI rather than an empty library.
- A rescan can be canceled or fail per file without freezing the UI.
- Search result ordering and exact-match ranking match the core contract.
- Metadata edits survive later rescans and sidecar imports.
- Samsung Notes absence falls back to the Android PDF chooser.

---

### Task 1: Create the Android/Slint application shell

**Files:**
- Create: `apps/notae-android/Cargo.toml`
- Create: `apps/notae-android/src/main.rs`
- Create: `apps/notae-android/ui/app.slint`
- Create: `apps/notae-android/android/AndroidManifest.xml`
- Create: `apps/notae-android/.cargo/config.toml`
- Create: `apps/notae-android/tests/smoke.rs`
- Modify: workspace `Cargo.toml`

**Interfaces:**
- `AppState` owns `Screen`, `LibraryState`, `SearchState`, and `OperationState`.
- `UiEvent` carries user actions into Rust.
- `UiModel` exposes observable values and callbacks to Slint.

- [ ] **Step 1: Write a smoke test for app startup state**

  Assert that a fresh app exposes the setup state and does not start a rescan automatically.

- [ ] **Step 2: Run the host smoke test and verify it fails**

  Run: `cargo test -p notae-android --test smoke`

  Expected: FAIL because the app crate and state model do not exist.

- [ ] **Step 3: Implement the Slint shell and Android activity entrypoint**

  Configure the Android backend, target the target device ABI, and provide host-testable state transitions. Keep Android calls behind a platform module so core state tests run on Linux.

- [ ] **Step 4: Run host tests and build the APK**

  Run: `cargo test -p notae-android --test smoke` and `cargo apk build --release -p notae-android`.

  Expected: PASS and a signed/debuggable APK is produced.

- [ ] **Step 5: Install and launch on the Galaxy Tab S10 FE+**

  Run the project’s `cargo apk run` deployment command. Expected: setup screen renders without a crash and remains responsive.

- [ ] **Step 6: Commit**

  ```bash
  git add apps/notae-android Cargo.toml
  git commit -m "feat: add Slint Android app shell"
  ```

### Task 2: Implement SAF root selection and document access

**Files:**
- Create: `apps/notae-android/src/android/saf.rs`
- Create: `apps/notae-android/src/storage.rs`
- Modify: `apps/notae-android/src/main.rs`
- Modify: `apps/notae-android/ui/app.slint`
- Create: `apps/notae-android/tests/storage_state.rs`

**Interfaces:**
- `trait LibraryRootProvider { async fn select_root(&self) -> Result<LibraryRoot, StorageError>; fn reopen(&self, root: &LibraryRoot) -> Result<LibrarySource, StorageError>; }`
- `LibrarySource` implements the `PdfCatalog` interface required by `notae-index`.
- `StorageState::{Unconfigured, Ready, PermissionLost, Error}`.

- [ ] **Step 1: Write failing storage-state tests**

  Cover setup, persisted root restoration, permission loss, explicit reauthorization, and rejecting a non-directory selection.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test storage_state`

  Expected: FAIL because SAF state handling is not implemented.

- [ ] **Step 3: Implement SAF selection and persisted permission**

  Launch `ACTION_OPEN_DOCUMENT_TREE`, persist read permission, wrap tree/document URIs, enumerate PDFs recursively, and expose readable URIs for the external viewer. Never convert the selected root into an assumed filesystem path.

- [ ] **Step 4: Run tests and perform the device permission scenario**

  Run: `cargo test -p notae-android --test storage_state`. On the tablet, select a test folder, revoke permission in Android Settings, relaunch, and verify the recovery screen.

  Expected: tests pass and permission loss is actionable.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: select and persist SAF library root"
  ```

### Task 3: Build the search-first library UI

**Files:**
- Modify: `apps/notae-android/ui/app.slint`
- Create: `apps/notae-android/src/ui/library.rs`
- Create: `apps/notae-android/src/ui/search.rs`
- Create: `apps/notae-android/tests/search_state.rs`

**Interfaces:**
- `LibraryViewModel::set_query(query: String)`
- `LibraryViewModel::search() -> Result<Vec<SearchRow>, UiError>`
- `SearchRow` contains ID, title, composer/artist, tags, status, and stable note key.
- `UiEvent::{OpenNote, EditMetadata, StartRescan, OpenSettings}`.

- [ ] **Step 1: Write failing view-model tests**

  Assert empty state, result projection, numeric ordering, query propagation, exact-match ranking, and status display for OCR pending/error/conflict.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test search_state`

  Expected: FAIL because the view model does not exist.

- [ ] **Step 3: Implement the Slint tablet layout and view model**

  Use a prominent search field, readable touch-sized rows, restrained Material-3-inspired spacing, and an explicit Rescan action. Do not add folder/category navigation.

- [ ] **Step 4: Run tests and visually inspect the device UI**

  Run: `cargo test -p notae-android --test search_state`. Install the APK and verify portrait/landscape resizing, keyboard input, scrolling, and touch targets on the target tablet.

  Expected: PASS and a usable search-first library screen.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: add search-first tablet library"
  ```

### Task 4: Add manual rescan, progress, and sidecar import preview

**Files:**
- Create: `apps/notae-android/src/operations/rescan.rs`
- Create: `apps/notae-android/src/operations/import.rs`
- Create: `apps/notae-android/src/operations/events.rs`
- Modify: `apps/notae-android/src/main.rs`
- Modify: `apps/notae-android/ui/app.slint`
- Create: `apps/notae-android/tests/operation_state.rs`

**Interfaces:**
- `OperationController::start_rescan()`
- `OperationController::cancel()`
- `OperationEvent::{Started, Progress, FileSucceeded, FileFailed, Completed, Canceled}`
- `ImportPreviewViewModel::load(preview: ImportPreview)`
- `ImportPreviewViewModel::apply() -> Result<ImportReport, UiError>`

- [ ] **Step 1: Write failing operation-state tests**

  Cover explicit-only start, progress aggregation, cancellation, per-file failure, completion, sidecar preview, and confirmation-required apply.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test operation_state`

  Expected: FAIL because operation orchestration is not implemented.

- [ ] **Step 3: Implement background rescan and import preview**

  Run core reconciliation through an injected catalog/index/OCR interface on worker threads. Use a deterministic fake OCR provider for this task; wire the Android provider in Task 7. Send non-blocking events to the UI. Keep preview generation read-only until the user confirms. Display new, changed, missing, hash-mismatch, duplicate-ID, OCR, and preserved-manual-metadata sections.

- [ ] **Step 4: Run tests and the operation smoke scenario**

  Run: `cargo test -p notae-android --test operation_state`. Exercise a fixture folder containing a valid PDF, a corrupt PDF, and a sidecar with one hash mismatch using the fake provider.

  Expected: valid files complete, the corrupt file is isolated, and import is not applied before confirmation.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: add manual rescan and sidecar preview"
  ```

### Task 5: Implement metadata editing

**Files:**
- Create: `apps/notae-android/src/ui/metadata_editor.rs`
- Modify: `apps/notae-android/ui/app.slint`
- Modify: `apps/notae-android/src/main.rs`
- Create: `apps/notae-android/tests/metadata_editor.rs`

**Interfaces:**
- `MetadataEditor::load(note_key) -> Result<EditableMetadata, UiError>`
- `MetadataEditor::save(input: EditableMetadata) -> Result<(), UiError>`
- `EditableMetadata { id, title, composer_or_artist, tags }`

- [ ] **Step 1: Write failing editor tests**

  Cover valid edits, empty-title rejection, duplicate-ID rejection, tag normalization, unchanged filename, manual provenance marking, and persistence after a rescan.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test metadata_editor`

  Expected: FAIL because the editor is not implemented.

- [ ] **Step 3: Implement the editor and save transaction**

  Validate through `notae-domain`, update the local index transactionally, preserve the PDF path and filename, and mark edited fields as manual provenance.

- [ ] **Step 4: Run tests and perform device edit scenario**

  Run: `cargo test -p notae-android --test metadata_editor`. Edit a title, composer, tags, and ID on the tablet, rescan, and import a sidecar that proposes different generated metadata.

  Expected: edits remain and the import preview identifies them as preserved manual fields.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: edit indexed note metadata"
  ```

### Task 6: Implement Samsung Notes handoff and chooser fallback

**Files:**
- Create: `apps/notae-android/src/android/pdf_intent.rs`
- Modify: `apps/notae-android/src/main.rs`
- Create: `apps/notae-android/tests/pdf_intent.rs`

**Interfaces:**
- `PdfLauncher::open(uri: ContentUri) -> Result<LaunchResult, ViewerError>`
- `LaunchResult::{SamsungNotes, SystemChooser}`

- [ ] **Step 1: Write failing intent tests**

  Cover Samsung Notes available, Samsung Notes unavailable, no compatible viewer, read-grant flags, and return-to-app state.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test pdf_intent`

  Expected: FAIL because intent dispatch is not implemented.

- [ ] **Step 3: Implement the external PDF intent**

  Prefer Samsung Notes only when the installed package resolves the PDF intent; otherwise launch the system chooser. Grant temporary read permission and never expose a private filesystem path.

- [ ] **Step 4: Run tests and perform device smoke scenarios**

  Run: `cargo test -p notae-android --test pdf_intent`. Open a valid PDF with Samsung Notes installed, then disable Samsung Notes and verify chooser fallback.

  Expected: both paths work and Notae remains usable after returning.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: open PDFs in Samsung Notes with fallback"
  ```

### Task 7: Add the Android OCR and PDF-page adapter

**Files:**
- Create: `apps/notae-android/src/android/pdf_pages.rs`
- Create: `apps/notae-android/src/android/ocr.rs`
- Modify: `apps/notae-android/src/main.rs`
- Create: `apps/notae-android/tests/ocr_adapter.rs`

**Interfaces:**
- `AndroidPdfDocument::open(uri: ContentUri) -> Result<Self, PdfError>`
- `AndroidPdfDocument::render_page(page: u32, dpi: u32) -> Result<GrayImage, PdfError>`
- `AndroidOcrProvider` implements the shared `OcrProvider` trait.

- [ ] **Step 1: Write failing adapter tests**

  Cover text-layer detection, page rendering failure, OCR progress, missing local language data, and per-page OCR errors. Use host-side fake Android services for deterministic state tests.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test ocr_adapter`

  Expected: FAIL because the Android adapters are not implemented.

- [ ] **Step 3: Implement the Android PDF/OCR adapter**

  Use Android's local PDF/page APIs behind the bridge, convert page bitmaps into the shared image type, and reuse the shared `TesseractProvider` and local language assets through the Android build. If native binding glue is required, keep it inside the `notae-ocr` Android module. Keep page processing off the UI thread and preserve per-page status.

- [ ] **Step 4: Run tests and the device OCR scenario**

  Run: `cargo test -p notae-android --test ocr_adapter`. On the tablet, index a text-layer PDF and an image-only PDF containing English and Polish printed text.

  Expected: existing text avoids OCR, image-only pages produce searchable text or a visible typed failure, and the UI remains responsive.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: add Android PDF and OCR adapters"
  ```

## Android-plan completion check

Run:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo apk build --release -p notae-android
```

Install the APK on the Samsung Galaxy Tab S10 FE+ and smoke-test folder selection, manual rescan, OCR progress, search, metadata editing, sidecar preview, and PDF handoff.
