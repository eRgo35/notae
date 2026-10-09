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
//! [`FilesystemCatalog`], and returns a [`RunReport`]. Every later stage of the
//! pipeline hangs off that enumeration: text extraction and page rendering
//! (PDF adapters), OCR, reconciliation against the index, and sidecar output.
//! Argument handling is a separate module and never changes shape as stages are
//! added.

pub mod args;
pub mod catalog;
pub mod image;
pub mod pdf;

use std::path::{Path, PathBuf};

use notae_index::PdfCatalog;

use crate::args::{Cli, CliError};
use crate::catalog::{CatalogFailure, FilesystemCatalog};

/// What a run observed, independent of what it wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    library_root: PathBuf,
    files: Vec<PdfState>,
    failures: Vec<CatalogFailure>,
}

impl RunReport {
    /// The library root that was enumerated.
    ///
    /// This is the name the run actually used: the name the user gave, without
    /// the trailing separators and `.` components Linux resolves away before
    /// it stats anything (see [`crate::catalog::ValidatedRoot::open`]). It is
    /// the directory that was opened, so `library`, `library/` and
    /// `library/.` all report the same root.
    #[must_use]
    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    /// The enumerated files, in stable relative-path order.
    #[must_use]
    pub fn files(&self) -> &[PdfState] {
        &self.files
    }

    /// The library-relative path of each enumerated file, in order.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.files.iter().map(|file| file.path()).collect()
    }

    /// How many PDFs were enumerated.
    #[must_use]
    pub fn enumerated(&self) -> usize {
        self.files.len()
    }

    /// How many PDFs were read and fingerprinted.
    #[must_use]
    pub fn readable(&self) -> usize {
        self.files.iter().filter(|file| file.is_readable()).count()
    }

    /// How many PDFs could not be read.
    #[must_use]
    pub fn unreadable(&self) -> usize {
        self.files.iter().filter(|file| !file.is_readable()).count()
    }

    /// Traversal problems that did not stop enumeration.
    #[must_use]
    pub fn failures(&self) -> &[CatalogFailure] {
        &self.failures
    }
}

/// Runs the indexer for one parsed command line.
///
/// A name the catalog skipped, an unreadable PDF, and a subdirectory it could
/// not read are all reported through the returned [`RunReport`]. `Err` is
/// reserved for a run that could not start or could not enumerate at all,
/// which is the only case that ends in a non-zero exit status. The boundary is
/// the library root: a root that cannot be read, that is a symbolic link, or
/// that is replaced between being validated and being walked, ends the run; the
/// same problem anywhere below it is one entry in the report.
///
/// The root is checked exactly once. [`Cli::validate`] opens the directory and
/// hands back the [`ValidatedRoot`] that is then walked, so there is no second
/// lookup between the check and the use that could accept a replacement; and
/// because that object holds the directory open, the catalog can check the name
/// still resolves to it before enumerating, so the boundary holds across the
/// whole run rather than only at the instant of validation. Later stages — text
/// extraction, OCR, reconciliation, sidecar output — hang off
/// [`RunReport::files`] and report their own per-file problems the same way, so
/// `Ok` keeps meaning "a report was produced", not "everything succeeded".
///
/// # Errors
/// Returns [`CliError`] when the library root is unusable — absent, not a
/// directory, unreadable, or a symbolic link however it is spelled — or when
/// the tree cannot be enumerated at all. A single unreadable PDF is reported
/// through the returned [`RunReport`], never as an error.
pub fn run(cli: Cli) -> Result<RunReport, CliError> {
    // The one root check, and the object that answers it: what is walked is
    // what was opened, not a name looked at twice.
    let root = cli.validate()?;
    let catalog = FilesystemCatalog::new(root);
    let verbose = cli.is_verbose();

    let files = catalog
        .enumerate()?
        .into_iter()
        .map(PdfState::from_file)
        .collect::<Vec<_>>();
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
    // Failures are printed whatever the verbosity: a run that reports per-file
    // problems is still a successful run, so the exit status cannot carry them.
    for failure in catalog.failures() {
        eprintln!("skipped {}: {}", failure.path(), failure.error());
    }

    Ok(RunReport {
        library_root: catalog.root().to_path_buf(),
        failures: catalog.failures(),
        files,
    })
}

/// The CLI's own view of one enumerated PDF.
///
/// The catalog yields [`notae_index::PdfFile`] values; this wrapper is what the
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
    pub fn is_readable(&self) -> bool {
        self.readable
    }

    /// The content fingerprint, when the file was readable.
    #[must_use]
    pub fn fingerprint(&self) -> Option<&notae_domain::PdfFingerprint> {
        self.fingerprint.as_ref()
    }
}
