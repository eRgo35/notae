//! The first-page thumbnail cache and the rescan pass that fills it.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use notae_app_core::operations::events::OperationEvent;
use notae_app_core::operations::rescan::OperationController;
use notae_app_core::pdf_pages::PdfError;
use notae_app_core::storage::LibraryCatalog;
use notae_app_core::thumbnails::ThumbnailCache;
use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, IndexStore, PdfCatalog, PdfFile};

const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn cache() -> (tempfile::TempDir, ThumbnailCache) {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache = ThumbnailCache::new(dir.path().join("thumbnails")).expect("cache");
    (dir, cache)
}

#[test]
fn only_content_keys_get_a_path() {
    let (_dir, cache) = cache();
    assert!(cache.path_for(KEY).is_some());
    assert!(cache.path_for("../escape").is_none());
}

#[test]
fn ensure_renders_once_and_reuses_the_file() {
    let (_dir, cache) = cache();
    let first = cache
        .ensure(KEY, |out: &Path| {
            std::fs::write(out, b"png").unwrap();
            Ok(true)
        })
        .expect("render");
    assert!(first.as_deref().is_some_and(Path::is_file));
    let second = cache
        .ensure(KEY, |_out: &Path| panic!("must not render twice"))
        .expect("reuse");
    assert_eq!(first, second);
}

#[test]
fn failed_render_leaves_no_partial_file() {
    let (_dir, cache) = cache();
    let result = cache.ensure(KEY, |out: &Path| {
        std::fs::write(out, b"half").unwrap();
        Err(PdfError::Platform("boom".to_owned()))
    });
    assert!(result.is_err());
    assert!(cache.existing(KEY).is_none());
    assert_eq!(
        std::fs::read_dir(cache.path_for(KEY).unwrap().parent().unwrap())
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn declined_render_is_not_cached() {
    let (_dir, cache) = cache();
    assert_eq!(
        cache
            .ensure(KEY, |_out: &Path| Ok(false))
            .expect("declined"),
        None
    );
    assert!(cache.existing(KEY).is_none());
}

#[test]
fn prune_removes_thumbnails_of_deleted_scores() {
    let (_dir, cache) = cache();
    let other = "b".repeat(64);
    for key in [KEY, other.as_str()] {
        cache
            .ensure(key, |out: &Path| {
                std::fs::write(out, b"png").unwrap();
                Ok(true)
            })
            .unwrap();
    }
    let keep: HashSet<String> = [KEY.to_owned()].into();
    assert_eq!(cache.prune(&keep).expect("prune"), 1);
    assert!(cache.existing(KEY).is_some());
    assert!(cache.existing(&other).is_none());
}

struct ThumbnailCatalog {
    file: PdfFile,
}

impl PdfCatalog for ThumbnailCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(vec![self.file.clone()])
    }
}

impl LibraryCatalog for ThumbnailCatalog {
    fn render_thumbnail(&self, _path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        std::fs::write(out, b"png").expect("write thumbnail");
        Ok(true)
    }
}

#[test]
fn rescan_writes_a_thumbnail_for_each_indexed_score() {
    let (_dir, cache) = cache();
    let digest = Sha256Digest::from_bytes([0xaa; 32]);
    let file = PdfFile::new(
        RelativePdfPath::new("0012 - Clair de lune.pdf").expect("path"),
        PdfFingerprint::new(digest, 1_024, None),
        "0012 - Clair de lune.pdf".to_owned(),
    );
    let controller = OperationController::with_thumbnails(
        IndexStore::open(":memory:").expect("index"),
        Box::new(ThumbnailCatalog { file }),
        cache.clone(),
    )
    .expect("start test worker");
    controller.start_rescan().expect("rescan");
    loop {
        match controller
            .recv_event_timeout(Duration::from_secs(5))
            .expect("event")
        {
            OperationEvent::Completed { .. } => break,
            OperationEvent::Failed { failure } => panic!("{failure:?}"),
            _ => {}
        }
    }
    assert!(cache.existing(&digest.to_string()).is_some());
}
