//! `PlatformCatalog`: the rescan's catalog over a `PlatformLibrary`.

#[path = "support/fake_platform.rs"]
mod fake_platform;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fake_platform::{FakePlatform, entry, readable, sha256_hex};
use notae_app_core::operations::events::OperationEvent;
use notae_app_core::operations::rescan::OperationController;
use notae_app_core::pdf_pages::PdfError;
use notae_app_core::platform::{DocumentContent, PlatformCatalog, PlatformError};
use notae_app_core::storage::{LibraryCatalog, StorageError};
use notae_app_core::text::Failure;
use notae_domain::{EpochTimestamp, IndexStatus, PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, IndexStore, PdfCatalog, PdfFile, Reconciler};
use sha2::{Digest, Sha256};

const HANDLE: &str = "content://tree/library";

fn catalog(platform: &Arc<FakePlatform>) -> PlatformCatalog {
    PlatformCatalog::new(Arc::clone(platform) as _, HANDLE.to_owned())
}

fn path(value: &str) -> RelativePdfPath {
    RelativePdfPath::new(value).expect("valid library-relative path")
}

#[test]
fn catalog_maps_entries_to_pdf_files() {
    let bytes = b"%PDF-1.4\nautumn";
    let platform = Arc::new(FakePlatform::default());
    let mut renamed = readable("Jazz/12 - Autumn Leaves.pdf", bytes);
    renamed.entry.display_name = "12 - Autumn Leaves (shared).pdf".to_owned();
    platform.set_entries(Ok(vec![
        renamed,
        readable("../escape.pdf", b"%PDF-1.4\nescape"),
        entry("locked.pdf", DocumentContent::Unreadable, 0),
        entry("broken.pdf", DocumentContent::InvalidPdf, 7),
    ]));
    let catalog = catalog(&platform);

    let files = catalog.enumerate().expect("platform listed the library");

    assert_eq!(
        files,
        [
            PdfFile::with_condition(
                path("Jazz/12 - Autumn Leaves.pdf"),
                "12 - Autumn Leaves (shared).pdf".to_owned(),
                FileCondition::Readable(PdfFingerprint::new(
                    Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
                    u64::try_from(bytes.len()).expect("fixture fits in u64"),
                    None,
                )),
            ),
            PdfFile::with_condition(
                path("locked.pdf"),
                "locked.pdf".to_owned(),
                FileCondition::Unreadable
            ),
            PdfFile::with_condition(
                path("broken.pdf"),
                "broken.pdf".to_owned(),
                FileCondition::InvalidPdf
            ),
        ]
    );
    assert_eq!(
        platform.calls(),
        [
            format!("enumerate {HANDLE}"),
            format!("hash {HANDLE} Jazz/12 - Autumn Leaves.pdf"),
            format!("hash {HANDLE} locked.pdf"),
            format!("hash {HANDLE} broken.pdf"),
        ]
    );
    assert_eq!(catalog.take_rejected_paths(), ["../escape.pdf"]);
    assert!(catalog.take_rejected_paths().is_empty(), "taken once");

    // The rescan reports the dropped entry as a failed file and counts it.
    let controller = OperationController::new(
        IndexStore::open(":memory:").expect("open test index"),
        Box::new(catalog),
    )
    .expect("start test worker");
    controller.start_rescan().expect("start scan");
    let mut failed = Vec::new();
    let summary = loop {
        match controller
            .recv_event_timeout(Duration::from_secs(2))
            .expect("operation event arrives")
        {
            OperationEvent::FileFailed { path, .. } => failed.push(path),
            OperationEvent::Completed { summary, .. } => break summary,
            OperationEvent::Failed { failure } => panic!("scan failed: {failure:?}"),
            _ => {}
        }
    };
    failed.sort();
    assert_eq!(failed, ["../escape.pdf", "broken.pdf", "locked.pdf"]);
    assert_eq!(summary.added, 1);
    assert_eq!(summary.failures, 3);
}

#[test]
fn unreadable_entry_becomes_inaccessible() {
    let platform = Arc::new(FakePlatform::default());
    let catalog = catalog(&platform);
    let mut index = IndexStore::open(":memory:").expect("open test index");
    platform.set_entries(Ok(vec![readable(
        "12 - Autumn Leaves.pdf",
        b"%PDF-1.4\nautumn",
    )]));
    let files = catalog.enumerate().expect("first listing");
    let plan = Reconciler::plan(&index, &files).expect("plan first scan");
    Reconciler::apply(plan, &mut index).expect("apply first scan");

    platform.set_entries(Ok(vec![entry(
        "12 - Autumn Leaves.pdf",
        DocumentContent::Unreadable,
        0,
    )]));
    let files = catalog.enumerate().expect("second listing");
    assert_eq!(files[0].condition(), &FileCondition::Unreadable);
    let plan = Reconciler::plan(&index, &files).expect("plan second scan");
    Reconciler::apply(plan, &mut index).expect("apply second scan");

    let notes = index.list_all().expect("read index");
    assert_eq!(notes.len(), 1, "the record keeps its identity");
    assert_eq!(notes[0].status(), IndexStatus::Inaccessible);
}

#[test]
fn permission_lost_maps_to_storage_permission_lost() {
    assert_eq!(
        StorageError::from(PlatformError::PermissionLost),
        StorageError::PermissionLost
    );
    assert_eq!(
        StorageError::from(PlatformError::Unavailable {
            detail: "provider crashed".to_owned(),
        }),
        StorageError::Other("provider crashed".to_owned())
    );

    // A revoke seen while scanning stays a distinct failure, not a generic unreadable root.
    let platform = Arc::new(FakePlatform::default());
    let catalog = catalog(&platform);
    platform.set_entries(Err(PlatformError::PermissionLost));
    let error = catalog.enumerate().expect_err("revoked root");
    assert_eq!(error, CatalogError::PermissionLost);
    assert_eq!(Failure::from(&error), Failure::LibraryPermissionLost);

    platform.set_entries(Err(PlatformError::Unavailable {
        detail: "provider crashed".to_owned(),
    }));
    assert_eq!(
        catalog.enumerate(),
        Err(CatalogError::Failed("provider crashed".to_owned()))
    );
}

#[test]
fn catalog_forwards_text_and_thumbnail_requests() {
    let platform = Arc::new(FakePlatform::default());
    let catalog = catalog(&platform);
    let score = path("Jazz/12 - Autumn Leaves.pdf");

    platform.set_text(Ok(Some("Autumn Leaves".to_owned())));
    assert_eq!(
        catalog.extract_text_layer(&score),
        Ok(Some("Autumn Leaves".to_owned()))
    );
    platform.set_text(Err(PlatformError::Unavailable {
        detail: "renderer closed".to_owned(),
    }));
    assert_eq!(
        catalog.extract_text_layer(&score),
        Err(PdfError::Platform("renderer closed".to_owned()))
    );

    platform.set_thumbnail(Ok(true));
    assert_eq!(
        catalog.render_thumbnail(&score, Path::new("/cache/abc.png.part")),
        Ok(true)
    );
    platform.set_thumbnail(Err(PlatformError::PermissionLost));
    assert_eq!(
        catalog.render_thumbnail(&score, Path::new("/cache/abc.png.part")),
        Err(PdfError::Platform(
            PlatformError::PermissionLost.to_string()
        ))
    );

    assert_eq!(
        platform.calls(),
        [
            format!("text {HANDLE} Jazz/12 - Autumn Leaves.pdf"),
            format!("text {HANDLE} Jazz/12 - Autumn Leaves.pdf"),
            format!("thumbnail {HANDLE} Jazz/12 - Autumn Leaves.pdf /cache/abc.png.part"),
            format!("thumbnail {HANDLE} Jazz/12 - Autumn Leaves.pdf /cache/abc.png.part"),
        ]
    );
}

#[test]
fn list_converts_millis_and_hashes_nothing() {
    let platform = Arc::new(FakePlatform::default());
    let mut listed = readable("Jazz/Autumn.pdf", b"%PDF-1.4\nautumn");
    listed.entry.modified_millis = Some(1_700_000_000_123);
    platform.set_entries(Ok(vec![listed, readable("old.pdf", b"%PDF-1.4\nold")]));
    let catalog = catalog(&platform);

    let listed = catalog.list().expect("platform listed the library");

    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].path, path("Jazz/Autumn.pdf"));
    assert_eq!(listed[0].filename, "Autumn.pdf");
    assert_eq!(listed[0].size, 15);
    assert_eq!(
        listed[0].modified,
        EpochTimestamp::new(1_700_000_000, 123_000_000)
    );
    assert_eq!(listed[1].modified, None);
    assert_eq!(platform.calls(), [format!("enumerate {HANDLE}")]);
}

#[test]
fn hash_maps_content_to_condition() {
    let platform = Arc::new(FakePlatform::default());
    let bytes = b"%PDF-1.4\nautumn";
    platform.set_hashes(HashMap::from([
        (
            "ok.pdf".to_owned(),
            Ok(DocumentContent::Sha256 {
                hex: sha256_hex(bytes),
            }),
        ),
        ("locked.pdf".to_owned(), Ok(DocumentContent::Unreadable)),
        (
            "bad.pdf".to_owned(),
            Ok(DocumentContent::Sha256 {
                hex: "not hex".to_owned(),
            }),
        ),
        ("broken.pdf".to_owned(), Ok(DocumentContent::InvalidPdf)),
        ("revoked.pdf".to_owned(), Err(PlatformError::PermissionLost)),
    ]));
    let catalog = catalog(&platform);
    let modified = EpochTimestamp::new(5, 0);

    assert_eq!(
        catalog.hash(&path("ok.pdf"), 15, modified),
        Ok(FileCondition::Readable(PdfFingerprint::new(
            Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
            15,
            modified,
        )))
    );
    assert_eq!(
        catalog.hash(&path("locked.pdf"), 1, None),
        Ok(FileCondition::Unreadable)
    );
    assert_eq!(
        catalog.hash(&path("bad.pdf"), 1, None),
        Ok(FileCondition::Unreadable)
    );
    assert_eq!(
        catalog.hash(&path("broken.pdf"), 1, None),
        Ok(FileCondition::InvalidPdf)
    );
    assert_eq!(
        catalog.hash(&path("revoked.pdf"), 1, None),
        Err(CatalogError::PermissionLost)
    );
    assert_eq!(platform.calls()[0], format!("hash {HANDLE} ok.pdf"));
}

#[test]
fn read_file_passes_bytes_and_absence() {
    let platform = Arc::new(FakePlatform::default());
    platform.set_files(HashMap::from([("sidecar.json".to_owned(), vec![1, 2, 3])]));
    let catalog = catalog(&platform);

    assert_eq!(catalog.read_file("sidecar.json"), Ok(Some(vec![1, 2, 3])));
    assert_eq!(catalog.read_file("missing.json"), Ok(None));
    assert_eq!(
        platform.calls(),
        [
            format!("read {HANDLE} sidecar.json"),
            format!("read {HANDLE} missing.json")
        ]
    );
}
