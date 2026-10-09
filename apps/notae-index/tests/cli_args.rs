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

// --- Root boundary: names the domain rejects, symlinks, and root failures ---

#[test]
fn records_a_drive_prefix_filename_without_aborting_the_batch() {
    let library = Library::new();
    library.write("C:.pdf", "drive");
    library.write("good.pdf", KNOWN_CONTENT);

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    let files = catalog
        .enumerate()
        .expect("a rejected name is a per-file failure, not a panic");

    assert_eq!(
        files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>(),
        vec!["good.pdf"],
        "only representable paths are enumerated"
    );
    assert!(
        files.iter().all(|f| f.is_readable()),
        "the rest of the batch was still fingerprinted"
    );

    let failures = catalog.failures();
    assert_eq!(failures.len(), 1, "the rejected name is reported: {failures:?}");
    assert_eq!(failures[0].path(), "C:.pdf");
    assert!(
        failures[0].error().contains("C:.pdf"),
        "the failure explains the rejected path: {}",
        failures[0].error()
    );
}

#[test]
fn a_rejected_name_in_a_subdirectory_is_reported_with_its_relative_path() {
    // A drive prefix is only a drive prefix at index 0, so `nested/C:.pdf` is
    // a legitimate POSIX path; a backslash is the separator the portable sidecar
    // cannot carry, wherever it appears.
    let library = Library::new();
    library.write("nested/C:.pdf", "fine");
    library.write("nested/back\\slash.pdf", "rejected");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    let files = catalog.enumerate().expect("enumerate");

    assert_eq!(
        files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>(),
        vec!["nested/C:.pdf"],
        "only the representable path is enumerated"
    );
    let failures = catalog.failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].path(), "nested/back\\slash.pdf");
}

#[cfg(unix)]
#[test]
fn never_reads_or_reports_a_symlink_pointing_outside_the_root() {
    let library = Library::new();
    library.write("inside.pdf", KNOWN_CONTENT);
    let outside = library._dir.path().join("outside-target.pdf");
    fs::write(&outside, "outside content").expect("write the target outside the root");
    std::os::unix::fs::symlink(&outside, library.root.join("outside-link.pdf"))
        .expect("link outside the root");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    let files = catalog.enumerate().expect("enumerate");

    let found = files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>();
    assert_eq!(found, vec!["inside.pdf"], "the link is not enumerated: {found:?}");
    assert!(
        !files
            .iter()
            .any(|f| f.path().as_str() == "outside-link.pdf"
                && f.fingerprint().is_some_and(|fp| fp.byte_len() == "outside content".len() as u64)),
        "no fingerprint of the external target exists"
    );

    let failures = catalog.failures();
    assert!(
        failures.iter().any(|f| f.path() == "outside-link.pdf"),
        "the skipped link is reported: {failures:?}"
    );
}

#[cfg(unix)]
#[test]
fn never_reads_a_symlink_that_points_outside_through_a_subdirectory() {
    let library = Library::new();
    let outside_dir = library._dir.path().join("outside-dir");
    fs::create_dir_all(&outside_dir).expect("create the directory outside the root");
    fs::write(outside_dir.join("target.pdf"), "outside content").expect("write the target");
    std::os::unix::fs::symlink(&outside_dir, library.root.join("linkdir"))
        .expect("link the directory");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    let files = catalog.enumerate().expect("enumerate");

    assert!(files.is_empty(), "a linked directory is not walked: {files:?}");
}

#[cfg(unix)]
#[test]
fn a_symlink_loop_terminates_without_reading_the_target() {
    let library = Library::new();
    library.write("real.pdf", KNOWN_CONTENT);
    std::os::unix::fs::symlink("loop.pdf", library.root.join("loop.pdf"))
        .expect("create a self-referencing link");
    std::os::unix::fs::symlink(&library.root, library.root.join("selfdir"))
        .expect("create a self-referencing directory link");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    let files = catalog.enumerate().expect("a loop does not hang or fail the run");

    let found = files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>();
    assert_eq!(found, vec!["real.pdf"], "only the real file is enumerated: {found:?}");
}

#[cfg(unix)]
#[test]
fn a_symlink_inside_the_root_is_skipped_rather_than_followed() {
    let library = Library::new();
    library.write("target.pdf", KNOWN_CONTENT);
    std::os::unix::fs::symlink("target.pdf", library.root.join("alias.pdf"))
        .expect("link inside the root");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");
    let files = catalog.enumerate().expect("enumerate");

    let found = files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>();
    assert_eq!(found, vec!["target.pdf"], "only the real file is enumerated: {found:?}");
}

#[cfg(unix)]
#[test]
fn an_unreadable_root_fails_the_run_and_names_the_root() {
    use std::os::unix::fs::PermissionsExt;

    let library = Library::new();
    library.write("hidden.pdf", KNOWN_CONTENT);
    fs::set_permissions(&library.root, fs::Permissions::from_mode(0o000))
        .expect("lock the root");
    if fs::read_dir(&library.root).is_ok() {
        // Running as a user that bypasses permissions (root).
        fs::set_permissions(&library.root, fs::Permissions::from_mode(0o755)).expect("unlock");
        return;
    }

    // The root is the boundary of the run: a tree whose top directory cannot
    // be read yields no trustworthy report at all, so it is a command failure
    // rather than an isolated per-file skip.
    let error = FilesystemCatalog::new(library.root.clone())
        .expect_err("a root that cannot be read is not a catalog");
    fs::set_permissions(&library.root, fs::Permissions::from_mode(0o755)).expect("unlock");
    assert!(
        error.to_string().contains(&library.root.display().to_string()),
        "the error names the root: {error}"
    );

    let library = Library::new();
    library.write("hidden.pdf", KNOWN_CONTENT);
    fs::set_permissions(&library.root, fs::Permissions::from_mode(0o000))
        .expect("lock the root");
    let cli = parse(&[library.root.to_str().expect("utf-8 path")]).expect("arguments parse");
    let run_error = notae_index_cli::run(cli).expect_err("an unreadable root cannot be reported on");
    fs::set_permissions(&library.root, fs::Permissions::from_mode(0o755)).expect("unlock");
    assert!(
        run_error.to_string().contains(&library.root.display().to_string()),
        "the run error names the root: {run_error}"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_root_is_not_mistaken_for_an_isolated_file_failure() {
    use std::os::unix::fs::PermissionsExt;

    let library = Library::new();
    library.write("hidden.pdf", KNOWN_CONTENT);
    let nested = library.root.join("nested");
    fs::create_dir_all(&nested).expect("create a subdirectory");
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o000))
        .expect("lock the subdirectory");
    if fs::read_dir(&nested).is_ok() {
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o755)).expect("unlock");
        return;
    }

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("the root itself is readable");
    let files = catalog.enumerate().expect("a locked subdirectory is an isolated failure");
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o755)).expect("unlock");

    assert_eq!(
        files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>(),
        vec!["hidden.pdf"],
        "the readable part of the tree is still enumerated"
    );
    let failures = catalog.failures();
    assert_eq!(failures.len(), 1, "the locked subdirectory is reported: {failures:?}");
    assert_eq!(
        failures[0].path(),
        "nested",
        "the failure names the subdirectory it concerns"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_root_is_refused_rather_than_followed() {
    let library = Library::new();
    let outside = library._dir.path().join("outside-library");
    fs::create_dir_all(&outside).expect("create the directory outside the root");
    fs::write(outside.join("secret.pdf"), "outside content").expect("write the target");
    fs::write(library.root.join("inside.pdf"), KNOWN_CONTENT).expect("write inside the root");
    let link = library._dir.path().join("library-link");
    std::os::unix::fs::symlink(&outside, &link).expect("link the directory as the root");

    // The root is the one name a user may legitimately hand the tool, so a
    // link there would silently retarget the whole run outside the tree the
    // user believes they selected. The link is refused by name instead.
    let error = FilesystemCatalog::new(link.clone())
        .expect_err("a symlinked root is not a catalog");
    assert!(
        error.to_string().contains("symbolic link"),
        "the refusal says the root is a link: {error}"
    );
    assert!(
        error.to_string().contains(&link.display().to_string()),
        "the refusal names the root as given: {error}"
    );

    let cli = parse(&[link.to_str().expect("utf-8 path")]).expect("arguments parse");
    let validate_error = cli
        .validate()
        .expect_err("argument validation refuses the link too");
    assert!(
        validate_error.to_string().contains("symbolic link"),
        "validation says the same thing: {validate_error}"
    );
}

#[cfg(unix)]
#[test]
fn a_root_retargeted_by_a_symbolic_link_after_validation_is_refused() {
    let library = Library::new();
    library.write("inside.pdf", KNOWN_CONTENT);
    // The catalog is opened over a real directory, which is the moment the
    // root is validated and held.
    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");

    // The name the run was given is moved aside and replaced by a link to a
    // tree the user never selected. Enumeration must not follow it.
    let moved = library._dir.path().join("moved-library");
    fs::rename(&library.root, &moved).expect("move the validated root aside");
    let outside = library._dir.path().join("outside-library");
    fs::create_dir_all(&outside).expect("create the directory outside the root");
    fs::write(outside.join("secret.pdf"), "outside content").expect("write the target");
    std::os::unix::fs::symlink(&outside, &library.root).expect("retarget the root at a link");

    let error = catalog
        .enumerate()
        .expect_err("a retargeted root must not be enumerated");
    let message = error.to_string();
    assert!(
        message.contains(&library.root.display().to_string()),
        "the failure names the root as given: {message}"
    );
    assert!(
        !message.contains("secret.pdf"),
        "nothing behind the link is reported: {message}"
    );
}

#[cfg(unix)]
#[test]
fn a_root_replaced_by_another_directory_after_validation_is_refused() {
    let library = Library::new();
    library.write("inside.pdf", KNOWN_CONTENT);
    let catalog = FilesystemCatalog::new(library.root.clone()).expect("catalog");

    // The same name is taken over by a different real directory: no symbolic
    // link is involved, so only the identity the catalog retained can catch it.
    let moved = library._dir.path().join("moved-library");
    fs::rename(&library.root, &moved).expect("move the validated root aside");
    fs::create_dir_all(&library.root).expect("create a different directory at the root name");
    fs::write(library.root.join("other.pdf"), "other content").expect("write into it");

    let error = catalog
        .enumerate()
        .expect_err("a root that changed identity must not be enumerated");
    let message = error.to_string();
    assert!(
        message.contains(&library.root.display().to_string()),
        "the failure names the root as given: {message}"
    );
    assert!(
        catalog.failures().is_empty(),
        "nothing from either tree is reported as a per-file failure: {:?}",
        catalog.failures()
    );
}

#[cfg(unix)]
#[test]
fn a_link_inside_the_root_still_degrades_to_a_reported_skip() {
    let library = Library::new();
    let outside = library._dir.path().join("outside-library");
    fs::create_dir_all(&outside).expect("create the directory outside the root");
    fs::write(outside.join("secret.pdf"), "outside content").expect("write the target");
    library.write("inside.pdf", KNOWN_CONTENT);
    std::os::unix::fs::symlink(&outside, library.root.join("linkdir"))
        .expect("link a directory inside the root");

    let catalog = FilesystemCatalog::new(library.root.clone()).expect("the root is a real directory");
    let files = catalog.enumerate().expect("a linked entry is an isolated failure");

    assert_eq!(
        files.iter().map(|f| f.path().as_str()).collect::<Vec<_>>(),
        vec!["inside.pdf"],
        "the rest of the tree is still indexed"
    );
    assert!(
        catalog.failures().iter().any(|f| f.path() == "linkdir"),
        "the skipped link is reported: {:?}",
        catalog.failures()
    );
}
