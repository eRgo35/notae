use std::time::Duration;

use notae_android::operations::events::OperationEvent;
use notae_android::operations::rescan::OperationController;
use notae_android::pdf_intent::ContentUri;
use notae_android::pdf_pages::{
    AndroidPdfDocument, PdfDocumentMetadata, PdfError, PdfPageBackend, classify_text_layer,
};
use notae_android::storage::LibraryCatalog;
use notae_domain::{IndexStatus, PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, IndexStore, OcrText, PdfCatalog, PdfFile};
use sha2::{Digest, Sha256};

struct TextCatalog {
    file: PdfFile,
    extracted: Result<Option<String>, PdfError>,
}

impl PdfCatalog for TextCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(vec![self.file.clone()])
    }
}

impl LibraryCatalog for TextCatalog {
    fn extract_text_layer(&self, _path: &RelativePdfPath) -> Result<Option<String>, PdfError> {
        self.extracted.clone()
    }
}

struct FakePdfBackend {
    metadata: Result<PdfDocumentMetadata, PdfError>,
}

impl PdfPageBackend for FakePdfBackend {
    fn inspect(&mut self, _uri: &ContentUri) -> Result<PdfDocumentMetadata, PdfError> {
        self.metadata.clone()
    }
}

fn document(text_layer: Option<&str>) -> AndroidPdfDocument {
    AndroidPdfDocument::open_with_backend(
        ContentUri::new("content://provider/tree/document/score.pdf").expect("content URI"),
        Box::new(FakePdfBackend {
            metadata: Ok(PdfDocumentMetadata {
                page_count: 1,
                text_layer: text_layer.map(str::to_owned),
            }),
        }),
    )
    .expect("one-page PDF opens")
}

#[test]
fn android_pdf_document_exposes_normalized_searchable_text() {
    let document = document(Some(" Etude \n\t in C "));

    assert_eq!(document.extract_text(), Some("Etude in C"));
}

#[test]
fn blank_or_unavailable_text_layer_is_not_searchable_text() {
    for text in [None, Some(" \n\t ")] {
        let document = document(text);
        assert_eq!(document.extract_text(), None);
    }
}

#[test]
fn zero_page_pdf_is_rejected_during_open() {
    let result = AndroidPdfDocument::open_with_backend(
        ContentUri::new("content://provider/tree/document/empty.pdf").expect("content URI"),
        Box::new(FakePdfBackend {
            metadata: Ok(PdfDocumentMetadata {
                page_count: 0,
                text_layer: None,
            }),
        }),
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("a PDF without pages is invalid"),
    };
    assert_eq!(error, PdfError::EmptyDocument);
}

#[test]
fn missing_text_layer_stays_ocr_pending_without_existing_searchable_text() {
    let outcome = classify_text_layer(Ok(None), false);

    assert_eq!(outcome.status, IndexStatus::OcrPending);
    assert_eq!(outcome.text, None);
    assert_eq!(outcome.failure, None);
}

#[test]
fn missing_text_layer_preserves_existing_sidecar_ocr() {
    let outcome = classify_text_layer(Ok(Some(" \n ".to_owned())), true);

    assert_eq!(outcome.status, IndexStatus::Ready);
    assert_eq!(outcome.text, None);
    assert_eq!(outcome.failure, None);
}

#[test]
fn text_layer_failure_is_visible_as_ocr_incomplete() {
    let outcome = classify_text_layer(
        Err(PdfError::Platform("PDF text extraction failed".to_owned())),
        false,
    );

    assert_eq!(outcome.status, IndexStatus::OcrIncomplete);
    assert_eq!(outcome.text, None);
    assert_eq!(
        outcome.failure,
        Some(PdfError::Platform("PDF text extraction failed".to_owned()))
    );
}

fn readable_pdf(path: &str, bytes: &[u8]) -> PdfFile {
    PdfFile::new(
        RelativePdfPath::new(path).expect("valid library-relative path"),
        PdfFingerprint::new(
            Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
            u64::try_from(bytes.len()).expect("fixture size fits u64"),
            None,
        ),
        path.to_owned(),
    )
}

fn wait_for_scan(controller: &OperationController) -> Vec<notae_android::ui::search::SearchRow> {
    controller.start_rescan().expect("start explicit rescan");
    loop {
        match controller.recv_event_timeout(Duration::from_secs(2)) {
            Some(OperationEvent::Completed { rows, .. }) => return rows,
            Some(OperationEvent::Failed { reason }) => panic!("rescan failed: {reason}"),
            Some(_) => {}
            None => panic!("rescan did not finish"),
        }
    }
}

#[test]
fn rescan_marks_image_only_pdf_pending_without_overwriting_search_state() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let database = directory.path().join("index.sqlite");
    let bytes = b"%PDF-1.4\nimage only";
    let file = readable_pdf("12 - Image Only.pdf", bytes);
    drop(IndexStore::open(&database).expect("create test index"));

    let controller = OperationController::new(
        IndexStore::open(&database).expect("open index"),
        Box::new(TextCatalog {
            file,
            extracted: Ok(None),
        }),
    );
    let rows = wait_for_scan(&controller);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "OCR pending");
    drop(controller);
    let store = IndexStore::open(&database).expect("reopen index");
    assert!(
        store
            .search("visible song lyric", 10)
            .expect("search OCR")
            .is_empty()
    );
}

#[test]
fn rescan_preserves_sidecar_ocr_when_platform_has_no_text_layer() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let database = directory.path().join("index.sqlite");
    let bytes = b"%PDF-1.4\nimage only";
    let file = readable_pdf("12 - Image Only.pdf", bytes);
    let hash = file.fingerprint().expect("readable PDF").sha256();
    let note = notae_domain::IndexedNote::new(
        file.path().clone(),
        file.fingerprint().expect("readable PDF").clone(),
        notae_domain::NoteMetadata::new("Image Only", None, []).expect("metadata"),
    );
    let store = IndexStore::open(&database).expect("open test index");
    store
        .reindex_note_with_ocr(&note, Some(&OcrText::new("sidecar searchable text")))
        .expect("seed sidecar text");
    drop(store);

    let controller = OperationController::new(
        IndexStore::open(&database).expect("reopen index for worker"),
        Box::new(TextCatalog {
            file,
            extracted: Ok(None),
        }),
    );
    let rows = wait_for_scan(&controller);

    assert_eq!(rows[0].status, "Ready");
    drop(controller);
    let store = IndexStore::open(&database).expect("reopen index after scan");
    assert_eq!(
        store
            .search("sidecar searchable text", 10)
            .expect("search imported OCR")
            .len(),
        1
    );
    assert_eq!(
        store.ocr_text(&hash).expect("read OCR text"),
        Some(OcrText::new("sidecar searchable text"))
    );
}

#[test]
fn rescan_reports_pdf_text_extraction_failure_and_marks_record_incomplete() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let database = directory.path().join("index.sqlite");
    let file = readable_pdf(
        "13 - Broken Text Layer.pdf",
        b"%PDF-1.4\ntext extraction fails",
    );
    let controller = OperationController::new(
        IndexStore::open(&database).expect("open index"),
        Box::new(TextCatalog {
            file,
            extracted: Err(PdfError::Platform("text layer unavailable".to_owned())),
        }),
    );
    controller.start_rescan().expect("start explicit rescan");

    let mut saw_file_failure = false;
    loop {
        match controller.recv_event_timeout(Duration::from_secs(2)) {
            Some(OperationEvent::FileFailed { path, reason }) => {
                assert_eq!(path, "13 - Broken Text Layer.pdf");
                assert_eq!(
                    reason,
                    "PDF platform operation failed: text layer unavailable"
                );
                saw_file_failure = true;
            }
            Some(OperationEvent::Completed { summary, rows }) => {
                assert!(saw_file_failure);
                assert_eq!(summary.failures, 1);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].status, "OCR incomplete");
                break;
            }
            Some(OperationEvent::Failed { reason }) => panic!("rescan failed: {reason}"),
            Some(_) => {}
            None => panic!("rescan did not finish"),
        }
    }
}
