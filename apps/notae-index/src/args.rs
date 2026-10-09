//! Command line arguments for the Linux indexer.
//!
//! The command shape is fixed: `notae-index <library-root>`. Parsing is a pure
//! function of the process arguments and [`Cli::validate`] only inspects the
//! filesystem, so the argument layer stays unchanged as later pipeline stages
//! are added behind [`crate::run`].

use std::path::{Path, PathBuf};

use clap::Parser;
use thiserror::Error;

/// The parsed command line.
#[derive(Debug, Clone, PartialEq, Eq, Parser)]
#[command(
    name = "notae-index",
    about = "Index a folder of PDFs into a portable Notae sidecar.",
    version
)]
pub struct Cli {
    /// The library root: the folder whose PDFs are indexed.
    #[arg(value_name = "LIBRARY_ROOT")]
    library_root: PathBuf,

    /// Print per-file progress to standard error. Traversal failures are
    /// printed either way.
    #[arg(short, long)]
    verbose: bool,
}

impl Cli {
    /// Parses arguments from the process argument list, excluding `argv[0]`.
    ///
    /// # Errors
    /// Returns [`CliError::Usage`] when the arguments do not match the command
    /// shape, for example when the required root is missing, and
    /// [`CliError::Informational`] when the request was `--help` or
    /// `--version`. Those two are successes, so they are distinguished here
    /// rather than folded into [`CliError::Usage`]; the caller prints the
    /// first to stdout and exits zero.
    pub fn try_parse_from<I, T>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(args).map_err(|error| {
            if error.use_stderr() {
                CliError::Usage(error.to_string())
            } else {
                CliError::Informational(error.to_string())
            }
        })
    }

    /// The library root named on the command line.
    #[must_use]
    pub fn library_root(&self) -> &Path {
        self.library_root.as_path()
    }

    /// Whether verbose progress output was requested.
    #[must_use]
    pub const fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Checks that the library root exists and is a directory this run can
    /// read.
    ///
    /// The same boundary policy the catalog applies is applied here, so a root
    /// that the walk could not trust is refused before it is walked: an
    /// unreadable root, and a root that is a symbolic link.
    ///
    /// # Errors
    /// Returns [`CliError::RootUnreadable`] when the root is absent, is not a
    /// directory, is a symbolic link, or cannot be opened for reading.
    pub fn validate(&self) -> Result<(), CliError> {
        let metadata = std::fs::symlink_metadata(&self.library_root)
            .map_err(|_| CliError::RootUnreadable(self.library_root.clone()))?;
        if metadata.file_type().is_symlink() {
            return Err(CliError::RootSymlinked(self.library_root.clone()));
        }
        if !metadata.is_dir() {
            return Err(CliError::RootUnreadable(self.library_root.clone()));
        }
        std::fs::read_dir(&self.library_root)
            .map_err(|_| CliError::RootUnreadable(self.library_root.clone()))?;
        Ok(())
    }
}

/// Failures produced before or during a run.
#[derive(Debug, Error)]
pub enum CliError {
    /// The command line asked for something informational, such as `--help` or
    /// `--version`. This is not a failure: the message goes to standard output
    /// and the process exits zero.
    #[error("{0}")]
    Informational(String),

    /// The command line did not match the command shape.
    #[error("{0}")]
    Usage(String),

    /// The library root does not exist, is not a directory, or cannot be read.
    #[error("library root is not readable: {0}")]
    RootUnreadable(PathBuf),

    /// The library root is a symbolic link. Symbolic links are never followed,
    /// so the run refuses the name rather than indexing whatever it points at.
    #[error(
        "library root is a symbolic link, which is never followed: {0}; \
         pass the directory it points at"
    )]
    RootSymlinked(PathBuf),

    /// Enumeration of the library failed as a whole.
    #[error(transparent)]
    Catalog(#[from] notae_index::CatalogError),
}
