//! Argument parsing and filesystem catalog behaviour.
//!
//! These tests exercise the CLI's own crates through its library surface, so
//! they can construct a catalog directly; the binary itself is only a thin
//! wrapper around [`notae_index_cli::run`].

use std::fs;
use std::path::{Path, PathBuf};

use notae_index::PdfCatalog;
use notae_index_cli::args::{Cli, CliError};
use notae_index_cli::catalog::FilesystemCatalog;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// A short ASCII body whose SHA-256 is fixed, so the fingerprint a catalog
/// computes can be compared against a digest derived independently of it.
const KNOWN_CONTENT: &str = "notae catalog fixture";
const KNOWN_SHA256: &str = "39eb0138637c4899847d59a59d52bb69c8cc805da157be4e97d4ecf2820e4c2a";

struct Library {
    _dir: TempDir,
    root: PathBuf,
}

impl Library {
    fn new() -> Self {
        let dir = TempDir::new().expect("temporary parent directory");
        // The root is a child, so a sibling file really is outside the library.
        let root = dir.path().join("library");
        fs::create_dir_all(&root).expect("create the library root");
        Self { _dir: dir, root }
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(&path, content).expect("write fixture");
        path
    }
}

fn paths(catalog: &FilesystemCatalog) -> Vec<String> {
    catalog
        .enumerate()
        .expect("enumerate the library")
        .iter()
        .map(|file| file.path().as_str().to_owned())
        .collect()
}

fn parse(args: &[&str]) -> Result<Cli, CliError> {
    let mut full = vec!["notae-index"];
    full.extend_from_slice(args);
    Cli::try_parse_from(full)
}

#[test]
fn root_argument_is_required() {
    let error = Cli::try_parse_from(["notae-index"]).expect_err("a missing root is rejected");
    assert!(
        error.to_string().contains("LIBRARY_ROOT"),
        "error names the missing argument: {error}"
    );
}

#[test]
fn parses_root_and_verbose_flag() {
    let cli = parse(&["/library", "--verbose"]).expect("valid arguments");
    assert_eq!(cli.library_root(), Path::new("/library"));
    assert!(cli.is_verbose());
}

#[test]
fn verbose_defaults_to_off() {
    let cli = parse(&["/library"]).expect("valid arguments");
    assert!(!cli.is_verbose());
}

#[test]
fn rejects_nonexistent_root() {
    let missing = Path::new("/definitely/not/a/real/library");
    let cli = parse(&[missing.to_str().expect("utf-8 path")]).expect("arguments parse");
    let error = cli.validate().expect_err("a nonexistent root is rejected");
    assert!(
        matches!(&error, CliError::RootUnreadable(path) if path == missing),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_a_file_as_root() {
    let library = Library::new();
    let file = library.write("notes.pdf", KNOWN_CONTENT);
    let cli = parse(&[file.to_str().expect("utf-8 path")]).expect("arguments parse");
    assert!(cli.validate().is_err(), "a file is not a library root");
}

#[test]
fn accepts_an_empty_existing_root() {
    let library = Library::new();
    let cli = parse(&[library.root.to_str().expect("utf-8 path")]).expect("arguments parse");
    cli.validate()
        .expect("an existing empty directory is a library root");
}

#[test]
fn new_rejects_a_root_it_cannot_read() {
    let error = FilesystemCatalog::new(PathBuf::from("/definitely/not/a/real/library"))
        .expect_err("a nonexistent root is not a catalog");
    assert!(error.to_string().contains("not readable"), "{error}");
}

#[test]
fn discovers_pdfs_recursively_in_both_cases() {
    let library = Library::new();
    library.write("top.pdf", "a");
    library.write("upper.PDF", "b");
    library.write("nested/inner.pdf", "c");
    library.write("nested/deeper/leaf.PDF", "d");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    assert_eq!(
        paths(&catalog),
        vec![
            "nested/deeper/leaf.PDF",
            "nested/inner.pdf",
            "top.pdf",
            "upper.PDF"
        ]
    );
}

#[test]
fn ignores_non_pdf_files() {
    let library = Library::new();
    library.write("notes.pdf", "a");
    library.write("notes.txt", "b");
    library.write("archive.pdf.zip", "c");
    library.write("sidecar.json", "d");
    library.write("noextension", "e");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    assert_eq!(paths(&catalog), vec!["notes.pdf"]);
}

#[test]
fn produces_root_relative_forward_slash_paths() {
    let library = Library::new();
    library.write("a/b/c/deep.pdf", "x");

    let files = FilesystemCatalog::new(library.root.clone())
        .expect("catalog")
        .enumerate()
        .expect("enumerate");

    let path = files[0].path().as_str();
    assert_eq!(path, "a/b/c/deep.pdf");
    assert!(!path.starts_with('/'), "path is relative: {path}");
    assert!(!path.contains('\\'), "path uses forward slashes: {path}");
}

#[test]
fn never_produces_a_file_outside_the_root() {
    let library = Library::new();
    library.write("inside.pdf", "a");
    let outside = library._dir.path().join("outside.pdf");
    fs::write(&outside, "b").expect("write the file beside the root");

    let found = paths(&FilesystemCatalog::new(library.root.clone()).expect("catalog"));
    assert_eq!(found, vec!["inside.pdf"]);
    assert!(
        found
            .iter()
            .all(|path| !path.contains("..") && !path.starts_with('/')),
        "no escaping path: {found:?}"
    );
}

#[cfg(unix)]
#[test]
fn records_an_unreadable_file_without_aborting_enumeration() {
    use std::os::unix::fs::PermissionsExt;

    let library = Library::new();
    library.write("good.pdf", "a");
    let locked = library.write("locked.pdf", "b");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("lock the file");
    if fs::File::open(&locked).is_ok() {
        // Running as a user that bypasses permissions (root); the check below
        // would be meaningless.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).expect("unlock the file");
        return;
    }

    let files = FilesystemCatalog::new(library.root.clone())
        .expect("catalog")
        .enumerate()
        .expect("an unreadable file does not fail the enumeration");

    let locked_entry = files
        .iter()
        .find(|file| file.path().as_str() == "locked.pdf")
        .expect("the locked file is still enumerated");
    assert!(
        !locked_entry.is_readable(),
        "the locked file is not readable"
    );
    assert!(
        locked_entry.fingerprint().is_none(),
        "no fingerprint exists"
    );
    assert!(
        files
            .iter()
            .any(|file| file.path().as_str() == "good.pdf" && file.is_readable()),
        "the readable sibling was still fingerprinted"
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).expect("unlock the file");
}

#[test]
fn records_an_unreadable_directory_without_aborting_enumeration() {
    use std::os::unix::fs::PermissionsExt;

    let library = Library::new();
    library.write("good.pdf", "a");
    let locked_dir = library.root.join("locked");
    fs::create_dir_all(&locked_dir).expect("create the directory");
    fs::write(locked_dir.join("hidden.pdf"), "b").expect("write inside it");
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o000))
        .expect("lock the directory");

    let result = FilesystemCatalog::new(library.root.clone())
        .expect("catalog")
        .enumerate();

    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o755)).expect("unlock it");

    let files = result.expect("an unreadable directory does not fail the enumeration");
    assert_eq!(files.len(), 1, "only the readable file is enumerated");
    assert_eq!(files[0].path().as_str(), "good.pdf");
}

#[test]
fn ordering_is_stable_across_runs() {
    let library = Library::new();
    for name in ["m.pdf", "a.pdf", "z.pdf", "b/nested.pdf", "b/a.pdf"] {
        library.write(name, name);
    }
    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");

    let first = paths(&catalog);
    let second = paths(&catalog);
    assert_eq!(first, second, "repeated enumeration is stable");
    assert_eq!(
        first,
        vec!["a.pdf", "b/a.pdf", "b/nested.pdf", "m.pdf", "z.pdf"],
        "results are sorted by relative path"
    );
}

#[test]
fn computes_a_real_sha256_fingerprint() {
    let library = Library::new();
    let path = library.write("known.pdf", KNOWN_CONTENT);
    let byte_len = fs::metadata(&path).expect("metadata").len();

    let files = FilesystemCatalog::new(library.root.clone())
        .expect("catalog")
        .enumerate()
        .expect("enumerate");
    let fingerprint = files[0].fingerprint().expect("the fixture is readable");

    assert_eq!(
        fingerprint.sha256().to_string(),
        KNOWN_SHA256,
        "the digest is the SHA-256 of the file contents"
    );
    assert_eq!(fingerprint.byte_len(), byte_len);
    assert!(
        fingerprint.modified().is_some(),
        "a filesystem modification time is observed"
    );
    assert_eq!(files[0].filename(), "known.pdf");
}

#[test]
fn run_enumerates_the_library_and_reports_it() {
    let library = Library::new();
    library.write("2 - Second.pdf", KNOWN_CONTENT);
    library.write("nested/10 - Tenth.pdf", "tenth");
    library.write("notes.txt", "ignored");

    let cli = parse(&[library.root.to_str().expect("utf-8 path")]).expect("arguments parse");
    let report = notae_index_cli::run(cli).expect("the run succeeds");

    assert_eq!(report.library_root(), library.root);
    assert_eq!(
        report.paths(),
        vec!["2 - Second.pdf", "nested/10 - Tenth.pdf"]
    );
    assert_eq!(report.enumerated(), 2);
    assert_eq!(report.readable(), 2);
    assert_eq!(report.unreadable(), 0);
    assert!(
        report.failures().is_empty(),
        "no failures: {:?}",
        report.failures()
    );
}

#[test]
fn run_reports_a_missing_root() {
    let cli = parse(&["/definitely/not/a/real/library"]).expect("arguments parse");
    let error = notae_index_cli::run(cli).expect_err("the run fails on a missing root");
    assert!(matches!(error, CliError::RootUnreadable(_)), "{error}");
}

#[test]
fn verbose_flag_does_not_change_the_report() {
    let library = Library::new();
    library.write("a.pdf", KNOWN_CONTENT);

    let quiet = notae_index_cli::run(parse(&[library.root.to_str().expect("utf-8 path")]).unwrap())
        .expect("quiet run");
    let loud = notae_index_cli::run(
        parse(&[library.root.to_str().expect("utf-8 path"), "--verbose"]).unwrap(),
    )
    .expect("verbose run");

    assert_eq!(quiet.paths(), loud.paths());
    assert_eq!(quiet.enumerated(), loud.enumerated());
}

#[test]
fn fingerprints_a_file_larger_than_one_read_buffer() {
    // Guards against a catalog that hashes a buffer it did not fully read.
    let library = Library::new();
    let big = "x".repeat(300_000);
    library.write("big.pdf", &big);

    let mut hasher = Sha256::new();
    hasher.update(big.as_bytes());
    let expected = hasher.finalize();
    let expected = notae_domain::Sha256Digest::from_bytes(expected.into());

    let files = FilesystemCatalog::new(library.root.clone())
        .expect("catalog")
        .enumerate()
        .expect("enumerate");
    let fingerprint = files[0].fingerprint().expect("readable");
    assert_eq!(fingerprint.byte_len(), big.len() as u64);
    assert_eq!(fingerprint.sha256(), expected);
}

#[test]
fn rereads_rewritten_content_rather_than_caching() {
    let library = Library::new();
    library.write("a.pdf", "one");
    library.write("a.pdf", "three");

    let files = FilesystemCatalog::new(library.root.clone())
        .expect("catalog")
        .enumerate()
        .expect("enumerate");
    assert_eq!(files[0].fingerprint().expect("readable").byte_len(), 5);
}
