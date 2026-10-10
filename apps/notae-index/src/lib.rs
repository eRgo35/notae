//! The Linux Notae indexer.
//!
//! This crate turns a folder of PDFs into a portable `.notae` sidecar. It
//! reuses [`notae_domain`] for the filename convention and record types, and
//! [`notae_index`] for the SQLite index, search, and reconciliation; the
//! Linux-specific pieces live here.
//!
//! # Shape of a run
//!
//! [`run`] validates the arguments, enumerates the library through a
//! [`FilesystemCatalog`], and returns a [`RunReport`]. The catalog phase is
//! read-only — it fingerprints files, classifies their condition, and
//! observes traversal problems. The pipeline phase lives in
//! [`pipeline::IndexPipeline::run`] and is what turns the catalog's view of
//! the library into indexed records: text extraction, OCR, reconciliation,
//! and per-file bookkeeping.
//!
//! [`run`]: crate::run
//! [`FilesystemCatalog`]: crate::catalog::FilesystemCatalog

pub mod args;
pub mod catalog;
pub mod image;
pub mod pdf;
pub mod pipeline;
pub mod report;

use notae_index::PdfCatalog;

use crate::args::{Cli, CliError};
use crate::catalog::FilesystemCatalog;
use crate::report::RunReport;

/// Re-exports the types a caller of [`run`] reaches for.
pub use crate::pipeline::{IndexPipeline, PipelineError, ProgressEvent};

/// The CLI's own view of one enumerated PDF.
///
/// The catalog yields [`notae_index::PdfFile`] values; this wrapper is what
/// pipeline stages attach to as text extraction, OCR, and sidecar fields are
/// added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfState {
    path: String,
    readable: bool,
    fingerprint: Option<notae_domain::PdfFingerprint>,
}

impl PdfState {
    /// Wraps an enumerated file.
    #[must_use]
    pub fn from_file(file: notae_index::PdfFile) -> Self {
        Self {
            path: file.path().as_str().to_owned(),
            readable: file.is_readable(),
            fingerprint: file.fingerprint().cloned(),
        }
    }

    /// The library-relative path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Whether the catalog read and hashed the file.
    #[must_use]
    pub const fn is_readable(&self) -> bool {
        self.readable
    }

    /// The content fingerprint, when the file was readable.
    #[must_use]
    pub fn fingerprint(&self) -> Option<&notae_domain::PdfFingerprint> {
        self.fingerprint.as_ref()
    }
}

/// Runs the indexer for one parsed command line.
///
/// A name the catalog skipped, an unreadable PDF, and a subdirectory it could
/// not read are all reported through the returned [`RunReport`]. `Err` is
/// reserved for a run that could not start or could not enumerate at all,
/// which is the only case that ends in a non-zero exit status.
///
/// The root is checked exactly once. [`Cli::validate`] opens the directory
/// and hands back the [`ValidatedRoot`](crate::catalog::ValidatedRoot) that
/// is then walked, so there is no second lookup between the check and the
/// use that could accept a replacement; and because that object holds the
/// directory open, the catalog can check the name still resolves to it
/// before enumerating, so the boundary holds across the whole run rather
/// than only at the instant of validation.
///
/// # Errors
/// Returns [`CliError`] when the library root is unusable — absent, not a
/// directory, unreadable, or a symbolic link however it is spelled — or when
/// the tree cannot be enumerated at all. A single unreadable PDF is reported
/// through the returned [`RunReport`], never as an error.
pub fn run(cli: Cli) -> Result<RunReport, CliError> {
    let root = cli.validate()?;
    let catalog = FilesystemCatalog::new(root);
    let verbose = cli.is_verbose();

    let files: Vec<PdfState> = catalog
        .enumerate()?
        .into_iter()
        .map(PdfState::from_file)
        .collect();
    if verbose {
        for file in &files {
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
    for failure in catalog.failures() {
        eprintln!("skipped {}: {}", failure.path(), failure.error());
    }

    Ok(RunReport::from_catalog(&catalog, files))
}
