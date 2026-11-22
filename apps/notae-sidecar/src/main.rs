//! `notae-sidecar <library-folder>`: writes `.notae/` for a score library.

use std::path::Path;
use std::process::ExitCode;

use notae_sidecar::{Pdftoppm, run};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [library] = args.as_slice() else {
        eprintln!("usage: notae-sidecar <library-folder>");
        return ExitCode::from(2);
    };
    match run(Path::new(library), &Pdftoppm) {
        Ok(report) => {
            for w in &report.warnings {
                eprintln!("warning: {w}");
            }
            for e in &report.errors {
                eprintln!("error: {e}");
            }
            if report.errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                eprintln!("nothing was written");
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
