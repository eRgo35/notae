//! The `notae-index` binary.
//!
//! # Exit codes
//!
//! * `0` — the command succeeded: an informational request (`--help`,
//!   `--version`), or a run that produced a [`RunReport`]. A report that
//!   carries isolated per-file failures is still a report, so it exits zero and
//!   the failures are printed rather than signalled through the status.
//! * non-zero — the command could not produce a report at all: the arguments
//!   are invalid, or the library root is unusable.
//!
//! A non-zero status therefore means "look at the command", never "some PDFs
//! were unreadable"; per-file problems are reported on standard error and
//! counted in the summary line.

use std::process::ExitCode;

use notae_index_cli::args::{Cli, CliError};

fn main() -> ExitCode {
    let cli = match Cli::try_parse_from(std::env::args_os()) {
        Ok(cli) => cli,
        Err(CliError::Informational(message)) => {
            println!("{message}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return report(error),
    };
    match notae_index_cli::run(cli) {
        Ok(report) => {
            if report.failures().is_empty() && report.unreadable() == 0 {
                println!(
                    "indexed {} of {} PDFs under {}",
                    report.readable(),
                    report.enumerated(),
                    report.library_root().display()
                );
            } else {
                println!(
                    "indexed {} of {} PDFs under {} ({} unreadable, {} traversal problems)",
                    report.readable(),
                    report.enumerated(),
                    report.library_root().display(),
                    report.unreadable(),
                    report.failures().len()
                );
                // The run produced a valid report, so the exit status is
                // success. `run` already printed each failure on standard
                // error; the summary line counts them.
            }
            ExitCode::SUCCESS
        }
        Err(error) => report(error),
    }
}

fn report(error: CliError) -> ExitCode {
    eprintln!("notae-index: {error}");
    ExitCode::FAILURE
}