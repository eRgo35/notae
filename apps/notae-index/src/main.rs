//! The `notae-index` binary.
//!
//! # Exit codes
//!
//! * `0` — the command succeeded: an informational request (`--help`,
//!   `--version`), or a run that produced a [`RunReport`](notae_index_cli::report::RunReport).
//!   A report that carries isolated per-file failures is still a report, so
//!   it exits zero and the failures are printed rather than signalled through
//!   the status. An isolated failure is any problem with something *below*
//!   the library root: a PDF that cannot be read, a subdirectory that
//!   cannot be listed, a skipped symbolic link, or a name the portable path
//!   type rejects.
//! * non-zero — the command could not produce a report at all: the
//!   arguments are invalid, or the library root is unusable. A root is
//!   unusable when it is absent, is not a directory, cannot be read, is a
//!   symbolic link however it is spelled, or no longer names the directory
//!   the run opened — the root is validated once, and the directory that
//!   validation opened is the one that gets walked.
//!
//! The dividing line is the library root itself. A root that exists but
//! cannot be read yields a run whose numbers describe nothing, so it is a
//! command failure rather than a report of zero PDFs — "indexed 0 of 0"
//! would claim a complete scan of a tree the process never saw.
//!
//! A non-zero status therefore means "look at the command", never "some
//! PDFs were unreadable"; per-file problems are reported on standard error
//! and counted in the summary line.

use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use notae_index::IndexStore;
use notae_index_cli::args::{Cli, CliError};
use notae_index_cli::catalog::FilesystemCatalog;
use notae_index_cli::pipeline::{IndexPipeline, PipelineError, ProgressEvent};
use notae_index_cli::report::RunReport;

/// Where the SQLite index is stored on disk.
const DEFAULT_INDEX_PATH: &str = ".notae/index.sqlite";

fn main() -> ExitCode {
    let cli = match Cli::try_parse_from(std::env::args_os()) {
        Ok(cli) => cli,
        Err(CliError::Informational(message)) => {
            println!("{message}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return report_cli_error(error),
    };

    let verbose = cli.is_verbose();
    let catalog = match notae_index_cli::open_catalog(&cli) {
        Ok(catalog) => catalog,
        Err(error) => return report_cli_error(error),
    };

    run_pipeline(catalog, verbose)
}

/// Runs the indexing pipeline against `catalog`.
///
/// Opens the index store at the library root, spawns a progress-drain thread,
/// and runs the pipeline. The thread prints progress events to standard
/// error; the main thread waits for the pipeline to finish and emits the
/// final summary line along with the catalog-phase diagnostics.
///
/// The catalog is walked exactly once across a CLI run: validation builds
/// it, and the pipeline enumerates it. Catalog-phase diagnostics that
/// previously came from the catalog-only path are read off the report
/// after the pipeline runs, so the tree is not re-listed for printing.
fn run_pipeline(catalog: FilesystemCatalog, verbose: bool) -> ExitCode {
    let library_root = catalog.root().to_path_buf();
    let index_path = library_root.join(DEFAULT_INDEX_PATH);
    if let Some(parent) = index_path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "notae-index: could not create index directory at {}: {error}",
            parent.display()
        );
        return ExitCode::FAILURE;
    }
    let mut store = match IndexStore::open(&index_path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!(
                "notae-index: could not open index at {}: {error}",
                index_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let (sender, receiver) = mpsc::channel();
    let printer_receiver = receiver;
    let printer = thread::spawn(move || {
        for event in printer_receiver.iter() {
            if let Some(line) = progress_line(&event) {
                eprintln!("{line}");
            }
        }
    });

    let pipeline = IndexPipeline::default_dpi();
    // The fixed-language policy is a placeholder for the OCR provider the
    // pipeline is configured with. For now the binary does not run OCR — the
    // pipeline still works for text-layer PDFs, which are the bulk of the
    // fixtures — so the policy only matters for image-only PDFs and is left
    // to the OCR provider's defaults when one is configured.
    let policy = notae_ocr::LanguagePolicy::Fixed(vec!["eng".to_string()]);
    struct NoOcr;
    impl notae_ocr::OcrProvider for NoOcr {
        fn recognize(
            &self,
            _page: &notae_ocr::OcrImage,
            _policy: &notae_ocr::LanguagePolicy,
        ) -> Result<notae_ocr::OcrText, notae_ocr::OcrError> {
            Err(notae_ocr::OcrError::EngineInitializationFailed {
                reason: "no OCR provider is configured".to_string(),
            })
        }
    }
    let ocr = NoOcr;

    let outcome = pipeline.run(&catalog, &mut store, &ocr, &policy, sender);
    let _ = printer.join();
    match outcome {
        Ok(report) => {
            print_catalog_diagnostics(&report, verbose);
            print_summary(&report)
        }
        Err(error) => report_pipeline_error(error),
    }
}

/// Emits the catalog-phase diagnostics the run observed.
///
/// These lines previously came from the catalog-only path that
/// `notae_index_cli::run` printed while it built the report. They are now
/// sourced from the [`RunReport`] the pipeline returned — the same
/// observable numbers, no second walk. The verbose flag is honoured so a
/// quiet run still gets the catalog-phase skips (every run reports them)
/// but a quiet user does not get the per-file enumeration.
fn print_catalog_diagnostics(report: &RunReport, verbose: bool) {
    if verbose {
        for file in report.files() {
            eprintln!(
                "{} {}",
                if file.is_readable() {
                    "found"
                } else {
                    "unreadable"
                },
                file.path()
            );
        }
    }
    for failure in report.failures() {
        eprintln!("skipped {}: {}", failure.path(), failure.error());
    }
}

/// Renders one progress event as a one-line message for standard error.
fn progress_line(event: &ProgressEvent) -> Option<String> {
    match event {
        ProgressEvent::Started => Some("starting indexing run".to_string()),
        ProgressEvent::FileStarted { path, position, total } => {
            Some(format!("[{position}/{total}] {path}"))
        }
        ProgressEvent::PageProgress { path, current, total } => Some(format!(
            "{path}: OCR page {current}/{total}"
        )),
        ProgressEvent::FileCompleted { path } => Some(format!("{path}: indexed")),
        ProgressEvent::FileFailed { path, reason } => {
            Some(format!("{path}: failed ({reason})"))
        }
        ProgressEvent::Finished => Some("finished indexing run".to_string()),
    }
}

/// Prints the summary line.
fn print_summary(report: &RunReport) -> ExitCode {
    let baseline = format!(
        "indexed {} of {} PDFs under {}",
        report.readable(),
        report.enumerated(),
        report.library_root().display(),
    );
    let mut extras = Vec::new();
    if report.unreadable() > 0 {
        extras.push(format!("{} unreadable", report.unreadable()));
    }
    if !report.failures().is_empty() {
        extras.push(format!("{} traversal problems", report.failures().len()));
    }
    if report.pdf_failed() > 0 {
        extras.push(format!("{} PDF failures", report.pdf_failed()));
    }
    if report.ocr_failed() > 0 {
        extras.push(format!("{} OCR failures", report.ocr_failed()));
    }
    if report.conflicts() > 0 {
        extras.push(format!("{} duplicate-ID conflicts", report.conflicts()));
    }
    if extras.is_empty() {
        println!("{baseline}");
    } else {
        println!("{baseline} ({})", extras.join(", "));
    }
    ExitCode::SUCCESS
}

/// Reports a CLI-level error to standard error.
fn report_cli_error(error: CliError) -> ExitCode {
    eprintln!("notae-index: {error}");
    ExitCode::FAILURE
}

/// Reports a pipeline-level error to standard error.
fn report_pipeline_error(error: PipelineError) -> ExitCode {
    eprintln!("notae-index: {error}");
    ExitCode::FAILURE
}
