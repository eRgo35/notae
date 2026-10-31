//! `PlatformCatalog`: the rescan's catalog over a `PlatformLibrary`.

#[path = "support/fake_platform.rs"]
mod fake_platform;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fake_platform::{FakePlatform, entry, readable};
use notae_app_core::operations::events::OperationEvent;
use notae_app_core::operations::rescan::OperationController;
use notae_app_core::pdf_pages::PdfError;
use notae_app_core::platform::{DocumentContent, PlatformCatalog, PlatformError};
use notae_app_core::storage::{LibraryCatalog, StorageError};
use notae_app_core::text::Failure;
use notae_domain::{IndexStatus, PdfFingerprint, RelativePdfPath, Sha256Digest};
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
    renamed.display_name = "12 - Autumn Leaves (shared).pdf".to_owned();
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
    assert_eq!(platform.calls(), [format!("enumerate {HANDLE}")]);
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
            message: "provider crashed".to_owned(),
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
        message: "provider crashed".to_owned(),
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
        message: "renderer closed".to_owned(),
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
