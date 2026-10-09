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
//!   all (missing/unusable root, a catalog-wide failure).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

struct Library {
    _dir: TempDir,
    root: std::path::PathBuf,
}

impl Library {
    fn new() -> Self {
        let dir = TempDir::new().expect("temporary parent directory");
        let root = dir.path().join("library");
        fs::create_dir_all(&root).expect("create the library root");
        Self { _dir: dir, root }
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, content).expect("write fixture");
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

fn index(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_notae-index"))
        .args(args)
        .output()
        .expect("the built binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_exits_successfully() {
    let output = index(&["--help"]);
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
}

#[test]
fn version_exits_successfully() {
    let output = index(&["--version"]);
    assert!(
        output.status.success(),
        "--version must exit 0; stderr: {}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).trim().is_empty(),
        "--version prints the version on stdout"
    );
}

#[test]
fn a_missing_root_argument_is_a_usage_error() {
    let output = index(&[]);
    assert!(
        !output.status.success(),
        "an invalid command line exits non-zero"
    );
    assert!(
        stderr(&output).contains("LIBRARY_ROOT"),
        "the usage error names the argument: {}",
        stderr(&output)
    );
}

#[test]
fn an_unknown_flag_is_a_usage_error() {
    let output = index(&["/tmp", "--nonsense"]);
    assert!(
        !output.status.success(),
        "an unknown flag exits non-zero"
    );
    assert!(
        !stderr(&output).is_empty(),
        "the usage error is reported on stderr"
    );
}

#[test]
fn an_unusable_root_exits_non_zero() {
    let output = index(&["/definitely/not/a/real/library"]);
    assert!(
        !output.status.success(),
        "a run that cannot produce a report exits non-zero"
    );
    assert!(
        stderr(&output).contains("library root"),
        "the reason is reported: {}",
        stderr(&output)
    );
}

#[test]
fn a_completed_run_with_isolated_failures_exits_zero_and_reports_them() {
    let library = Library::new();
    library.write("good.pdf", "readable");
    // A filename the domain type rejects: an isolated per-file failure.
    library.write("C:.pdf", "drive");

    let output = index(&[library.root().to_str().expect("utf-8 path"), "--verbose"]);
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
}

#[test]
fn a_clean_run_exits_zero() {
    let library = Library::new();
    library.write("good.pdf", "readable");

    let output = index(&[library.root().to_str().expect("utf-8 path")]);
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
}