//! A [`PdfCatalog`] over an ordinary Linux directory tree.
//!
//! This is the only place in the CLI that knows about `std::fs`. Everything
//! above it consumes [`PdfFile`] values, so the Android Storage Access
//! Framework adapter and this desktop walk are interchangeable.
//!
//! # The library root boundary
//!
//! Two rules keep a run inside the root it was given.
//!
//! * **Symbolic links are never followed.** A symlinked file, including one
//!   that resolves to a target still inside the root, is skipped and reported
//!   as a [`CatalogFailure`]; a symlinked directory is not walked. Skipping
//!   unconditionally — rather than canonicalising the target and comparing it
//!   with the canonical root — is what makes the guarantee hold: the link is
//!   never opened, so no bytes from outside the root can be read even if the
//!   link is retargeted between the check and the read, and a symlink loop
//!   terminates because the walk never follows a link in the first place.
//! * **A name the domain type cannot represent is a reported skip.** POSIX
//!   permits filenames the portable [`RelativePdfPath`] rejects, such as
//!   `C:.pdf`, whose leading `X:` would be a Windows drive prefix in a
//!   sidecar. Those files are not indexed, because indexing them would put a
//!   non-portable path in the sidecar; they are recorded as
//!   [`CatalogFailure`]s and the rest of the batch continues. The domain
//!   invariant stays strict and the catalog degrades, so one unusual Linux
//!   filename costs one file rather than the whole run.

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

/// Records one traversal problem.
///
/// The path is shown relative to `root` when it lies inside it, so reports read
/// the same way as the paths that succeeded. The root itself has no relative
/// form — [`Path::strip_prefix`] yields an empty path for it — so an empty
/// result falls back to the root as given, which is what identifies the thing
/// that failed.
fn push_failure(
    failures: &RefCell<Vec<CatalogFailure>>,
    root: &Path,
    path: &Path,
    error: impl Into<String>,
) {
    let relative = path.strip_prefix(root).map_or_else(
        |_| path.display().to_string(),
        |inside| {
            let inside = inside.display().to_string();
            if inside.is_empty() {
                root.display().to_string()
            } else {
                inside
            }
        },
    );
    failures.borrow_mut().push(CatalogFailure {
        path: relative,
        error: error.into(),
    });
}

/// Enumerates the PDFs under one library root.
///
/// Per-file problems — a file that cannot be opened, a directory that cannot be
/// read — are recorded in [`failures`](Self::failures) and the corresponding
/// entry is still enumerated as [`FileCondition::Unreadable`] where the path is
/// known, so one bad PDF cannot abort a batch. Entries that are skipped
/// outright — a symlink, or a name the portable path type rejects — appear in
/// [`failures`](Self::failures) only; see the module documentation for the
/// root-boundary rules that decide this.
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
        push_failure(&self.failures, &self.root, path, error.to_string());
    }

    fn record_message(&self, path: &Path, error: impl Into<String>) {
        push_failure(&self.failures, &self.root, path, error);
    }
}

impl PdfCatalog for FilesystemCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        let mut relatives = Vec::new();
        collect_pdf_paths(&self.root, &self.root, "", &mut relatives, &self.failures);
        relatives.sort();

        let files = relatives
            .into_iter()
            .filter_map(|relative| {
                let filename = relative
                    .rsplit('/')
                    .next()
                    .unwrap_or(relative.as_str())
                    .to_owned();
                // A name POSIX allows but the portable path type refuses is a
                // reported skip, not a panic: one file is lost, not the batch.
                let path = match RelativePdfPath::new(&relative) {
                    Ok(path) => path,
                    Err(error) => {
                        self.record_message(Path::new(&relative), error.to_string());
                        return None;
                    }
                };
                let absolute = self.root.join(path.as_str());
                Some(match fingerprint(&absolute) {
                    Ok(fingerprint) => PdfFile::with_condition(
                        path,
                        filename,
                        FileCondition::Readable(fingerprint),
                    ),
                    Err(error) => {
                        self.record(&absolute, &error);
                        PdfFile::with_condition(path, filename, FileCondition::Unreadable)
                    }
                })
            })
            .collect();
        Ok(files)
    }
}

/// Collects every `.pdf`/`.PDF` path below `dir`, as `prefix`-joined
/// forward-slash strings relative to the library root.
///
/// Symbolic links are skipped without being followed; see the module
/// documentation for why. `DirEntry::file_type` does not traverse a link, so
/// this holds even for a loop or a link that leaves the root.
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
            push_failure(failures, library_root, dir, error.to_string());
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_failure(failures, library_root, dir, error.to_string());
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                push_failure(failures, library_root, &path, error.to_string());
                continue;
            }
        };
        if file_type.is_symlink() {
            push_failure(
                failures,
                library_root,
                &path,
                format!("{}: symbolic link skipped", name.to_string_lossy()),
            );
            continue;
        }
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

/// Whether a directory entry name names a PDF, case-insensitively.
fn is_pdf(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case(".pdf"))
}

/// Hashes one file end to end and collects its platform metadata.
///
/// The link check is the last line of defence for the root boundary. The walk
/// already refuses to collect a symlink, but a regular file can be replaced by
/// one between collection and this call; [`std::fs::symlink_metadata`] inspects
/// the name itself, and comparing its device and inode with those of the handle
/// that was actually opened proves the bytes came from the vetted entry. The
/// check costs two `stat` calls and no syscall beyond them.
///
/// # Errors
/// Returns the I/O error when the file is a symbolic link, changed identity
/// between the two calls, or cannot be opened or read, so the caller can record
/// it and enumerate the file as unreadable.
fn fingerprint(path: &Path) -> Result<PdfFingerprint, io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other("symbolic link skipped"));
    }
    let mut file = File::open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err(io::Error::other(
                "the entry changed identity while it was being read",
            ));
        }
    }
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
