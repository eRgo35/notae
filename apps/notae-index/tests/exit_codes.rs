//! The exit codes the built binary actually returns.
//!
//! These tests run `CARGO_BIN_EXE_notae-index` as a process, because the exit
//! code is the contract the plan cares about and a unit test of `main` cannot
//! observe it.
//!
//! Exit-code contract:
//!
//! * `0` — an informational request (`--help`, `--version`) and any run that
//!   produced a report, including one carrying isolated per-file failures.
//! * non-zero — the command line is invalid, or no report could be produced at
//!   all (missing/unusable root, a symlinked root, a catalog-wide failure).

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Library {
    _dir: TempDir,
    root: std::path::PathBuf,
}

impl Library {
    fn new() -> TestResult<Self> {
        let dir = TempDir::new()?;
        let root = dir.path().join("library");
        fs::create_dir_all(&root)?;
        Ok(Self { _dir: dir, root })
    }

    fn write(&self, relative: &str, content: &str) -> TestResult {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

fn index(args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_notae-index"))
        .args(args)
        .output()
}

fn utf8(path: &Path) -> TestResult<&str> {
    path.to_str()
        .ok_or_else(|| format!("{} is not UTF-8", path.display()).into())
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_exits_successfully() -> TestResult {
    let output = index(&["--help"])?;
    assert!(
        output.status.success(),
        "--help must exit 0; stderr: {}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("LIBRARY_ROOT"),
        "--help prints usage on stdout: {}",
        stdout(&output)
    );
    Ok(())
}

#[test]
fn version_exits_successfully() -> TestResult {
    let output = index(&["--version"])?;
    assert!(
        output.status.success(),
        "--version must exit 0; stderr: {}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).trim().is_empty(),
        "--version prints the version on stdout"
    );
    Ok(())
}

#[test]
fn a_missing_root_argument_is_a_usage_error() -> TestResult {
    let output = index(&[])?;
    assert!(
        !output.status.success(),
        "an invalid command line exits non-zero"
    );
    assert!(
        stderr(&output).contains("LIBRARY_ROOT"),
        "the usage error names the argument: {}",
        stderr(&output)
    );
    Ok(())
}

#[test]
fn an_unknown_flag_is_a_usage_error() -> TestResult {
    let output = index(&["/tmp", "--nonsense"])?;
    assert!(!output.status.success(), "an unknown flag exits non-zero");
    assert!(
        !stderr(&output).is_empty(),
        "the usage error is reported on stderr"
    );
    Ok(())
}

#[test]
fn an_unusable_root_exits_non_zero() -> TestResult {
    let output = index(&["/definitely/not/a/real/library"])?;
    assert!(
        !output.status.success(),
        "a run that cannot produce a report exits non-zero"
    );
    assert!(
        stderr(&output).contains("library root"),
        "the reason is reported: {}",
        stderr(&output)
    );
    Ok(())
}

#[test]
fn a_completed_run_with_isolated_failures_exits_zero_and_reports_them() -> TestResult {
    let library = Library::new()?;
    library.write("good.pdf", "readable")?;
    // A filename the domain type rejects: an isolated per-file failure.
    library.write("C:.pdf", "drive")?;

    let output = index(&[utf8(library.root())?, "--verbose"])?;
    assert!(
        output.status.success(),
        "a run that completes with per-file failures exits 0; stdout: {}",
        stdout(&output)
    );

    let reported = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        reported.contains("C:.pdf"),
        "the per-file failure is still reported: {reported}"
    );
    assert!(
        reported.contains("good.pdf"),
        "the readable file is reported: {reported}"
    );
    Ok(())
}

#[test]
fn a_clean_run_exits_zero() -> TestResult {
    let library = Library::new()?;
    library.write("good.pdf", "readable")?;

    let output = index(&[utf8(library.root())?])?;
    assert!(
        output.status.success(),
        "a clean run exits 0; stderr: {}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("indexed 1 of 1"),
        "the summary is printed: {}",
        stdout(&output)
    );
    Ok(())
}

/// Removes read permission from `path`, reporting whether the lock took effect.
///
/// A user that bypasses permissions — root — would see the directory as
/// readable whatever the mode says, and the caller skips its assertion rather
/// than pretending the run failed for the wrong reason.
fn lock_directory(path: &Path) -> TestResult<bool> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o000))?;
    if fs::read_dir(path).is_ok() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        return Ok(false);
    }
    Ok(true)
}

fn unlock_directory(path: &Path) -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
fn an_unreadable_root_exits_non_zero_and_names_it() -> TestResult {
    let library = Library::new()?;
    library.write("hidden.pdf", "content")?;
    if !lock_directory(library.root())? {
        return Ok(());
    }

    let output = index(&[utf8(library.root())?]);
    unlock_directory(library.root())?;
    let output = output?;

    assert!(
        !output.status.success(),
        "a run over a root it cannot read cannot be trusted, so it exits non-zero; \
         stdout: {}",
        stdout(&output)
    );
    assert!(
        !stdout(&output).contains("indexed"),
        "no report is claimed: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains(&library.root().display().to_string()),
        "the message names the root that could not be read: {}",
        stderr(&output)
    );
    Ok(())
}

#[test]
fn an_unreadable_file_under_a_readable_root_still_exits_zero() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let library = Library::new()?;
    library.write("readable.pdf", "content")?;
    library.write("nested/locked.pdf", "content")?;
    let locked = library.root().join("nested/locked.pdf");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))?;
    if fs::File::open(&locked).is_ok() {
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o644))?;
        return Ok(());
    }

    let output = index(&[utf8(library.root())?]);
    let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o644));
    let output = output?;

    assert!(
        output.status.success(),
        "an isolated per-file failure does not fail the run; stderr: {}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("indexed 1 of 2"),
        "the summary counts the unreadable file: {}",
        stdout(&output)
    );
    let reported = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        reported.contains("nested/locked.pdf"),
        "the unreadable file is still named: {reported}"
    );
    Ok(())
}

#[test]
fn a_symlinked_root_never_indexes_the_directory_it_points_at() -> TestResult {
    let library = Library::new()?;
    let outside = library._dir.path().join("outside-library");
    fs::create_dir_all(&outside)?;
    fs::write(outside.join("secret.pdf"), "outside content")?;
    let link = library._dir.path().join("library-link");
    std::os::unix::fs::symlink(&outside, &link)?;

    let output = index(&[utf8(&link)?])?;

    let reported = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        !reported.contains("secret.pdf") && !stdout(&output).contains("indexed"),
        "nothing behind the link is indexed or reported: {reported}"
    );
    assert!(
        !output.status.success(),
        "a symlinked root is refused rather than followed, so it exits non-zero; \
         stdout: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("symbolic link"),
        "the refusal says why: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains(&link.display().to_string()),
        "the refusal names the root as given: {}",
        stderr(&output)
    );
    Ok(())
}

/// A link is a link however the command line spells it.
///
/// Linux resolves a trailing separator — and a trailing `.`, and a doubled
/// separator — before it stats anything, so the binary is the thing that has to
/// refuse these: a run that accepted `link/` would index the link's target and
/// exit zero, which is exactly the outcome the root boundary forbids.
#[test]
fn no_spelling_of_a_symlinked_root_is_followed_by_the_binary() -> TestResult {
    let library = Library::new()?;
    let outside = library._dir.path().join("outside-library");
    fs::create_dir_all(&outside)?;
    fs::write(outside.join("secret.pdf"), "outside content")?;
    std::os::unix::fs::symlink(&outside, library._dir.path().join("library-link"))?;
    std::os::unix::fs::symlink(
        library._dir.path().join("nothing-here"),
        library._dir.path().join("dead-link"),
    )?;

    for spelling in [
        "library-link",
        "library-link/",
        "library-link/.",
        "library-link//",
        "library-link/./",
        "dead-link",
        "dead-link/",
        "dead-link/.",
        "dead-link//",
    ] {
        let root = library._dir.path().join(spelling);
        let output = index(&[utf8(&root)?])?;
        let reported = format!("{}{}", stdout(&output), stderr(&output));

        assert!(
            !output.status.success(),
            "`{spelling}` is refused, so the run exits non-zero; stdout: {}",
            stdout(&output)
        );
        assert!(
            !reported.contains("secret.pdf") && !stdout(&output).contains("indexed"),
            "nothing behind the link is indexed for `{spelling}`: {reported}"
        );
        assert!(
            stderr(&output).contains("symbolic link"),
            "`{spelling}` is refused as the link it is: {reported}"
        );
    }
    Ok(())
}

/// Normalising the root's spelling must not cost a user a working command.
#[test]
fn a_real_root_exits_zero_however_the_binary_is_spelled() -> TestResult {
    let library = Library::new()?;
    library.write("inside.pdf", "readable")?;

    for spelling in [
        "library",
        "library/",
        "library//",
        "library/.",
        "library/./",
    ] {
        let root = library._dir.path().join(spelling);
        let output = index(&[utf8(&root)?])?;
        assert!(
            output.status.success(),
            "`{spelling}` is the same directory and is indexed; stderr: {}",
            stderr(&output)
        );
        assert!(
            stdout(&output).contains("indexed 1 of 1"),
            "`{spelling}` indexes the library: {}",
            stdout(&output)
        );
    }
    Ok(())
}
