# Notae Linux Indexer and OCR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Linux-only Rust CLI that indexes manually prepared PDFs, performs local multilingual printed-text OCR, and writes the portable `.notae` sidecar consumed by Android.

**Architecture:** The CLI reuses `notae-domain` and `notae-index`. Linux-specific adapters enumerate ordinary filesystem paths, compute fingerprints, extract existing text, render image-only pages, and invoke a local OCR provider. The CLI writes a versioned SQLite sidecar plus manifest without modifying source PDFs.

**Tech Stack:** Rust 2024, `clap`, `tracing-subscriber`, `image`, `pdfium-render` for Linux PDF rendering/text access, Tesseract/Leptonica through a Rust adapter, shared `notae-domain`/`notae-index`, and fixture-based integration tests.

**Spec:** `docs/superpowers/specs/2026-10-09-notae-design.md`

## Global Constraints

- Linux is the only PC platform in the first release.
- Command: `notae-index <library-root>`.
- Source PDFs MUST NOT be renamed, moved, copied, or deleted.
- Output is `.notae/index.sqlite` and `.notae/manifest.json`.
- OCR is printed-text-only and fully local.
- Automatic language/script detection is the default.
- Initial language data MUST cover English, Polish, and Latin; additional installed packs are supported.
- A single PDF/OCR failure MUST NOT abort the batch.
- Re-running the CLI MUST reuse unchanged fingerprints and preserve manual metadata in an existing sidecar.
- Runtime operation MUST NOT require network access.

## Review Focus

- The CLI handles a flat directory and nested storage paths without treating folders as collections.
- `2 - ...pdf` sorts before `10 - ...pdf` in generated metadata.
- A corrupt PDF is reported while other PDFs finish.
- OCR text is reproducible enough for sidecar import and search.
- Language detection failure is visible and never silently produces a falsely ready record.

---

### Task 1: Add the Linux CLI and filesystem catalog

**Files:**
- Create: `apps/notae-index/Cargo.toml` (package name: `notae-index-cli`)
- Create: `apps/notae-index/src/main.rs`
- Create: `apps/notae-index/src/args.rs`
- Create: `apps/notae-index/src/catalog.rs`
- Create: `apps/notae-index/tests/cli_args.rs`
- Modify: workspace `Cargo.toml`

**Interfaces:**
- `Cli { library_root: PathBuf, verbose: bool }`
- `FilesystemCatalog::new(root: PathBuf) -> Result<Self, CatalogError>`
- `FilesystemCatalog::enumerate() -> Result<Vec<PdfFile>, CatalogError>`
- `run(cli: Cli) -> Result<RunReport, CliError>`

- [ ] **Step 1: Write failing CLI and enumeration tests**

  Cover required root argument, missing root, recursive `.pdf`/`.PDF` discovery, ignoring non-PDF files, stable relative paths, and per-file permission errors.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index-cli --test cli_args`

  Expected: FAIL because the CLI crate does not exist.

- [ ] **Step 3: Implement arguments and filesystem catalog**

  Use relative paths from the library root, calculate byte metadata, and stream file errors into the run report rather than aborting enumeration. Keep output deterministic for fixtures.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-index-cli --test cli_args`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-index Cargo.toml
  git commit -m "feat: add Linux indexer filesystem catalog"
  ```

### Task 2: Add PDF text extraction and page rendering

**Files:**
- Create: `apps/notae-index/src/pdf.rs`
- Create: `apps/notae-index/src/image.rs`
- Create: `apps/notae-index/tests/pdf_fixtures.rs`
- Modify: `apps/notae-index/Cargo.toml`

**Interfaces:**
- `LinuxPdfDocument::open(path: &Path) -> Result<Self, PdfError>`
- `LinuxPdfDocument::extract_text() -> Result<Option<String>, PdfError>`
- `LinuxPdfDocument::render_page(page: u32, dpi: u32) -> Result<GrayImage, PdfError>`
- `LinuxPdfDocument::page_count() -> Result<u32, PdfError>`

- [ ] **Step 1: Add fixture PDFs and write failing adapter tests**

  Include one text-layer PDF, one image-only PDF, one multi-page PDF, and one corrupt file. Assert page counts, existing text detection, image rendering dimensions, and isolated corrupt-file errors.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index-cli --test pdf_fixtures`

  Expected: FAIL because the PDF adapter is not implemented.

- [ ] **Step 3: Implement the Linux PDF adapter**

  Use `pdfium-render` to open PDFs, extract an existing text layer when available, and render image-only pages at a fixed OCR DPI. Convert rendered pages to the image type consumed by the OCR provider. Document the required Pdfium runtime asset for Linux development and release packaging.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-index-cli --test pdf_fixtures`

  Expected: PASS with Pdfium available; corrupt files return typed errors.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-index
  git commit -m "feat: extract and render Linux PDFs"
  ```

### Task 3: Implement multilingual automatic OCR

**Files:**
- Create: `crates/notae-ocr/src/image.rs`
- Create: `crates/notae-ocr/src/lib.rs`
- Create: `crates/notae-ocr/src/language.rs`
- Create: `crates/notae-ocr/src/tesseract.rs`
- Create: `crates/notae-ocr/tests/language_and_ocr.rs`
- Modify: workspace `Cargo.toml`

**Interfaces:**
- `type OcrImage = image::GrayImage`
- `LanguagePolicy::automatic(installed: InstalledLanguages) -> Self`
- `LanguageDetector::detect(image: &OcrImage) -> Result<DetectedLanguage, OcrError>`
- `TesseractProvider::new(config: TesseractConfig) -> Result<Self, OcrError>`
- `impl OcrProvider for TesseractProvider`
- `OcrText { text, detected_languages, confidence, warnings }`

- [ ] **Step 1: Write failing OCR tests**

  Use deterministic English and Polish fixtures. Assert that automatic mode selects available packs, Latin-script content is searchable, confidence/warnings are recorded, missing packs produce a typed warning, and OCR errors do not panic.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-ocr --test language_and_ocr`

  Expected: FAIL because the OCR crate/provider is not implemented.

- [ ] **Step 3: Implement the Tesseract-backed provider**

  Discover installed language data locally, use orientation/script detection to narrow candidates, run the matching language packs, normalize whitespace, and persist detected language/confidence metadata. Do not download language data. Keep the provider behind the shared trait so Android can supply a compatible adapter later.

- [ ] **Step 4: Run focused OCR tests with local language data**

  Run: `cargo test -p notae-ocr --test language_and_ocr`

  Expected: PASS when the required local packs are installed; missing-pack cases pass by asserting the typed warning path.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/notae-ocr
  git commit -m "feat: add local multilingual OCR provider"
  ```

### Task 4: Build the incremental sidecar pipeline

**Files:**
- Create: `apps/notae-index/src/pipeline.rs`
- Create: `apps/notae-index/src/report.rs`
- Modify: `apps/notae-index/src/main.rs`
- Create: `apps/notae-index/tests/pipeline.rs`

**Interfaces:**
- `IndexPipeline::run(catalog, store, ocr, output) -> Result<RunReport, PipelineError>`
- `RunReport { processed, reused, moved, deleted, ocr_failed, pdf_failed, conflicts }`
- `ProgressEvent::{Started, FileStarted, PageProgress, FileCompleted, FileFailed, Finished}`

- [ ] **Step 1: Write failing pipeline tests**

  Cover a first run, rerun with unchanged PDFs, same-hash rename, changed content, text-layer reuse without OCR, image-only OCR, duplicate ID, corrupt PDF, and partial completion.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index-cli --test pipeline`

  Expected: FAIL because the pipeline is not implemented.

- [ ] **Step 3: Implement incremental processing**

  Reuse unchanged fingerprints, extract text before OCR, render/OCR only image-only or unsearchable pages, collect per-file errors, preserve existing manual provenance, and publish progress events. Do not write the final sidecar until the run has a consistent report.

- [ ] **Step 4: Run the focused tests**

  Run: `cargo test -p notae-index-cli --test pipeline`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add apps/notae-index
  git commit -m "feat: add incremental Linux indexing pipeline"
  ```

### Task 5: Write the sidecar and CLI documentation

**Files:**
- Modify: `apps/notae-index/src/main.rs`
- Create: `apps/notae-index/tests/sidecar_output.rs`
- Create: `apps/notae-index/README.md`
- Modify: repository `README.md`

**Interfaces:**
- CLI exit code `0` for completed runs, non-zero only when the command itself cannot produce a valid report.
- Per-file failures appear in the report and manifest; they do not make a partially successful run look complete.

- [ ] **Step 1: Write failing output tests**

  Assert `.notae/index.sqlite` and `manifest.json` creation, schema version, relative paths, hashes, OCR status, detected languages, tool version, stable output, and non-zero command failure for an unusable root.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `cargo test -p notae-index-cli --test sidecar_output`

  Expected: FAIL because CLI output and documentation are incomplete.

- [ ] **Step 3: Implement output and user documentation**

  Write the portable SQLite sidecar through `notae-index`, emit a manifest with schema/tool metadata, print a concise summary plus actionable per-file failures, and document Linux prerequisites, local OCR language data, invocation, reruns, and Android import.

- [ ] **Step 4: Run the focused tests and CLI smoke scenario**

  Run: `cargo test -p notae-index-cli --test sidecar_output`, then `cargo run -p notae-index-cli -- <fixture-root>`

  Expected: sidecar files are created, unchanged reruns reuse work, and failures are visible.

- [ ] **Step 5: Run the full workspace checks**

  Run:

  ```bash
  cargo test --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  cargo run -p notae-index-cli -- --help
  ```

  Expected: all tests pass, no warnings remain, and help documents the command.

- [ ] **Step 6: Commit**

  ```bash
  git add apps/notae-index crates/notae-ocr README.md
  git commit -m "feat: add Linux OCR sidecar indexer"
  ```

## Linux-plan completion check

Run the CLI against a representative local library containing text PDFs, image-only scans, English/Polish text, a duplicate ID, a corrupt PDF, and a renamed unchanged file. Verify the generated sidecar imports through the Android preview without network access.
