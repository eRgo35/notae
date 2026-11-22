//! The fast scan: unchanged files are not read again, and the PC sidecar stands in for reading.

#[path = "support/fake_platform.rs"]
mod fake_platform;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fake_platform::{FakePlatform, Listed, readable, sha256_hex};
use notae_app_core::operations::events::{OperationEvent, RescanSummary};
use notae_app_core::operations::rescan::OperationController;
use notae_app_core::platform::PlatformCatalog;
use notae_app_core::thumbnails::ThumbnailCache;
use notae_app_core::ui::metadata_editor::EditableMetadata;
use notae_domain::{
    IndexStatus, IndexedNote, RelativePdfPath, SIDECAR_FILE, SIDECAR_VERSION, SidecarEntry,
    SidecarFile,
};
use notae_index::IndexStore;

const HANDLE: &str = "content://tree/library";

const ALPHA: &[u8] = b"%PDF-1.4\nalpha";
const BETA: &[u8] = b"%PDF-1.4\nbeta!";
const GAMMA: &[u8] = b"%PDF-1.4\ngamma";

/// A test library: the fake platform, the worker over it, and where its state lives.
struct Library {
    platform: Arc<FakePlatform>,
    controller: Option<OperationController>,
    database: std::path::PathBuf,
    thumbnails: std::path::PathBuf,
    _directory: tempfile::TempDir,
}

impl Library {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("index.sqlite");
        let thumbnails = directory.path().join("thumbnails");
        let platform = Arc::new(FakePlatform::default());
        platform.set_thumbnail(Ok(true));
        let controller = start(&platform, &database, &thumbnails);
        Self {
            platform,
            controller: Some(controller),
            database,
            thumbnails,
            _directory: directory,
        }
    }

    /// Stops the worker, lets `edit` change its index, and starts a new worker over it.
    fn restart(&mut self, edit: impl FnOnce(&IndexStore)) {
        drop(self.controller.take());
        edit(&IndexStore::open(&self.database).expect("reopen index"));
        self.controller = Some(start(&self.platform, &self.database, &self.thumbnails));
    }

    fn controller(&self) -> &OperationController {
        self.controller.as_ref().expect("worker is running")
    }

    /// Runs one scan; returns its summary and the platform calls it made.
    fn scan(&self) -> (RescanSummary, Vec<String>) {
        let before = self.platform.calls().len();
        self.controller().start_rescan().expect("start rescan");
        let summary = loop {
            match self.controller().recv_event_timeout(Duration::from_secs(5)) {
                Some(OperationEvent::Completed { summary, .. }) => break summary,
                Some(OperationEvent::Failed { failure }) => panic!("rescan failed: {failure:?}"),
                Some(_) => {}
                None => panic!("rescan did not finish"),
            }
        };
        (summary, self.platform.calls()[before..].to_vec())
    }

    fn save_metadata(&self, metadata: EditableMetadata) {
        self.controller()
            .save_metadata(metadata)
            .expect("send metadata");
        loop {
            match self.controller().recv_event_timeout(Duration::from_secs(5)) {
                Some(OperationEvent::MetadataSaved { .. }) => return,
                Some(OperationEvent::MetadataFailed { failure }) => {
                    panic!("metadata save failed: {failure:?}")
                }
                Some(_) => {}
                None => panic!("metadata save did not finish"),
            }
        }
    }

    /// Stops the worker and opens its index.
    fn into_store(mut self) -> (IndexStore, Self) {
        drop(self.controller.take());
        (
            IndexStore::open(&self.database).expect("reopen index"),
            self,
        )
    }
}

fn start(platform: &Arc<FakePlatform>, database: &Path, thumbnails: &Path) -> OperationController {
    OperationController::with_thumbnails(
        IndexStore::open(database).expect("open index"),
        Box::new(PlatformCatalog::new(
            Arc::clone(platform) as _,
            HANDLE.to_owned(),
        )),
        ThumbnailCache::new(thumbnails.to_path_buf()).expect("thumbnail cache"),
    )
    .expect("start worker")
}

fn listed(path: &str, bytes: &[u8], millis: Option<i64>) -> Listed {
    let mut item = readable(path, bytes);
    item.entry.modified_millis = millis;
    item
}

fn three_files() -> Vec<Listed> {
    vec![
        listed("1 - Alpha.pdf", ALPHA, Some(1_700_000_000_000)),
        listed("2 - Beta.pdf", BETA, Some(1_700_000_001_000)),
        listed("3 - Gamma.pdf", GAMMA, Some(1_700_000_002_000)),
    ]
}

fn count(calls: &[String], method: &str) -> usize {
    let prefix = format!("{method} ");
    calls
        .iter()
        .filter(|call| call.starts_with(&prefix))
        .count()
}

fn sidecar_entry(path: &str, bytes: &[u8]) -> SidecarEntry {
    SidecarEntry {
        path: path.to_owned(),
        size: u64::try_from(bytes.len()).expect("fixture fits in u64"),
        sha256: sha256_hex(bytes),
        number: None,
        title: None,
        composer: None,
        tags: Vec::new(),
        text: Some(format!("sidecar text of {path}")),
    }
}

fn sidecar(entries: Vec<SidecarEntry>) -> Vec<u8> {
    serde_json::to_vec(&SidecarFile {
        version: SIDECAR_VERSION,
        generator: "test".to_owned(),
        scores: entries,
    })
    .expect("sidecar serializes")
}

fn serve(platform: &FakePlatform, files: Vec<(String, Vec<u8>)>) {
    platform.set_files(files.into_iter().collect::<HashMap<_, _>>());
}

fn note_at(store: &IndexStore, path: &str) -> IndexedNote {
    store
        .list_all()
        .expect("list notes")
        .into_iter()
        .find(|note| note.path().as_str() == path)
        .expect("note is indexed")
}

fn tags(note: &IndexedNote) -> Vec<String> {
    note.metadata().tags().iter().cloned().collect()
}

#[test]
fn first_scan_hashes_each_file_once() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));

    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 3);
    assert_eq!(count(&calls, "text"), 3);
    assert_eq!(count(&calls, "thumbnail"), 3);
}

#[test]
fn second_scan_reads_nothing() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));
    library.scan();

    library.platform.set_entries(Ok(three_files()));
    let (summary, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 0);
    assert_eq!(count(&calls, "text"), 0);
    assert_eq!(count(&calls, "thumbnail"), 0);
    assert_eq!(summary, RescanSummary::default());
}

#[test]
fn changed_size_or_time_is_hashed() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));
    library.scan();

    library.platform.set_entries(Ok(vec![
        listed(
            "1 - Alpha.pdf",
            b"%PDF-1.4\nalpha, longer",
            Some(1_700_000_000_000),
        ),
        listed("2 - Beta.pdf", BETA, Some(1_700_000_009_000)),
        listed("3 - Gamma.pdf", GAMMA, Some(1_700_000_002_000)),
    ]));
    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 2);
    assert!(!calls.iter().any(|call| call.contains("3 - Gamma.pdf")));
}

#[test]
fn missing_time_is_always_hashed() {
    let library = Library::new();
    let files = || vec![listed("1 - Alpha.pdf", ALPHA, None)];
    library.platform.set_entries(Ok(files()));
    let (_, first) = library.scan();
    library.platform.set_entries(Ok(files()));
    let (_, second) = library.scan();

    assert_eq!(count(&first, "hash"), 1);
    assert_eq!(count(&second, "hash"), 1);
}

#[test]
fn sidecar_match_reads_no_pdf() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));
    let mut alpha = sidecar_entry("1 - Alpha.pdf", ALPHA);
    alpha.number = Some(101);
    alpha.title = Some("Adeste Fideles".to_owned());
    alpha.composer = Some("Wade".to_owned());
    alpha.tags = vec!["Z katalogu".to_owned()];
    alpha.text = Some("Adeste fideles laeti triumphantes".to_owned());
    let beta = sidecar_entry("2 - Beta.pdf", BETA);
    let thumbnail = b"\x89PNG alpha thumbnail".to_vec();
    serve(
        &library.platform,
        vec![
            (SIDECAR_FILE.to_owned(), sidecar(vec![alpha, beta])),
            (
                format!(".notae/thumbnails/{}.png", sha256_hex(ALPHA)),
                thumbnail.clone(),
            ),
            (
                format!(".notae/thumbnails/{}.png", sha256_hex(BETA)),
                b"\x89PNG beta thumbnail".to_vec(),
            ),
        ],
    );

    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 1);
    assert_eq!(count(&calls, "text"), 1);
    assert_eq!(count(&calls, "thumbnail"), 1);
    assert!(
        calls
            .iter()
            .filter(|call| call.starts_with("hash ") || call.starts_with("text "))
            .all(|call| call.ends_with("3 - Gamma.pdf"))
    );
    let (store, library) = library.into_store();
    let hits = store.search("triumphantes", 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].note.path().as_str(), "1 - Alpha.pdf");
    let note = note_at(&store, "1 - Alpha.pdf");
    assert_eq!(note.title(), "Adeste Fideles");
    assert_eq!(note.id().map(|id| id.get()), Some(101));
    assert_eq!(note.metadata().composer_or_artist(), Some("Wade"));
    assert_eq!(tags(&note), ["Z katalogu"]);
    let cached = library
        .thumbnails
        .join(format!("{}.png", sha256_hex(ALPHA)));
    assert_eq!(std::fs::read(cached).expect("thumbnail cached"), thumbnail);
}

#[test]
fn size_mismatch_ignores_the_entry() {
    let library = Library::new();
    let bytes = b"%PDF-1.4\nxy"; // 11 bytes
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", bytes, Some(1))]));
    let mut entry = sidecar_entry("1 - Alpha.pdf", bytes);
    entry.size = 10;
    entry.title = Some("From the sidecar".to_owned());
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry]))],
    );

    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 1);
    let (store, _library) = library.into_store();
    assert_eq!(note_at(&store, "1 - Alpha.pdf").title(), "Alpha");
}

#[test]
fn changed_entry_is_reapplied_to_an_unchanged_file() {
    let library = Library::new();
    let mut entry = sidecar_entry("1 - Alpha.pdf", ALPHA);
    entry.tags = vec!["Z katalogu".to_owned()];
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry.clone()]))],
    );
    library.scan();

    entry.tags.push("Komunia".to_owned());
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry]))],
    );
    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 0);
    assert_eq!(count(&calls, "text"), 0);
    let (store, _library) = library.into_store();
    assert!(tags(&note_at(&store, "1 - Alpha.pdf")).contains(&"Komunia".to_owned()));
}

#[test]
fn manual_edit_survives_a_changed_entry() {
    let library = Library::new();
    let mut entry = sidecar_entry("1 - Alpha.pdf", ALPHA);
    entry.composer = Some("Bach".to_owned());
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry.clone()]))],
    );
    library.scan();
    library.save_metadata(EditableMetadata {
        note_key: sha256_hex(ALPHA),
        id: "1".to_owned(),
        title: "Alpha".to_owned(),
        composer_or_artist: "Ja".to_owned(),
        tags: Vec::new(),
    });

    entry.composer = Some("Mozart".to_owned());
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry]))],
    );
    library.scan();

    let (store, _library) = library.into_store();
    assert_eq!(
        note_at(&store, "1 - Alpha.pdf")
            .metadata()
            .composer_or_artist(),
        Some("Ja")
    );
}

#[test]
fn broken_sidecar_is_ignored_and_reported() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), b"{".to_vec())],
    );

    let (summary, calls) = library.scan();

    assert!(summary.sidecar_ignored);
    assert_eq!(summary.added, 3);
    assert_eq!(count(&calls, "hash"), 3);
}

#[test]
fn empty_sidecar_text_falls_back_to_the_text_layer() {
    let library = Library::new();
    let mut entry = sidecar_entry("1 - Alpha.pdf", ALPHA);
    entry.text = None;
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry]))],
    );

    let (summary, calls) = library.scan();

    assert!(!summary.sidecar_ignored);
    assert_eq!(count(&calls, "hash"), 0);
    assert_eq!(count(&calls, "text"), 1);
}

#[test]
fn sidecar_is_read_after_the_listing() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));

    let (_, calls) = library.scan();

    let listing = calls
        .iter()
        .position(|call| call.starts_with("enumerate "))
        .expect("listed");
    let read = calls
        .iter()
        .position(|call| *call == format!("read {HANDLE} {SIDECAR_FILE}"))
        .expect("sidecar read");
    assert!(listing < read);
}

#[test]
fn an_interrupted_note_is_finished_by_the_next_scan() {
    let mut library = Library::new();
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    library.scan();
    // A scan killed after reconciliation committed leaves the note as it was added.
    library.restart(|store| {
        let note = note_at(store, "1 - Alpha.pdf");
        store
            .reindex_note(&note.with_status(IndexStatus::Discovered))
            .expect("reset status");
    });

    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "text"), 1);
    let (store, _library) = library.into_store();
    assert_eq!(
        note_at(&store, "1 - Alpha.pdf").status(),
        IndexStatus::OcrPending
    );
}

#[test]
fn a_missing_cached_thumbnail_is_rendered_again() {
    let library = Library::new();
    library.platform.set_entries(Ok(three_files()));
    library.scan();
    std::fs::remove_file(
        library
            .thumbnails
            .join(format!("{}.png", sha256_hex(ALPHA))),
    )
    .expect("thumbnail was cached");

    library.platform.set_entries(Ok(three_files()));
    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 0);
    assert_eq!(count(&calls, "text"), 0);
    let rendered: Vec<_> = calls
        .iter()
        .filter(|call| call.starts_with("thumbnail "))
        .collect();
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains("1 - Alpha.pdf"));
}

#[test]
fn an_entry_for_other_content_is_not_applied_to_an_unchanged_file() {
    let library = Library::new();
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    library.scan();

    let mut entry = sidecar_entry("1 - Alpha.pdf", ALPHA);
    entry.sha256 = sha256_hex(BETA);
    entry.title = Some("Wrong".to_owned());
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(
        &library.platform,
        vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry]))],
    );
    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "hash"), 0);
    let (store, _library) = library.into_store();
    assert_eq!(note_at(&store, "1 - Alpha.pdf").title(), "Alpha");
    let path = RelativePdfPath::new("1 - Alpha.pdf").expect("valid path");
    assert_eq!(store.sidecar_digest(&path).expect("read digest"), None);
}

/// Scans `1 - Alpha.pdf` with a sidecar entry once at `first`, then again at `second`.
fn rescan_sidecar_file(first: Option<i64>, second: Option<i64>) -> Vec<String> {
    let library = Library::new();
    library
        .platform
        .set_text(Ok(Some("pdf text layer".to_owned())));
    let mut entry = sidecar_entry("1 - Alpha.pdf", ALPHA);
    entry.text = Some("Adeste fideles laeti triumphantes".to_owned());
    let files = vec![(SIDECAR_FILE.to_owned(), sidecar(vec![entry]))];
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, first)]));
    serve(&library.platform, files.clone());
    library.scan();

    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, second)]));
    serve(&library.platform, files);
    let (_, calls) = library.scan();

    let (store, _library) = library.into_store();
    let hits = store.search("triumphantes", 10).expect("search");
    assert_eq!(hits.len(), 1, "the sidecar text is still searchable");
    calls
}

#[test]
fn sidecar_text_survives_a_changed_time() {
    let calls = rescan_sidecar_file(Some(1), Some(2));

    assert_eq!(count(&calls, "hash"), 0);
    assert_eq!(count(&calls, "text"), 0);
}

#[test]
fn sidecar_text_survives_a_provider_without_times() {
    let calls = rescan_sidecar_file(None, None);

    assert_eq!(count(&calls, "hash"), 0);
    assert_eq!(count(&calls, "text"), 0);
}

#[test]
fn a_missing_cached_thumbnail_is_copied_from_an_applied_sidecar() {
    let library = Library::new();
    let thumbnail = b"\x89PNG alpha thumbnail".to_vec();
    let files = vec![
        (
            SIDECAR_FILE.to_owned(),
            sidecar(vec![sidecar_entry("1 - Alpha.pdf", ALPHA)]),
        ),
        (
            format!(".notae/thumbnails/{}.png", sha256_hex(ALPHA)),
            thumbnail.clone(),
        ),
    ];
    let cached = library
        .thumbnails
        .join(format!("{}.png", sha256_hex(ALPHA)));
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(&library.platform, files.clone());
    library.scan();

    // A changed time, then an unchanged file: both copy the applied entry's thumbnail.
    for millis in [2, 2] {
        std::fs::remove_file(&cached).expect("thumbnail was cached");
        library
            .platform
            .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(millis))]));
        serve(&library.platform, files.clone());
        let (_, calls) = library.scan();

        assert_eq!(count(&calls, "thumbnail"), 0);
        assert_eq!(std::fs::read(&cached).expect("thumbnail cached"), thumbnail);
    }
}

#[test]
fn an_interrupted_sidecar_note_is_finished_from_its_entry() {
    let mut library = Library::new();
    let files = vec![(
        SIDECAR_FILE.to_owned(),
        sidecar(vec![sidecar_entry("1 - Alpha.pdf", ALPHA)]),
    )];
    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(1))]));
    serve(&library.platform, files.clone());
    library.scan();
    library.restart(|store| {
        let note = note_at(store, "1 - Alpha.pdf");
        store
            .reindex_note(&note.with_status(IndexStatus::Discovered))
            .expect("reset status");
    });

    library
        .platform
        .set_entries(Ok(vec![listed("1 - Alpha.pdf", ALPHA, Some(2))]));
    serve(&library.platform, files);
    let (_, calls) = library.scan();

    assert_eq!(count(&calls, "text"), 0);
    let (store, _library) = library.into_store();
    assert_eq!(
        note_at(&store, "1 - Alpha.pdf").status(),
        IndexStatus::Ready
    );
}
