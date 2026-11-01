//! A host-only `LibraryRootProvider` over an ordinary directory tree.
//!
//! The tests drive it in place of the Storage Access Framework picker: `select_root` pops a
//! queued path, and `reopen` hands back a [`LibrarySource`] whose catalog walks the directory and
//! hashes every PDF. Include it with `#[path = "support/fs_catalog.rs"] mod fs_catalog;`.
#![allow(dead_code)] // Each test binary uses a different part of the helper.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use notae_app_core::storage::{
    LibraryCatalog, LibraryRoot, LibraryRootId, LibraryRootProvider, LibrarySource, StorageError,
};
use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfCatalog, PdfFile};
use sha2::{Digest, Sha256};

/// A directory that exists and is not a symbolic link.
pub struct ValidatedRoot(PathBuf);

impl ValidatedRoot {
    /// Refuses a missing path, a symbolic link and anything that is not a directory.
    pub fn open(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{} is a symbolic link", path.display()));
        }
        if !metadata.is_dir() {
            return Err(format!("{} is not a directory", path.display()));
        }
        Ok(Self(path.to_path_buf()))
    }
}

/// Lists and fingerprints the PDFs below a [`ValidatedRoot`]; symbolic links are skipped.
pub struct FilesystemCatalog {
    root: PathBuf,
}

impl FilesystemCatalog {
    pub fn new(root: ValidatedRoot) -> Self {
        Self { root: root.0 }
    }
}

impl PdfCatalog for FilesystemCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        let mut found = Vec::new();
        collect_pdfs(&self.root, "", &mut found).map_err(|error| {
            CatalogError::Unreadable(format!("{}: {error}", self.root.display()))
        })?;
        found.sort();
        Ok(found
            .into_iter()
            .filter_map(|relative| {
                let path = RelativePdfPath::new(&relative).ok()?;
                let filename = relative.rsplit('/').next().unwrap_or(&relative).to_owned();
                let condition = match fingerprint(&self.root.join(&relative)) {
                    Some(fingerprint) => FileCondition::Readable(fingerprint),
                    None => FileCondition::Unreadable,
                };
                Some(PdfFile::with_condition(path, filename, condition))
            })
            .collect())
    }
}

fn collect_pdfs(dir: &Path, prefix: &str, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let joined = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if file_type.is_dir() {
            collect_pdfs(&entry.path(), &joined, out)?;
        } else if name.to_ascii_lowercase().ends_with(".pdf") {
            out.push(joined);
        }
    }
    Ok(())
}

fn fingerprint(path: &Path) -> Option<PdfFingerprint> {
    let mut file = File::open(path).ok()?;
    let modified = file.metadata().ok()?.modified().ok();
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut byte_len = 0_u64;
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        byte_len += u64::try_from(read).ok()?;
    }
    PdfFingerprint::from_system_time(
        Sha256Digest::from_bytes(hasher.finalize().into()),
        byte_len,
        modified,
    )
}

struct HostLibraryCatalog(FilesystemCatalog);

impl PdfCatalog for HostLibraryCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        self.0.enumerate()
    }
}

impl LibraryCatalog for HostLibraryCatalog {}

/// What the next `select_root` call returns.
#[derive(Debug)]
enum QueuedSelection {
    Directory(PathBuf),
    /// A path that is not a directory, to assert [`StorageError::NotADirectory`].
    NotADirectory,
}

#[derive(Debug, Default)]
struct Inner {
    next_selection: Option<QueuedSelection>,
    persisted: HashMap<LibraryRootId, LibraryRoot>,
}

/// The Linux-side `LibraryRootProvider`.
///
/// Drives `select_root` from an in-memory queue so the host tests can exercise the state machine
/// without a UI; an empty queue produces [`StorageError::Cancelled`].
#[derive(Debug, Default)]
pub struct FilesystemLibraryRootProvider {
    inner: Mutex<Inner>,
}

impl FilesystemLibraryRootProvider {
    /// Returns a provider with an empty selection queue.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("provider mutex poisoned")
    }

    /// Queues a directory for the next `select_root` call.
    pub fn queue_directory(&self, path: impl Into<PathBuf>) {
        self.lock().next_selection = Some(QueuedSelection::Directory(path.into()));
    }

    /// Queues a non-directory path for the next `select_root` call, which rejects it.
    pub fn queue_non_directory(&self, _path: impl Into<PathBuf>) {
        self.lock().next_selection = Some(QueuedSelection::NotADirectory);
    }

    /// Persists a root as if the platform had granted its permission.
    pub fn remember(&self, root: LibraryRoot) {
        self.lock().persisted.insert(root.id().clone(), root);
    }

    /// Drops the persisted entry, so the next `reopen(root)` reports `PermissionLost`.
    pub fn revoke_persisted(&self, root: &LibraryRoot) {
        self.lock().persisted.remove(root.id());
    }
}

impl LibraryRootProvider for FilesystemLibraryRootProvider {
    async fn select_root(&self) -> Result<LibraryRoot, StorageError> {
        let queued = self.lock().next_selection.take();
        let path = match queued {
            Some(QueuedSelection::Directory(path)) => path,
            Some(QueuedSelection::NotADirectory) => return Err(StorageError::NotADirectory),
            None => return Err(StorageError::Cancelled),
        };
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| StorageError::Other(error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(StorageError::Other(format!(
                "{} is a symbolic link, which is never followed",
                path.display()
            )));
        }
        if !metadata.is_dir() {
            return Err(StorageError::NotADirectory);
        }
        let display_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let root = LibraryRoot::new(display_name, path.to_string_lossy());
        self.remember(root.clone());
        Ok(root)
    }

    fn reopen(&self, root: &LibraryRoot) -> Result<LibrarySource, StorageError> {
        if !self.lock().persisted.contains_key(root.id()) {
            return Err(StorageError::PermissionLost);
        }
        let path = PathBuf::from(root.handle());
        // A vanished or non-directory root has one remedy (pick the folder again), so both map to
        // `PermissionLost` and the recovery screen offers one affordance.
        let validated = ValidatedRoot::open(&path).map_err(|message| {
            if message.contains("No such file") || message.contains("not a directory") {
                StorageError::PermissionLost
            } else {
                StorageError::Other(message)
            }
        })?;
        let catalog = HostLibraryCatalog(FilesystemCatalog::new(validated));
        Ok(LibrarySource::new(root.clone(), Box::new(catalog)))
    }
}
