//! The `notae-index` binary.
//!
//! # Exit codes
//!
//! * `0` — the command succeeded: an informational request (`--help`,
//!   `--version`), or a run that produced a [`RunReport`]. A report that
//!   carries isolated per-file failures is still a report, so it exits zero and
//!   the failures are printed rather than signalled through the status. An
//!   isolated failure is any problem with something *below* the library root: a
//!   PDF that cannot be read, a subdirectory that cannot be listed, a skipped
//!   symbolic link, or a name the portable path type rejects.
//! * non-zero — the command could not produce a report at all: the arguments
//!   are invalid, or the library root is unusable. A root is unusable when it
//!   is absent, is not a directory, cannot be read, is a symbolic link however
//!   it is spelled, or no longer names the directory the run opened — the root
//!   is validated once, and the directory that validation opened is the one
//!   that gets walked.
//!
//! The dividing line is the library root itself. A root that exists but cannot
//! be read yields a run whose numbers describe nothing, so it is a command
//! failure rather than a report of zero PDFs — "indexed 0 of 0" would claim a
//! complete scan of a tree the process never saw.
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