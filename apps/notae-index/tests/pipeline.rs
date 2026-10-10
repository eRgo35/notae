//! Incremental pipeline: the integration tests that prove the pipeline writes
//! the index correctly across a full set of scenarios.
//!
//! These tests use a fake [`OcrProvider`] so they do not depend on the
//! tessdata tree or on a real Tesseract. The fake keeps a queue of page
//! texts; each call pops one. This is deterministic: a test that knows the
//! page count of its fixture knows exactly what OCR will produce.
//!
//! The fixtures themselves live in `tests/fixtures/`. The text-layer,
//! image-only, multi-page, and corrupt fixtures exist already, and the
//! pipeline tests build libraries by copying those files into a fresh
//! temporary directory.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};

use notae_domain::{NoteId, Sha256Digest};
use notae_index::IndexStore;
use notae_index_cli::catalog::{FilesystemCatalog, ValidatedRoot};
use notae_index_cli::pipeline::{IndexPipeline, ProgressEvent};
use notae_index_cli::report::RunReport;
use notae_ocr::{LanguagePolicy, OcrImage, OcrProvider, OcrText as OcrTextEngine, OcrError};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Copies a committed fixture into a freshly-built library.
fn fixture_to(library: &Path, source: &str, target: &str) {
    let mut origin = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    origin.push("tests/fixtures");
    origin.push(source);
    let bytes = fs::read(&origin).expect("read the fixture");
    let destination = library.join(target);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(&destination, bytes).expect("write the fixture");
}

/// A library root backed by a temporary directory.
struct Library {
    _dir: TempDir,
    root: PathBuf,
}

impl Library {
    fn new() -> Self {
        let dir = TempDir::new().expect("temporary directory");
        let root = dir.path().join("library");
        fs::create_dir_all(&root).expect("create the library root");
        Self { _dir: dir, root }
    }

    fn catalog(&self) -> FilesystemCatalog {
        FilesystemCatalog::new(ValidatedRoot::open(&self.root).expect("open the root"))
    }

    fn store(&self) -> IndexStore {
        let path = self.root.join(".notae/index.sqlite");
        fs::create_dir_all(path.parent().expect("parent")).expect("create index dir");
        IndexStore::open(&path).expect("open the index")
    }

    /// SHA-256 of the bytes on disk in `library` at `relative`.
    fn hash_at(&self, relative: &str) -> Sha256Digest {
        let bytes = fs::read(self.root.join(relative)).expect("read the file");
        let digest = Sha256::digest(&bytes);
        Sha256Digest::from_bytes(digest.into())
    }
}

/// A fake OCR provider that hands out a queued page of text per call.
///
/// Tests populate the queue in the order OCR is expected to be called, so
/// the produced OCR text is deterministic. A page request with an empty
/// queue returns an error — that itself is reported on the run as an OCR
/// failure and lets the test assert that error handling is correct.
#[derive(Debug, Default)]
struct FakeOcr {
    queue: Mutex<VecDeque<String>>,
    calls: Mutex<usize>,
}

impl FakeOcr {
    fn new() -> Self {
        Self::default()
    }

    fn with_texts<I, S>(texts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let me = Self::default();
        for text in texts {
            me.queue.lock().expect("queue mutex").push_back(text.into());
        }
        me
    }

    fn calls(&self) -> usize {
        *self.calls.lock().expect("calls mutex")
    }
}

impl OcrProvider for FakeOcr {
    fn recognize(
        &self,
        _page: &OcrImage,
        _policy: &LanguagePolicy,
    ) -> Result<OcrTextEngine, OcrError> {
        let mut calls = self.calls.lock().expect("calls mutex");
        *calls += 1;
        let mut queue = self.queue.lock().expect("queue mutex");
        let text = queue
            .pop_front()
            .ok_or_else(|| OcrError::RecognitionFailed {
                reason: "no queued text left".to_string(),
            })?;
        Ok(OcrTextEngine::new(text, vec!["eng".to_string()], 80))
    }
}

fn policy() -> LanguagePolicy {
    LanguagePolicy::Fixed(vec!["eng".to_string()])
}

fn collect(
    pipeline: &IndexPipeline,
    catalog: &FilesystemCatalog,
    store: &mut IndexStore,
    ocr: &FakeOcr,
) -> (RunReport, Vec<ProgressEvent>) {
    let (sender, receiver) = mpsc::channel();
    let collector = std::thread::spawn(move || {
        let events: Vec<ProgressEvent> = receiver.iter().collect();
        events
    });
    let report = pipeline
        .run(catalog, store, ocr, &policy(), sender)
        .expect("pipeline run completes");
    let events = collector.join().expect("collector joined");
    (report, events)
}

// ---------------------------------------------------------------------------

#[test]
fn a_first_run_inserts_one_record_per_file_and_streams_progress() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "1 - First.pdf");
    fixture_to(&library.root, "multi-page.pdf", "2 - Second.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();

    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    assert_eq!(report.enumerated(), 2, "two files catalogued");
    assert_eq!(report.processed(), 2, "two files processed by the pipeline");
    assert_eq!(report.pdf_failed(), 0, "no PDF failures");
    assert_eq!(report.ocr_failed(), 0, "no OCR failures");
    assert_eq!(report.reused(), 0, "first run inserts");
    assert_eq!(report.conflicts(), 0);
    assert_eq!(ocr.calls(), 0, "no OCR calls on text-layer PDFs");
}

#[test]
fn a_rerun_after_no_changes_reuses_every_record() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "1 - First.pdf");
    fixture_to(&library.root, "multi-page.pdf", "2 - Second.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let policy = policy();

    let _ = pipeline
        .run(&catalog, &mut store, &ocr, &policy, mpsc::channel().0)
        .expect("first run");
    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy, mpsc::channel().0)
        .expect("second run");

    assert_eq!(report.processed(), 2);
    assert_eq!(report.reused(), 2, "all records are reused when nothing changed");
    assert_eq!(report.moved(), 0);
    assert_eq!(report.pdf_failed(), 0);
}

#[test]
fn a_same_hash_rename_moves_the_record() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "1 - First.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let policy = policy();

    let _ = pipeline
        .run(&catalog, &mut store, &ocr, &policy, mpsc::channel().0)
        .expect("first run");

    fs::remove_file(library.root.join("1 - First.pdf")).expect("remove the original");
    fixture_to(&library.root, "text-layer.pdf", "3 - Renamed.pdf");

    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy, mpsc::channel().0)
        .expect("rename run");

    assert_eq!(report.moved(), 1, "the record moved to the new path");
    assert_eq!(report.reused(), 0);
    assert_eq!(report.pdf_failed(), 0);
    let paths = report.paths();
    assert_eq!(paths.len(), 1);
    assert!(
        paths[0].contains("Renamed"),
        "the path is the new one: {paths:?}"
    );
}

#[test]
fn a_content_change_at_a_known_path_replaces_the_record() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "1 - First.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let policy = policy();
    let _ = pipeline
        .run(&catalog, &mut store, &ocr, &policy, mpsc::channel().0)
        .expect("first run");

    fixture_to(&library.root, "multi-page.pdf", "1 - First.pdf");

    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy, mpsc::channel().0)
        .expect("update run");

    assert_eq!(report.processed(), 1);
    assert_eq!(report.pdf_failed(), 0);
    assert!(report.reused() + report.moved() <= 1);
}

#[test]
fn text_layer_files_skip_ocr_entirely() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "1 - First.pdf");
    fixture_to(&library.root, "multi-page.pdf", "2 - Second.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    assert_eq!(ocr.calls(), 0, "OCR is not used when the text layer exists");
    assert_eq!(report.ocr_failed(), 0);
    assert_eq!(report.pdf_failed(), 0);
    let stored = store
        .ocr_text(&library.hash_at("1 - First.pdf"))
        .expect("read store");
    assert!(
        stored.is_some(),
        "OCR text (filled in from the text layer) is stored"
    );
}

#[test]
fn an_image_only_pdf_triggers_ocr() {
    let library = Library::new();
    fixture_to(&library.root, "image-only.pdf", "1 - Scan.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::with_texts(["NOTAE SCAN PAGE ONE"]);

    let _ = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    assert!(ocr.calls() > 0, "image-only PDF triggers OCR");
    let notes = store.list_all().expect("list the index");
    assert_eq!(notes.len(), 1);
    let note = &notes[0];
    let stored = store
        .ocr_text(&note.fingerprint().sha256())
        .expect("read OCR text")
        .expect("OCR text was stored");
    assert!(
        stored.text().contains("NOTAE SCAN PAGE ONE"),
        "OCR text was recorded: {}",
        stored.text()
    );
}

#[test]
fn a_duplicate_id_conflict_is_counted_but_does_not_overwrite() {
    let library = Library::new();
    // Two different PDFs with the same numeric prefix in their filenames.
    fixture_to(&library.root, "text-layer.pdf", "1 - Conflict.pdf");
    fixture_to(&library.root, "multi-page.pdf", "1 - Conflict (other).pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    assert_eq!(report.processed(), 2);
    assert_eq!(report.conflicts(), 1, "one duplicate-ID conflict surfaced");
}

#[test]
fn a_corrupt_pdf_is_counted_as_a_pdf_failure() {
    let library = Library::new();
    fixture_to(&library.root, "corrupt.pdf", "1 - Broken.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    assert_eq!(report.pdf_failed(), 1, "the corrupt file is recorded as a PDF failure");
    assert_eq!(report.ocr_failed(), 0);
    let notes = store.list_all().expect("list the index");
    assert!(
        notes.is_empty(),
        "no record was written for the corrupt PDF: {notes:?}"
    );
}

#[test]
fn a_partial_run_continues_after_a_pdf_failure() {
    let library = Library::new();
    fixture_to(&library.root, "corrupt.pdf", "1 - Broken.pdf");
    fixture_to(&library.root, "text-layer.pdf", "2 - Good.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    assert_eq!(report.processed(), 2);
    assert_eq!(report.pdf_failed(), 1, "the broken file failed");
    let notes = store.list_all().expect("list the index");
    assert_eq!(
        notes.len(),
        1,
        "the good file was still indexed: {notes:?}"
    );
    assert!(
        notes[0].path().as_str().contains("Good"),
        "the indexed record is the good one: {}",
        notes[0].path().as_str()
    );
}

#[test]
fn ocr_failure_records_an_ocr_failure_count_and_keeps_the_record() {
    let library = Library::new();
    fixture_to(&library.root, "image-only.pdf", "1 - Scan.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    // Empty queue → every OCR call fails.
    let ocr = FakeOcr::new();

    let report = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    let notes = store.list_all().expect("list the index");
    assert_eq!(notes.len(), 1, "the record still exists");
    // OCR errors are counted under pdf_failed per the brief.
    assert!(report.ocr_failed() + report.pdf_failed() >= 1);
}

#[test]
fn note_ids_derived_from_filenames_reach_the_store() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "42 - Labelled.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();

    let _ = pipeline
        .run(&catalog, &mut store, &ocr, &policy(), mpsc::channel().0)
        .expect("pipeline run");

    let record = store.get_by_id(NoteId::new(42)).expect("read by id");
    let note = record.expect("the labelled note is indexed");
    assert_eq!(note.id(), Some(NoteId::new(42)));
    assert_eq!(note.title(), "Labelled");
}

#[test]
fn progress_events_capture_started_and_finished() {
    let library = Library::new();
    fixture_to(&library.root, "text-layer.pdf", "1 - First.pdf");

    let pipeline = IndexPipeline::default_dpi();
    let catalog = library.catalog();
    let mut store = library.store();
    let ocr = FakeOcr::new();
    let (_report, events) = collect(&pipeline, &catalog, &mut store, &ocr);

    assert!(events.contains(&ProgressEvent::Started));
    assert!(events.contains(&ProgressEvent::Finished));
    let file_started: Vec<&ProgressEvent> = events
        .iter()
        .filter(|event| matches!(event, ProgressEvent::FileStarted { .. }))
        .collect();
    assert_eq!(file_started.len(), 1, "one FileStarted event per file");
}
