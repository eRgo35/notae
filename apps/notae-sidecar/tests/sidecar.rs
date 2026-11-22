//! Tests of the sidecar tool over a fixture library and a fake renderer.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use notae_domain::SidecarFile;
use notae_sidecar::{Renderer, read_index, run};
use sha2::{Digest, Sha256};

struct FakeRenderer {
    calls: RefCell<Vec<PathBuf>>,
    fail: bool,
}

impl FakeRenderer {
    fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            fail: false,
        }
    }
}

impl Renderer for FakeRenderer {
    fn render(&self, pdf: &Path, out_png: &Path) -> Result<(), String> {
        self.calls.borrow_mut().push(pdf.to_path_buf());
        if self.fail {
            return Err("boom".to_owned());
        }
        fs::write(out_png, b"png").map_err(|e| e.to_string())
    }
}

fn fixture(dir: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/library");
    for entry in fs::read_dir(src).expect("read fixture") {
        let entry = entry.expect("entry");
        fs::copy(entry.path(), dir.join(entry.file_name())).expect("copy");
    }
}

fn load(dir: &Path) -> SidecarFile {
    let json = fs::read_to_string(dir.join(".notae/library.json")).expect("library.json");
    serde_json::from_str(&json).expect("parses as SidecarFile")
}

#[test]
fn writes_library_json_from_pdfs_and_csv() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    let report = run(dir.path(), &FakeRenderer::new()).expect("run");
    assert!(report.errors.is_empty() && report.warnings.is_empty());

    let file = load(dir.path());
    assert_eq!(file.version, 1);
    assert!(file.generator.starts_with("notae-sidecar "));
    let paths: Vec<_> = file.scores.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(paths, ["001 - Adeste Fideles.pdf", "Plain Title.pdf"]);

    let bytes = fs::read(dir.path().join("001 - Adeste Fideles.pdf")).expect("pdf");
    let e = &file.scores[0];
    assert_eq!(e.size, bytes.len() as u64);
    assert_eq!(
        e.sha256,
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    assert_eq!(e.number, Some(1));
    assert_eq!(e.title.as_deref(), Some("Adeste Fideles"));
    assert_eq!(e.composer.as_deref(), Some("Anon"));
    assert_eq!(e.tags, ["Boże Narodzenie", "Komunia"]);
    assert!(
        dir.path()
            .join(format!(".notae/thumbnails/{}.png", e.sha256))
            .is_file()
    );
}

#[test]
fn quoted_multiline_csv_and_bom_parse() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    let rows = read_index(&dir.path().join("Indeks.csv")).expect("read");
    let row = &rows["001 - Adeste Fideles.pdf"];
    assert_eq!(
        row.text.as_deref(),
        Some("Line one, with comma\nline \"two\"")
    );
    run(dir.path(), &FakeRenderer::new()).expect("run");
    assert_eq!(
        load(dir.path()).scores[0].text.as_deref(),
        Some("Line one, with comma\nline \"two\"")
    );
}

#[test]
fn empty_id_gives_null_number() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    run(dir.path(), &FakeRenderer::new()).expect("run");
    let e = &load(dir.path()).scores[1];
    assert_eq!(e.number, None);
    assert_eq!(e.composer, None);
    assert!(e.tags.is_empty());
    assert_eq!(e.text, None);
}

#[test]
fn pdf_without_row_is_a_warning() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    fs::write(dir.path().join("007 - Extra.pdf"), b"%PDF extra").expect("write");
    let report = run(dir.path(), &FakeRenderer::new()).expect("run");
    assert!(report.errors.is_empty());
    assert_eq!(report.warnings.len(), 1);
    let e = &load(dir.path()).scores[1];
    assert_eq!(e.path, "007 - Extra.pdf");
    assert_eq!(e.number, Some(7));
    assert_eq!(e.title.as_deref(), Some("Extra"));
    assert_eq!(e.composer, None);
}

#[test]
fn row_without_pdf_writes_nothing() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    fs::remove_file(dir.path().join("Plain Title.pdf")).expect("rm");
    let renderer = FakeRenderer::new();
    let report = run(dir.path(), &renderer).expect("run");
    assert_eq!(report.errors.len(), 1);
    assert!(!dir.path().join(".notae").exists());
    assert!(renderer.calls.borrow().is_empty());
}

#[test]
fn renderer_failure_writes_nothing() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    let renderer = FakeRenderer {
        fail: true,
        ..FakeRenderer::new()
    };
    let report = run(dir.path(), &renderer).expect("run");
    assert!(!report.errors.is_empty());
    assert!(!dir.path().join(".notae/library.json").exists());
}

#[test]
fn rerun_is_byte_identical_and_renders_only_missing() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    run(dir.path(), &FakeRenderer::new()).expect("first");
    let first = fs::read(dir.path().join(".notae/library.json")).expect("read");

    let renderer = FakeRenderer::new();
    run(dir.path(), &renderer).expect("second");
    assert_eq!(
        fs::read(dir.path().join(".notae/library.json")).expect("read"),
        first
    );
    assert!(renderer.calls.borrow().is_empty());

    let gone = load(dir.path()).scores[1].sha256.clone();
    fs::remove_file(dir.path().join("Plain Title.pdf")).expect("rm");
    let csv = fs::read_to_string(dir.path().join("Indeks.csv")).expect("csv");
    let kept: Vec<_> = csv.lines().take(3).collect(); // header + the two-line first row
    fs::write(dir.path().join("Indeks.csv"), kept.join("\n")).expect("csv");
    run(dir.path(), &FakeRenderer::new()).expect("third");
    assert!(
        !dir.path()
            .join(format!(".notae/thumbnails/{gone}.png"))
            .exists()
    );
    assert_eq!(load(dir.path()).scores.len(), 1);
}

#[test]
fn skips_notae_and_sources_and_leaves_old_files() {
    let dir = tempfile::tempdir().expect("tmp");
    fixture(dir.path());
    fs::create_dir_all(dir.path().join(".notae")).expect("mkdir");
    fs::create_dir_all(dir.path().join("sources")).expect("mkdir");
    fs::write(dir.path().join(".notae/old.pdf"), b"x").expect("w");
    fs::write(dir.path().join("sources/scan.pdf"), b"y").expect("w");
    fs::write(dir.path().join(".notae/index.sqlite"), b"sqlite").expect("w");
    let report = run(dir.path(), &FakeRenderer::new()).expect("run");
    assert!(report.warnings.is_empty());
    assert_eq!(load(dir.path()).scores.len(), 2);
    assert_eq!(
        fs::read(dir.path().join(".notae/index.sqlite")).expect("r"),
        b"sqlite"
    );
}
