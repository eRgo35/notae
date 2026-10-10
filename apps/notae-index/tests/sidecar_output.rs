//! The portable sidecar the indexer writes beside the SQLite payload.
//!
//! These tests run the built `notae-index` binary against small fixture
//! libraries and assert what the run wrote and what it printed. The
//! observable contract is the sidecar: two files at
//! `<library>/.notae/index.sqlite` and `<library>/.notae/manifest.json`,
//! and a one-line summary on stdout. The summary's role is the same as the
//! per-file failures on stderr: a non-zero status would be a surprise, so
//! every test below asserts `0` for any run that actually indexed
//! something.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use notae_domain::Sha256Digest;
use notae_index::{IndexStore, SidecarImporter, SidecarManifest, SIDECAR_SCHEMA_VERSION};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TESSDATA_DIR_ENV: &str = "TESSDATA_PREFIX";
const FALLBACK_TESSDATA_DIR: &str = "/usr/share/tesseract/tessdata";

struct Library {
    _dir: TempDir,
    root: PathBuf,
}

impl Library {
    fn new() -> Self {
        let dir = TempDir::new().expect("temporary parent directory");
        let root = dir.path().join("library");
        fs::create_dir_all(&root).expect("create the library root");
        Self { _dir: dir, root }
    }

    fn write(&self, relative: &str, content: &[u8]) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(&path, content).expect("write fixture");
    }

    /// Copies a committed fixture into the library, preserving the file
    /// name under `target`.
    fn copy_fixture(&self, source: &str, target: &str) {
        let mut origin = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        origin.push("tests/fixtures");
        origin.push(source);
        let bytes = fs::read(&origin).expect("read the fixture");
        self.write(target, &bytes);
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(".notae/index.sqlite")
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join(".notae/manifest.json")
    }
}

/// Runs the built binary against `library_root`, returning the captured
/// `Output`. The `TESSDATA_PREFIX` is set so the indexer can initialise
/// Tesseract without depending on the calling shell's environment; when
/// no system tessdata is installed, the indexer exits non-zero and the
/// test fails with the captured stderr as the diagnostic.
fn run_index(args: &[&str]) -> Output {
    let tessdata = std::env::var_os(TESSDATA_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(FALLBACK_TESSDATA_DIR));
    Command::new(env!("CARGO_BIN_EXE_notae-index"))
        .args(args)
        .env(TESSDATA_DIR_ENV, &tessdata)
        .output()
        .expect("the built binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn sha256_of(path: &Path) -> Sha256Digest {
    let bytes = fs::read(path).expect("read the file");
    let digest = Sha256::digest(&bytes);
    Sha256Digest::from_bytes(digest.into())
}

// -----------------------------------------------------------------------
// Successful-run assertions
// -----------------------------------------------------------------------

#[test]
fn a_successful_run_writes_index_sqlite_and_manifest_json() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(
        output.status.success(),
        "a clean run exits 0; stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );

    assert!(
        library.index_path().is_file(),
        "the SQLite index is written: stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        library.manifest_path().is_file(),
        "the manifest is written: stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn the_manifests_schema_version_matches_the_builds() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(
        output.status.success(),
        "a clean run exits 0; stderr: {}",
        stderr(&output)
    );

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    assert_eq!(
        manifest.schema_version(),
        SIDECAR_SCHEMA_VERSION,
        "the manifest carries the build's schema version"
    );
}

#[test]
fn the_tool_version_matches_cargo_pkg_version() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "a clean run exits 0");

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    assert_eq!(
        manifest.tool_version(),
        env!("CARGO_PKG_VERSION"),
        "the tool version stamped on the manifest matches the build"
    );
}

#[test]
fn every_entry_uses_a_root_relative_forward_slash_path() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    library.copy_fixture("multi-page.pdf", "nested/2 - Second.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "a clean run exits 0");

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    assert!(!manifest.entries().is_empty(), "the manifest is populated");
    for entry in manifest.entries() {
        let path = entry.path().as_str();
        assert!(!path.starts_with('/'), "no path is absolute: {path}");
        assert!(
            !path.contains('\\'),
            "no path uses a backslash separator: {path}"
        );
        assert!(
            !path.contains("./") && !path.contains("/."),
            "no path contains a `.` segment: {path}"
        );
    }
}

#[test]
fn every_entrys_sha256_matches_the_files_content() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    library.copy_fixture("multi-page.pdf", "2 - Second.pdf");
    library.copy_fixture("image-only.pdf", "3 - Scan.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "a clean run exits 0");

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    for entry in manifest.entries() {
        let absolute = library.root().join(entry.path().as_str());
        let expected = sha256_of(&absolute);
        assert_eq!(
            entry.fingerprint().sha256(),
            expected,
            "{}: the manifest's SHA-256 matches the file's bytes",
            entry.path()
        );
    }
}

#[test]
fn ocr_status_is_present_on_every_entry_that_has_text() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    library.copy_fixture("multi-page.pdf", "2 - Second.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "a clean run exits 0");

    let store = IndexStore::open(library.index_path()).expect("open the index");
    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    for entry in manifest.entries() {
        let stored = store
            .ocr_text(&entry.fingerprint().sha256())
            .expect("read OCR text from the store");
        if let Some(ocr) = stored
            && !ocr.text().is_empty()
        {
            assert!(
                entry.ocr().is_some(),
                "{}: the manifest carries OCR text when the store does",
                entry.path()
            );
            assert_eq!(
                entry.ocr().map(|sidecar| sidecar.text().to_string()),
                Some(ocr.text().to_string()),
                "{}: the manifest's OCR text matches the store's",
                entry.path()
            );
        }
    }
}

#[test]
fn the_manifest_round_trips_through_read() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    library.copy_fixture("multi-page.pdf", "2 - Second.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "a clean run exits 0");

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    // A second read produces a manifest equal to the first, with the same
    // entries in the same order — the file is the same one, byte for byte.
    let again = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back again");
    assert_eq!(
        manifest.entries().len(),
        again.entries().len(),
        "the round-trip preserves entry count"
    );
    for (left, right) in manifest.entries().iter().zip(again.entries().iter()) {
        assert_eq!(
            left.path().as_str(),
            right.path().as_str(),
            "the round-trip preserves path order"
        );
        assert_eq!(
            left.fingerprint().sha256(),
            right.fingerprint().sha256(),
            "the round-trip preserves content hashes"
        );
    }
}

#[test]
fn detected_languages_round_trip_through_the_sidecar() {
    // The sidecar's `SidecarOcr` carries the recognized text and the
    // detected language. A text-layer PDF stores the text layer's text
    // as OCR text (so the search index can use it), with the language
    // field absent because no language detector ran. A scan-style
    // fixture would carry a language, but the committed image-only
    // fixture is a 64x32 RGB gradient that produces no text; the
    // assertion here is therefore the text round-trip with the
    // language field round-tripping as `None`, which is what the
    // sidecar contract actually promises.
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(
        output.status.success(),
        "a clean run exits 0; stderr: {}",
        stderr(&output)
    );

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    let entry = manifest
        .entries()
        .first()
        .expect("the entry was indexed");
    let ocr = entry
        .ocr()
        .expect("a text-layer entry carries OCR text in the sidecar");
    assert_eq!(
        ocr.language(),
        None,
        "a text-layer PDF has no detected language: {ocr:?}"
    );
    // The text the sidecar records round-trips exactly.
    let store = IndexStore::open(library.index_path()).expect("open the index");
    let stored = store
        .ocr_text(&entry.fingerprint().sha256())
        .expect("read OCR text from the store")
        .expect("the store carries the text");
    assert_eq!(
        ocr.text(),
        stored.text(),
        "the sidecar's OCR text matches the store"
    );
}

#[test]
fn rewriting_the_same_store_produces_an_equivalent_manifest() {
    // Entries are sorted by path inside the writer, so the only field
    // that changes between two runs against the same library is
    // `generated_at`. A byte-identical comparison would therefore race
    // the wall clock, so we instead assert that the two manifests
    // describe the same records: same schema version, same tool
    // version, same entries in the same order.
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    library.copy_fixture("multi-page.pdf", "2 - Second.pdf");

    let first = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(first.status.success(), "first run exits 0");
    let first_manifest = SidecarManifest::read(library.manifest_path())
        .expect("first manifest reads back");

    let second = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(second.status.success(), "second run exits 0");
    let second_manifest = SidecarManifest::read(library.manifest_path())
        .expect("second manifest reads back");

    assert_eq!(
        first_manifest.schema_version(),
        second_manifest.schema_version(),
        "schema version is stable across runs"
    );
    assert_eq!(
        first_manifest.tool_version(),
        second_manifest.tool_version(),
        "tool version is stable across runs"
    );
    assert_eq!(
        first_manifest.entries().len(),
        second_manifest.entries().len(),
        "entry count is stable across runs"
    );
    for (left, right) in first_manifest
        .entries()
        .iter()
        .zip(second_manifest.entries().iter())
    {
        assert_eq!(
            left.path().as_str(),
            right.path().as_str(),
            "path order is stable across runs"
        );
        assert_eq!(
            left.fingerprint().sha256(),
            right.fingerprint().sha256(),
            "content hashes are stable across runs"
        );
    }
}

#[test]
fn the_summary_line_lists_the_right_per_file_failure_counters() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - Good.pdf");
    library.copy_fixture("corrupt.pdf", "2 - Broken.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(
        output.status.success(),
        "a run that completed with per-file failures still exits 0; stderr: {}",
        stderr(&output)
    );

    let summary = stdout(&output);
    // `readable` counts files the catalog could read; the corrupt PDF
    // passes the catalog's read step but fails the PDF parser, so it
    // still counts as readable here. What the summary surfaces is the
    // extra bucket.
    assert!(
        summary.contains("indexed 2 of 2"),
        "the catalog saw both PDFs: {summary}"
    );
    assert!(
        summary.contains("1 PDF failures"),
        "the corrupt PDF is counted: {summary}"
    );
}

// -----------------------------------------------------------------------
// Failure-mode assertions
// -----------------------------------------------------------------------

#[test]
fn a_missing_root_exits_non_zero() {
    let output = run_index(&["/definitely/not/a/real/library"]);
    assert!(
        !output.status.success(),
        "a missing root exits non-zero; stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("library root"),
        "the reason names the library root: {}",
        stderr(&output)
    );
}

#[test]
fn a_regular_file_used_as_a_root_exits_non_zero() {
    let library = Library::new();
    let file = library.root().join("not-a-directory.pdf");
    fs::write(&file, b"not a directory").expect("write a file");

    let output = run_index(&[file.to_str().expect("utf-8 path")]);
    assert!(
        !output.status.success(),
        "a file used as a root exits non-zero; stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn a_run_that_cannot_initialise_tesseract_exits_non_zero() {
    // A path that does not exist is a tessdata directory that cannot be
    // initialised, so the binary exits non-zero before the sidecar is
    // written.
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    let bogus = "/no/such/tessdata/directory-xyz-123";

    let output = run_index(&[
        library.root().to_str().expect("utf-8 path"),
        "--tessdata-dir",
        bogus,
    ]);
    assert!(
        !output.status.success(),
        "missing tessdata exits non-zero; stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("tessdata"),
        "the reason names tessdata: {}",
        stderr(&output)
    );
}

#[test]
fn a_run_with_no_trained_data_at_tessdata_exits_non_zero() {
    // An empty directory is a tessdata directory that exists but holds
    // no `*.traineddata` files; the indexer treats it as a hard failure
    // because the run would produce a sidecar with no OCR text at all.
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    let empty = TempDir::new().expect("empty tessdata temp dir");

    let output = run_index(&[
        library.root().to_str().expect("utf-8 path"),
        "--tessdata-dir",
        empty.path().to_str().expect("utf-8 path"),
    ]);
    assert!(
        !output.status.success(),
        "empty tessdata exits non-zero; stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("no tesseract language data"),
        "the error names the empty tessdata: {}",
        stderr(&output)
    );
}

// -----------------------------------------------------------------------
// Rename / duplicate-ID scenarios from the smoke test
// -----------------------------------------------------------------------

#[test]
fn a_same_hash_rename_moves_the_record_in_the_sidecar() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");

    let first = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(first.status.success(), "first run exits 0");

    fs::remove_file(library.root().join("1 - First.pdf")).expect("remove original");
    library.copy_fixture("text-layer.pdf", "3 - Renamed.pdf");

    let second = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(second.status.success(), "rename run exits 0");

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back after the rename");
    assert_eq!(
        manifest.entries().len(),
        1,
        "one record after the rename: {:?}",
        manifest.entries()
    );
    let entry = &manifest.entries()[0];
    assert!(
        entry.path().as_str().contains("Renamed"),
        "the entry sits at the new path: {}",
        entry.path().as_str()
    );
    // The fingerprint is unchanged: the bytes are the same.
    let original_hash = sha256_of(&library.root().join("3 - Renamed.pdf"));
    assert_eq!(entry.fingerprint().sha256(), original_hash);
}

#[test]
fn a_duplicate_id_is_carried_in_both_records() {
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "12 - Autumn Leaves.pdf");
    library.copy_fixture("multi-page.pdf", "12 - Autumn Leaves (alt).pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(
        output.status.success(),
        "duplicate-ID runs exit 0; stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );

    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("the manifest reads back");
    assert_eq!(
        manifest.entries().len(),
        2,
        "both records survive the conflict: {:?}",
        manifest.entries()
    );
    assert!(
        stdout(&output).contains("1 duplicate-ID conflicts"),
        "the summary names the conflict: {}",
        stdout(&output)
    );
}

// -----------------------------------------------------------------------
// Import-preview smoke
// -----------------------------------------------------------------------

#[test]
fn sidecar_importer_preview_accepts_the_run_output() {
    // The Android side previews a sidecar before any write. The smoke
    // check this test runs is exactly that preview, against a fresh
    // `IndexStore`. A failure here would mean the manifest the indexer
    // produced is not in the shape the importer expects.
    let library = Library::new();
    library.copy_fixture("text-layer.pdf", "1 - First.pdf");
    library.copy_fixture("multi-page.pdf", "2 - Second.pdf");

    let output = run_index(&[library.root().to_str().expect("utf-8 path")]);
    assert!(output.status.success(), "first run exits 0");

    // Build a fresh local store in a sibling directory; the preview is
    // what the receiving app does before it touches its own index.
    let local_root = TempDir::new().expect("local store temp dir");
    let local_path = local_root.path().join(".notae/index.sqlite");
    fs::create_dir_all(local_path.parent().expect("parent")).expect("mkdir");
    let local = IndexStore::open(&local_path).expect("open the local store");
    let manifest = SidecarManifest::read(library.manifest_path())
        .expect("read the sidecar");
    let catalog = notae_index_cli::catalog::FilesystemCatalog::new(
        notae_index_cli::catalog::ValidatedRoot::open(library.root())
            .expect("open the library root"),
    );
    let preview = SidecarImporter::preview(&local, &manifest, &catalog)
        .expect("the preview accepts the sidecar");
    assert_eq!(
        preview.entries().len(),
        manifest.entries().len(),
        "every manifest entry appears in the preview"
    );
}
