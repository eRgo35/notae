# Notae Android App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Rust/Slint tablet app for Samsung Galaxy Tab S10 FE+, including folder selection, manual rescans, search, metadata editing, sidecar preview, OCR orchestration, and Samsung Notes handoff.

**Architecture:** `notae-android` consumes the shared domain/index crates and owns only UI state, Android lifecycle, Storage Access Framework integration, external intents, and Android OCR/PDF adapters. Slint renders the search-first interface; long-running work runs off the UI thread and reports typed progress events.

**Tech Stack:** Rust 2024, Slint, `android-activity`, `cargo-apk2`, Android Storage Access Framework, Android PDF intents, shared `notae-domain`/`notae-index` crates, and the shared `notae-ocr` provider contract. A thin Java `NativeActivity` subclass forwards SAF results to Rust.

**Spec:** `docs/superpowers/specs/2026-10-09-notae-design.md`

**Revision:** 2026-10-10 — Task 2 migrates packaging to cargo-apk2, adds the Java result bridge, and keeps the picker future off the blocked UI thread; recursive SAF enumeration is Task 3. Review this revision before implementation.

## Global Constraints

- Target device: Samsung Galaxy Tab S10 FE+.
- One persisted SAF tree permission and one active library root.
- No automatic launch rescan and no background watcher.
- PDFs are never copied, renamed, moved, or rendered by the app.
- Samsung Notes is preferred; Android chooser is the fallback.
- UI work MUST remain responsive while the picker is pending, scanning, hashing, importing, or running OCR.
- No network calls, telemetry, cloud OCR, or remote model downloads at runtime.
- Metadata edits update the local index and preserve manual provenance.
- Java is limited to the `NativeActivity` result bridge; no Gradle migration or business logic.
- The first smoke run must use the target tablet.

## Review Focus

- SAF result delivery reaches Rust through the Java `NativeActivity` subclass; the Slint event loop remains responsive while the picker is open; selected tree permissions survive relaunch and revocation is actionable.
- Recursive SAF enumeration returns nested PDFs and isolates per-file read failures.
- A rescan can be canceled or fail per file without freezing the UI.
- Search result ordering and exact-match ranking match the core contract.
- Metadata edits survive later rescans and sidecar imports.
- Samsung Notes absence falls back to the Android PDF chooser.

---

### Task 1: Create the Android/Slint application shell

**Status:** Shell, host startup tests, and APK build completed in commits `dc6b794`–`a7aef74` with `cargo-apk` 0.10. Task 2 replaces the packager; the device launch check remains pending.

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

- [x] **Step 1: Write a smoke test for app startup state**

  Assert that a fresh app exposes the setup state and does not start a rescan automatically.

- [x] **Step 2: Run the host smoke test and verify it fails**

  Run: `cargo test -p notae-android --test smoke`

  Expected: FAIL because the app crate and state model do not exist.

- [x] **Step 3: Implement the Slint shell and Android activity entrypoint**

  Configure the Android backend, target the target device ABI, and provide host-testable state transitions. Keep Android calls behind a platform module so core state tests run on Linux.

- [x] **Step 4: Run host tests and build the APK**

  Run: `cargo test -p notae-android --test smoke` and `cargo apk build --release -p notae-android`.

  Expected: PASS and a signed/debuggable APK is produced.

- [ ] **Step 5: Install and launch on the Galaxy Tab S10 FE+**

  After Task 2 migrates packaging, run `cargo apk2 run -p notae-android`. Expected: setup screen renders without a crash and remains responsive.

- [x] **Step 6: Commit**

  ```bash
  git add apps/notae-android Cargo.toml
  git commit -m "feat: add Slint Android app shell"
  ```

### Task 2: Replace cargo-apk with cargo-apk2 and bridge SAF results

**Files:**
- Modify: `apps/notae-android/Cargo.toml`
- Create: `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java`
- Modify: `apps/notae-android/android/AndroidManifest.xml`
- Modify: `apps/notae-android/src/android_main.rs`
- Modify: `apps/notae-android/NOTE-android-activity-result-jni.md`
- Modify: `apps/notae-android/NOTE-unsafe-android-main.md`

**Interfaces:**
- `NotaeActivity extends NativeActivity`; its instance-native method is `nativeOnActivityResult(int requestCode, int resultCode, String treeUri, int readGrantFlags)`.
- `decode_activity_result(result_code: i32, tree_uri: Option<String>, grant_flags: i32) -> Result<Option<String>, StorageError>` returns a selected tree URI only for `RESULT_OK` with a URI and read grant, `Ok(None)` for cancellation, and an error for malformed or denied results.
- The Rust JNI symbol `Java_org_notae_android_NotaeActivity_nativeOnActivityResult` uses the Activity receiver to access `ContentResolver`, persists the URI permission and root metadata before dispatching `PickerOutcome` through `PickerState::dispatch(request_code, outcome)`.
- `LibraryRootProvider::select_root()` completes only after the matching result has been validated and persisted. `handle_picker_response` schedules its future with `slint::spawn_local`; it MUST NOT block the Slint/Android main event loop while awaiting the picker.

- [x] **Step 1: Write failing result-contract tests**

  Add host tests for a successful result with read permission, cancellation, `RESULT_OK` without a URI, and `RESULT_OK` without the read grant. Assert the accepted URI; reuse the existing request-code dispatch tests rather than duplicating them.

  ```rust
  const RESULT_OK: i32 = -1;
  const RESULT_CANCELED: i32 = 0;
  const FLAG_GRANT_READ_URI_PERMISSION: i32 = 1;
  let uri = "content://provider/tree/music".to_owned();

  assert!(matches!(
      decode_activity_result(RESULT_OK, Some(uri.clone()), FLAG_GRANT_READ_URI_PERMISSION),
      Ok(Some(selected)) if selected == uri
  ));
  assert!(matches!(
      decode_activity_result(RESULT_CANCELED, None, 0),
      Ok(None)
  ));
  assert!(decode_activity_result(RESULT_OK, None, FLAG_GRANT_READ_URI_PERMISSION).is_err());
  assert!(decode_activity_result(RESULT_OK, Some(uri), 0).is_err());
  ```

- [x] **Step 2: Run focused tests and verify they fail**

  Run: `cargo test -p notae-android --test picker_fut`

  Expected: FAIL because Activity-result validation and the new boundary tests are not implemented.

- [x] **Step 3: Configure cargo-apk2 and implement the result bridge**

  Install the documented CLI version with `cargo install cargo-apk2 --version 1.4.0`. In `Cargo.toml`, set `java_sources = "android/src/main/java"` and `apk_name = "notae-android"`, declare `.NotaeActivity` as the launcher activity, set `application.has_code = true`, add the `MAIN`/`LAUNCHER` filter, and add activity metadata `android.app.lib_name = "notae_android"`. Keep the existing min/target SDK values.

  Implement the Java activity as a thin subclass. Its result override must call `super`, pass the URI string (or null) and only the read grant bit to Rust, and must not own root state:

  ```java
  package org.notae.android;

  import android.app.NativeActivity;
  import android.content.Intent;

  public final class NotaeActivity extends NativeActivity {
      private native void nativeOnActivityResult(
          int requestCode, int resultCode, String treeUri, int readGrantFlags);

      @Override
      protected void onActivityResult(int requestCode, int resultCode, Intent data) {
          super.onActivityResult(requestCode, resultCode, data);
          String uri = data == null || data.getData() == null
              ? null : data.getData().toString();
          int readFlags = data == null ? 0
              : data.getFlags() & Intent.FLAG_GRANT_READ_URI_PERMISSION;
          nativeOnActivityResult(requestCode, resultCode, uri, readFlags);
      }
  }
  ```

  Implement the JNI entrypoint in `saf.rs`. Validate the result with the tested helper, take persistable read permission through the Activity's `ContentResolver`, query the display name, write the root URI/name/ID to `SharedPreferences`, then dispatch success. Dispatch cancellation as `PickerOutcome::Cancelled`; dispatch platform and permission failures as `PickerOutcome::Platform`. Keep raw JNI/unsafe operations inside `saf.rs`; do not alter `ANativeActivityCallbacks`.
 
 In `android_main.rs`, remove `pollster::block_on` from `handle_picker_response`. Schedule the future with `slint::spawn_local(async move { ... })`, then call `apply_picker_result` when it resolves. The callback must return immediately so Slint and Android can process events, including the Activity result that wakes the future. Remove the now-unused Android app dependency on `pollster`; retain its host-test uses if still needed elsewhere.

- [x] **Step 4: Verify host behavior and packaged APK**

  Run: `cargo test -p notae-android --test picker_fut`, `cargo test -p notae-android --test storage_state`, and `cargo apk2 build --release -p notae-android`.

  Inspect `target/release/apk/notae-android.apk` with `$HOME/Android/Sdk/build-tools/36.0.0/aapt dump xmltree target/release/apk/notae-android.apk AndroidManifest.xml` and `unzip -l target/release/apk/notae-android.apk`. Expected: launcher is `org.notae.android.NotaeActivity`, its `android.app.lib_name` is `notae_android`, the APK contains `classes.dex` and `libnotae_android.so`, and host tests pass.

  Verified: `cargo test -p notae-android` (33 tests), `cargo apk2 check -p notae-android --target aarch64-linux-android`, release APK build, manifest and payload inspection. APK signing and alignment verification passed.

- [ ] **Step 5: Smoke-test the picker on the Galaxy Tab S10 FE+**

  Install with `cargo apk2 run -p notae-android`. Open the folder picker and confirm Notae's UI remains event-responsive while DocumentsUI is active; select a folder, cancel a second selection, relaunch and reopen the saved root, then revoke the permission in Android Settings and verify recovery UI. This hardware gate cannot be marked complete without the target tablet.

  Status: pending; `adb devices` reported no connected device. Repeat this scenario in the Android-plan completion check below.

- [x] **Step 6: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat(android): package Java SAF result bridge"
  ```

### Task 3: Enumerate PDFs recursively through the SAF tree

**Files:**
- Create: `apps/notae-android/src/saf_tree.rs`
- Modify: `apps/notae-android/src/lib.rs`
- Modify: `apps/notae-android/src/android/saf.rs`
- Create: `apps/notae-android/tests/saf_catalog.rs`
- Modify: `apps/notae-android/NOTE-android-activity-result-jni.md`

**Interfaces:**
- `DocumentRow { document_id: String, display_name: String, mime_type: String }` describes one SAF child.
- `DocumentTreeReader::children(parent_id: &str) -> Result<Vec<DocumentRow>, CatalogError>` lists one directory; `read_pdf(document_id: &str) -> std::io::Result<Vec<u8>>` reads one PDF.
- `enumerate_document_tree(reader: &impl DocumentTreeReader, root_id: &str) -> Result<Vec<PdfFile>, CatalogError>` is the host-testable recursive walk.
- `SafPdfCatalog::enumerate() -> Result<Vec<PdfFile>, CatalogError>` implements `notae_index::PdfCatalog` by adapting `ContentResolver` to `DocumentTreeReader`.

- [x] **Step 1: Write failing recursive-catalog tests**

  In `tests/saf_catalog.rs`, implement a fake `DocumentTreeReader` with a root PDF, a nested directory containing another PDF, a non-PDF, and one unreadable PDF. Assert the two readable PDFs have stable root-relative paths and fingerprints, the non-PDF is omitted, the unreadable entry has `FileCondition::Unreadable`, and a child-query failure returns `CatalogError` rather than a partial list.

  ```rust
  let files = enumerate_document_tree(&fake_tree, "root").unwrap();
  let paths: Vec<_> = files.iter().map(|file| file.path().as_str()).collect();
  assert_eq!(paths, ["root.pdf", "nested/practice.pdf", "locked.pdf"]);
  assert!(files.iter().filter(|file| file.path().as_str() != "locked.pdf").all(|file| file.fingerprint().is_some()));
  assert!(files.iter().find(|file| file.path().as_str() == "locked.pdf").is_some_and(|file| matches!(file.condition(), FileCondition::Unreadable)));
  assert!(files.iter().all(|file| file.filename() != "notes.txt"));
  ```

- [x] **Step 2: Run focused tests and verify they fail**

  Run: `cargo test -p notae-android --test saf_catalog`

  Expected: FAIL because `enumerate_document_tree` and the SAF adapter are not implemented.

- [x] **Step 3: Implement the SAF document-tree walk**

  Implement `DocumentTreeReader` for an Android adapter in `saf.rs`. Use `DocumentsContract.buildChildDocumentsUriUsingTree` with each parent document ID, read child rows into `DocumentRow`, and recurse into directory MIME types. Open PDF streams through `ContentResolver.openInputStream` and read them in fixed-size chunks through JNI; the host-testable walker computes fingerprints and root-relative `PdfFile` paths. Convert individual PDF read failures to `FileCondition::Unreadable`; return `CatalogError` on any directory-query failure so reconciliation cannot mistake an inaccessible subtree for deleted files. Close every cursor and input stream on success and error paths. Reject display names that are not one path segment; never convert a SAF URI into a filesystem path.

  ```java
  Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentId);
  Cursor cursor = resolver.query(children, projection, null, null, null);
  Uri document = DocumentsContract.buildDocumentUriUsingTree(treeUri, documentId);
  InputStream pdf = resolver.openInputStream(document);
  ```

- [x] **Step 4: Verify host catalog behavior**

  Run: `cargo test -p notae-android --test saf_catalog` and `cargo test -p notae-android --test storage_state`. Expected: nested PDFs have stable root-relative paths and fingerprints; non-PDF files are omitted; unreadable PDFs are represented as `FileCondition::Unreadable`; child-query failures abort enumeration rather than returning a partial list.

  Verified: `cargo test -p notae-android` passes (37 tests), including the SAF catalog and storage-state suites; Android-target `cargo apk2 check` and release APK build pass.

- [ ] **Step 5: Verify recursive enumeration on the Galaxy Tab S10 FE+**

  Choose a test tree containing PDFs at multiple depths and a non-PDF file, rescan, and verify nested PDFs are indexed while the non-PDF is ignored and per-file read failures are isolated. This hardware gate cannot be marked complete without the target tablet.

  Status: pending; `adb devices` reported no connected device. Repeat this scenario in the Android-plan completion check below.

- [x] **Step 6: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat(android): enumerate SAF PDF trees"
  ```

### Task 4: Build the search-first library UI

**Files:**
- Modify: `apps/notae-android/ui/app.slint`
- Create: `apps/notae-android/src/ui/library.rs`
- Create: `apps/notae-android/src/ui/search.rs`
- Create: `apps/notae-android/tests/search_state.rs`

**Interfaces:**
- `LibraryViewModel::set_query(query: String)`
- `LibraryViewModel::search() -> Result<Vec<SearchRow>, UiError>`
- `SearchRow` contains ID, title, composer/artist, tags, status, and stable note key.
- `LibraryViewModel` uses the host-testable search projection; existing `RequestRescan` and `OpenSettings` actions remain in the state machine. `OpenNote` and `EditMetadata` are added with the PDF/editor tasks when their handlers are implemented.

- [x] **Step 1: Write failing view-model tests**

  Assert empty state, result projection, numeric ordering, query propagation, exact-match ranking, and status display for OCR pending/error/conflict.

- [x] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test search_state`

  Expected: FAIL because the view model does not exist.

- [x] **Step 3: Implement the Slint tablet layout and view model**

  Use a prominent search field, readable touch-sized rows, restrained Material-3-inspired spacing, and an explicit Rescan action. `LibraryViewModel` queries `IndexStore` for ranked search or numeric-order browsing and projects stable note keys and status labels. The result-list binding is ready for rows; Task 5 connects it to rescan output. Do not add folder/category navigation.

- [x] **Step 4: Run host tests and build the APK**

  Run: `cargo test -p notae-android` and `cargo apk2 build --release -p notae-android`.

  Verified: all 41 package tests pass; Android-target `cargo apk2 check` and signed/aligned release APK build pass.

- [ ] **Step 5: Visually inspect the library on the Galaxy Tab S10 FE+**

  Verify portrait/landscape resizing, keyboard input, scrolling, and touch targets on the target tablet.

  Status: pending; `adb devices` reported no connected device. Repeat this scenario in the Android-plan completion check below.


- [x] **Step 6: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: add search-first tablet library"
  ```

### Task 5: Add manual rescan, progress, and sidecar import preview

**Files:**
- Create: `apps/notae-android/src/operations/rescan.rs`
- Create: `apps/notae-android/src/operations/import.rs`
- Create: `apps/notae-android/src/operations/events.rs`
- Modify: `apps/notae-android/src/android_main.rs`
- Modify: `apps/notae-android/src/storage.rs`
- Modify: `apps/notae-android/src/saf_tree.rs`
- Modify: `apps/notae-android/src/android/saf.rs`
- Modify: `apps/notae-android/src/ui.rs`
- Modify: `apps/notae-android/src/state.rs`
- Modify: `apps/notae-android/src/ui/search.rs`
- Modify: `apps/notae-android/ui/app.slint`
- Modify: `crates/notae-index/src/sidecar.rs`
- Create: `apps/notae-android/tests/operation_state.rs`
- Modify: `apps/notae-android/tests/saf_catalog.rs`

**Interfaces:**
- `OperationController::start_rescan()`
- `OperationController::cancel()`
- `OperationEvent::{Started, Progress, FileSucceeded, FileFailed, Completed, Canceled}`
- `ImportPreviewViewModel::load(preview: ImportPreview)`
- `ImportPreviewViewModel::apply() -> Result<ImportReport, UiError>`

- [x] **Step 1: Write failing operation-state tests**

  Cover explicit-only start, progress aggregation, cancellation, per-file failure, completion, sidecar preview, and confirmation-required apply.

- [x] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test operation_state`

  Expected: FAIL because operation orchestration is not implemented.

- [x] **Step 3: Implement background rescan and import preview**

  Run core reconciliation through an injected catalog/index interface on worker threads. Send non-blocking events to the UI. Keep preview generation read-only until the user confirms. Display new, changed, missing, hash-mismatch, duplicate-ID, OCR-availability, and preserved-manual-metadata sections. A rendered-page input is not available yet; wire the OCR provider in Task 8.

- [x] **Step 4: Run tests and the operation smoke scenario**

  Run: `cargo test -p notae-android --test operation_state`. Exercise the worker with a readable entry, an unreadable entry, and a root sidecar with one hash mismatch; keep OCR-provider execution for Task 8.

  Expected: valid files complete, the corrupt file is isolated, and import is not applied before confirmation.

  Verified: `cargo test -p notae-android` (48 tests), Android-target `cargo apk2 check`, and signed/aligned release APK build. Worker smoke tests cover explicit start, unreadable-file isolation, cancellation before writes, automatic root-sidecar discovery, hash-mismatch preview, and no sidecar write before confirmation. No rendered-page input exists in this task, so OCR execution/provider injection is deferred to Task 8; sidecar OCR availability and metadata are shown in the preview.

- [x] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: add manual rescan and sidecar preview"
  ```

  Commit: `3ebeb24 feat: add manual rescan and sidecar preview`.

### Task 6: Implement metadata editing

**Files:**
- Create: `apps/notae-android/src/ui/metadata_editor.rs`
- Modify: `apps/notae-android/src/ui.rs`
- Modify: `apps/notae-android/src/ui/search.rs`
- Modify: `apps/notae-android/src/operations/events.rs`
- Modify: `apps/notae-android/src/operations/rescan.rs`
- Modify: `apps/notae-android/src/android_main.rs`
- Modify: `apps/notae-android/ui/app.slint`
- Create: `apps/notae-android/tests/metadata_editor.rs`
- Modify: `apps/notae-android/tests/operation_state.rs`

**Interfaces:**
- `MetadataEditor::load(note_key) -> Result<EditableMetadata, UiError>`
- `MetadataEditor::save(input: EditableMetadata) -> Result<(), UiError>`
- `EditableMetadata { id, title, composer_or_artist, tags }`

- [x] **Step 1: Write failing editor tests**

  Cover valid edits, empty-title rejection, duplicate-ID rejection, tag normalization, unchanged filename, manual provenance marking, and persistence after a rescan.

- [x] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-android --test metadata_editor`

  Expected: FAIL because the editor is not implemented.

- [x] **Step 3: Implement the editor and save transaction**

  Validate through `notae-domain`, update the local index transactionally, preserve the PDF path and filename, and mark edited fields as manual provenance.

- [x] **Step 4: Run host tests and Android build**

  Run: `cargo test -p notae-android --test metadata_editor`, Android-target `cargo apk2 check`, and release APK build.

  Expected: edits remain and the import preview identifies them as preserved manual fields.

  Verified on host and Android build; device scenario remains pending because no tablet is attached. `cargo test -p notae-android` (54 tests), Android-target `cargo apk2 check`, and signed/aligned release APK build pass. Tests verify normalized edits, unique-ID rejection without partial writes, preserved file identity/status/OCR, manual provenance across rescan, sidecar preservation preview, and worker/UI event flow.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-android
  git commit -m "feat: edit indexed note metadata"
  ```

### Task 7: Implement Samsung Notes handoff and chooser fallback

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

### Task 8: Add the Android OCR and PDF-page adapter

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
cargo apk2 build --release -p notae-android
```

Install the APK on the Samsung Galaxy Tab S10 FE+ and smoke-test folder selection, manual rescan, OCR progress, search, metadata editing, sidecar preview, and PDF handoff.
