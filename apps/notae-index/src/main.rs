//! The `notae-index` binary.

use std::process::ExitCode;

use notae_index_cli::args::Cli;

fn main() -> ExitCode {
    let cli = match Cli::try_parse_from(std::env::args_os()) {
        Ok(cli) => cli,
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
                ExitCode::SUCCESS
            } else {
                println!(
                    "indexed {} of {} PDFs under {} ({} unreadable, {} traversal problems)",
                    report.readable(),
                    report.enumerated(),
                    report.library_root().display(),
                    report.unreadable(),
                    report.failures().len()
                );
                ExitCode::FAILURE
            }
        }
        Err(error) => report(error),
    }
}

fn report(error: notae_index_cli::args::CliError) -> ExitCode {
    eprintln!("notae-index: {error}");
    ExitCode::FAILURE
}
