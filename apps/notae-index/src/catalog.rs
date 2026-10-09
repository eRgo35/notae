//! A [`PdfCatalog`] over an ordinary Linux directory tree.
//!
//! This is the only place in the CLI that knows about `std::fs`. Everything
//! above it consumes [`PdfFile`] values, so the Android Storage Access
//! Framework adapter and this desktop walk are interchangeable.

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfCatalog, PdfFile};
use sha2::{Digest, Sha256};

/// Read buffer for hashing; large enough that hashing is not syscall bound.
const READ_BUFFER: usize = 64 * 1024;

/// One traversal problem that did not stop enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFailure {
    /// The path that could not be visited or read, relative to the root when
    /// it is inside it.
    path: String,
    /// The operating system's description of the problem.
    error: String,
}

impl CatalogFailure {
    /// The path the failure concerns.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The underlying error message.
    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }
}

/// Enumerates the PDFs under one library root.
///
/// Per-file problems — a file that cannot be opened, a directory that cannot be
/// read — are recorded in [`failures`](Self::failures) and the corresponding
/// entry is still enumerated as [`FileCondition::Unreadable`] where the path is
/// known, so one bad PDF cannot abort a batch.
#[derive(Debug)]
pub struct FilesystemCatalog {
    root: PathBuf,
    failures: RefCell<Vec<CatalogFailure>>,
}

impl FilesystemCatalog {
    /// Opens a library root for enumeration.
    ///
    /// # Errors
    /// Returns [`CatalogError::Unreadable`] when the root does not exist, is
    /// not a directory, or cannot be opened.
    pub fn new(root: PathBuf) -> Result<Self, CatalogError> {
        let metadata = std::fs::metadata(&root)
            .map_err(|error| CatalogError::Unreadable(format!("{}: {error}", root.display())))?;
        if !metadata.is_dir() {
            return Err(CatalogError::Unreadable(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        Ok(Self {
            root,
            failures: RefCell::new(Vec::new()),
        })
    }

    /// The library root this catalog walks.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every traversal problem recorded since the catalog was opened.
    #[must_use]
    pub fn failures(&self) -> Vec<CatalogFailure> {
        self.failures.borrow().clone()
    }

    fn record(&self, path: &Path, error: &io::Error) {
        record_failure(&self.failures, &self.root, path, error);
    }
}

impl PdfCatalog for FilesystemCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        let mut relatives = Vec::new();
        collect_pdf_paths(&self.root, &self.root, "", &mut relatives, &self.failures);
        relatives.sort();

        let files = relatives
            .into_iter()
            .map(|relative| {
                let filename = relative
                    .rsplit('/')
                    .next()
                    .unwrap_or(relative.as_str())
                    .to_owned();
                let path = RelativePdfPath::new(relative)
                    .expect("collected paths are already root relative");
                let absolute = self.root.join(path.as_str());
                match fingerprint(&absolute) {
                    Ok(fingerprint) => PdfFile::with_condition(
                        path,
                        filename,
                        FileCondition::Readable(fingerprint),
                    ),
                    Err(error) => {
                        self.record(&absolute, &error);
                        PdfFile::with_condition(path, filename, FileCondition::Unreadable)
                    }
                }
            })
            .collect();
        Ok(files)
    }
}

/// Collects every `.pdf`/`.PDF` path below `dir`, as `prefix`-joined
/// forward-slash strings relative to the library root.
fn collect_pdf_paths(
    library_root: &Path,
    dir: &Path,
    prefix: &str,
    out: &mut Vec<String>,
    failures: &RefCell<Vec<CatalogFailure>>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            record_failure(failures, library_root, dir, &error);
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                record_failure(failures, library_root, dir, &error);
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                record_failure(failures, library_root, &path, &error);
                continue;
            }
        };
        if file_type.is_dir() {
            let nested = if prefix.is_empty() {
                name.to_string_lossy().into_owned()
            } else {
                format!("{prefix}/{}", name.to_string_lossy())
            };
            collect_pdf_paths(library_root, &path, &nested, out, failures);
            continue;
        }
        if !is_pdf(&name) {
            continue;
        }
        out.push(if prefix.is_empty() {
            name.to_string_lossy().into_owned()
        } else {
            format!("{prefix}/{}", name.to_string_lossy())
        });
    }
}

fn record_failure(
    failures: &RefCell<Vec<CatalogFailure>>,
    root: &Path,
    path: &Path,
    error: &io::Error,
) {
    let display = match path.strip_prefix(root) {
        Ok(relative) => relative.display().to_string(),
        Err(_) => path.display().to_string(),
    };
    failures.borrow_mut().push(CatalogFailure {
        path: display,
        error: error.to_string(),
    });
}

/// Whether a directory entry name names a PDF, case-insensitively.
fn is_pdf(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case(".pdf"))
}

/// Hashes one file end to end and collects its platform metadata.
///
/// # Errors
/// Returns the I/O error when the file cannot be opened or read, so the caller
/// can record it and enumerate the file as unreadable.
fn fingerprint(path: &Path) -> Result<PdfFingerprint, io::Error> {
    let metadata = std::fs::metadata(path)?;
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER];
    let mut byte_len = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        byte_len += u64::try_from(read).expect("a chunk length fits in u64");
    }
    let digest = hasher.finalize();
    let sha256 = Sha256Digest::from_bytes(digest.into());
    PdfFingerprint::from_system_time(sha256, byte_len, metadata.modified().ok())
        .ok_or_else(|| io::Error::other("the file's modification time is out of range"))
}
