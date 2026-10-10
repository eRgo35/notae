//! Storage Access Framework glue and the host-side equivalent.
//!
//! This module defines the *contract* every [`LibraryRootProvider`]
//! implementation satisfies. The Android implementation lives in
//! `crate::android::saf` (Android-only); the Linux implementation lives
//! in this file under `#[cfg(not(target_os = "android"))]` and is what
//! the host tests drive.
//!
//! # Why the root is opaque
//!
//! The selected library root is whatever the platform hands us back from
//! `ACTION_OPEN_DOCUMENT_TREE` — on Android, a `tree:` URI that does not
//! resolve to a `Path` at all. Treating the root as a `Path` would lose
//! that distinction: the SAF root is permission-bound, the directory is
//! not necessarily under the app's data directory, and the system can
//! revoke access at any moment without warning. The contract here keeps
//! the root as an opaque handle ([`LibraryRoot::handle`]) and lets the
//! provider translate it back to whatever native form its reopen path
//! needs.
//!
//! # The async story
//!
//! [`LibraryRootProvider::select_root`] is `async` because on Android the
//! Storage Access Framework returns its result through the Activity
//! Result callback, which the host-side `android-activity` runtime
//! bridges to an async future. We use native AFIT (Rust 1.75+ async
//! functions in traits), not `#[async_trait]`: the workspace is on
//! 1.98.1, the trait is consumed through `impl` (never `dyn`), and the
//! extra box per call that `#[async_trait]` would impose is irrelevant
//! given that selection happens once per app session. The returned
//! future is `Send` because the Android runtime hands the result back
//! on a different thread than the caller; host tests use
//! [`pollster::block_on`] to await it from the test thread.
//!
//! # What `LibrarySource` is
//!
//! [`LibrarySource`] is what the rest of the app handles around. It
//! implements [`notae_index::PdfCatalog`] so the rescan logic in Task 4
//! can iterate PDFs through the same trait whether the catalog walks a
//! Linux directory or a SAF tree. The catalog itself is held as
//! `Box<dyn PdfCatalog + Send>`: the `+ Send` is what lets a future
//! `OperationController` move the source to a worker thread. The trait
//! is object-safe (`enumerate(&self)` returning `Result<Vec<PdfFile>,
//! CatalogError>`), so a boxed reference is fine.
//!
//! # Send, not Sync
//!
//! [`LibrarySource`] is `Send` and intentionally not `Sync`. A rescan
//! takes exclusive ownership of one source for the duration of the
//! enumeration, and the platform-specific catalogs underneath are not
//! safe to share (`FilesystemCatalog` uses `RefCell` for failure
//! accumulation; the SAF adapter streams documents off the
//! `ContentResolver`). Promoting one to `Sync` would force a `Mutex`
//! into a hot path that does not need it.

#![forbid(unsafe_code)]

use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use notae_index::{CatalogError, PdfCatalog, PdfFile};

/// A monotonic counter used to disambiguate [`LibraryRootId`] values
/// created in the same nanosecond. Wall-clock nanoseconds give us a
/// monotonic-enough id; the counter breaks ties on a fast machine and
/// on a rewound clock.
static ROOT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique, opaque identifier for a library root.
///
/// The id is generated when the root is first selected and stays with
/// the root across persistence and reopen. It is what a caller uses to
/// refer to the root without having to carry the whole
/// [`LibraryRoot`] around.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LibraryRootId(String);

impl LibraryRootId {
    /// Builds an id from a string. Used by tests and by the
    /// persistence layer when re-loading a previously-persisted id.
    #[must_use]
    pub fn from_string(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LibraryRootId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The library root the user selected.
///
/// The handle is whatever the platform returned from selection: on
/// Android a `tree:` URI string; on host a serialised path. It is
/// deliberately opaque — only the provider that produced it knows how
/// to turn it back into something it can walk. The display name is what
/// the UI shows in the setup and permission-lost screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRoot {
    id: LibraryRootId,
    display_name: String,
    handle: String,
    /// Last time `reopen` successfully verified the root. On Android
    /// this drives the decision to re-check the persisted permission
    /// flag; on host it is set at selection time for symmetry.
    last_verified: SystemTime,
}

impl LibraryRoot {
    /// Builds a root from its parts. Used by the Android adapter when
    /// it converts an `ACTION_OPEN_DOCUMENT_TREE` result, and by the
    /// host adapter when a test queues a path.
    #[must_use]
    pub fn new(display_name: impl Into<String>, handle: impl Into<String>) -> Self {
        Self {
            id: next_root_id(),
            display_name: display_name.into(),
            handle: handle.into(),
            last_verified: SystemTime::now(),
        }
    }

    /// Reconstructs a root from its serialised form (id, display name,
    /// handle). Used by the Android adapter when re-loading a persisted
    /// root from `SharedPreferences`.
    #[must_use]
    pub fn from_parts(id: LibraryRootId, display_name: String, handle: String) -> Self {
        Self {
            id,
            display_name,
            handle,
            last_verified: UNIX_EPOCH,
        }
    }

    /// Builds an empty placeholder root. Used by the Android
    /// provider when it needs to surface a [`StorageError::PermissionLost`]
    /// but has no persisted root to attach to (the user has not
    /// picked anything yet, or the persisted state has been cleared).
    /// The placeholder carries a fresh id and empty fields so the
    /// error carries a `LibraryRoot` shape the storage state can
    /// render.
    #[must_use]
    pub fn placeholder() -> Self {
        Self::new("", "")
    }

    /// Returns the root's unique id.
    #[must_use]
    pub fn id(&self) -> &LibraryRootId {
        &self.id
    }

    /// Returns the user-facing display name (e.g. the folder name).
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the opaque, persistence-shaped handle (a `tree:` URI on
    /// Android, a serialised path on host).
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Returns the timestamp at which the root was last verified
    /// readable.
    #[must_use]
    pub fn last_verified(&self) -> SystemTime {
        self.last_verified
    }
}

fn next_root_id() -> LibraryRootId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = ROOT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    LibraryRootId(format!("root-{nanos}-{counter}"))
}

/// Errors a [`LibraryRootProvider`] can surface.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The user dismissed the SAF picker without choosing anything.
    #[error("no library root was selected")]
    Cancelled,

    /// The user selected something that is not a directory (e.g. a
    /// single PDF in `ACTION_OPEN_DOCUMENT`).
    #[error("the selected item is not a directory")]
    NotADirectory,

    /// The persisted permission for this root was revoked. The user
    /// has to re-select the same folder through the SAF picker to
    /// re-grant read access.
    #[error("permission for the library root was lost; reauthorize to continue")]
    PermissionLost,

    /// The platform refused to grant the SAF permission outright
    /// (e.g. a `SecurityException` from the JNI side or a missing
    /// manifest entry). Distinct from [`PermissionLost`](Self::PermissionLost),
    /// which is a revocation after-the-fact: this one never had the
    /// permission to begin with.
    #[error("the platform refused to grant the SAF permission")]
    PermissionDenied,

    /// A lower-level failure: an I/O error, a JNI panic, an unexpected
    /// platform response. The string is what the platform reported.
    #[error("storage error: {0}")]
    Other(String),
}

/// The high-level state of the storage layer.
///
/// `Ready` carries a live [`LibrarySource`] so the UI can render the
/// library list. `PermissionLost` carries the original
/// [`LibraryRoot`] so the recovery screen can offer a "reauthorize"
/// button that re-uses the same handle. `Error` carries the
/// [`StorageError`] so the UI can show a retry-able message.
#[derive(Debug)]
pub enum StorageState {
    /// The user has not picked a library root yet, or the install is
    /// fresh. The UI shows the setup screen.
    Unconfigured,
    /// A library root is selected and open; the contained
    /// [`LibrarySource`] is what the rescan logic will consume.
    Ready(LibrarySource),
    /// The root's persisted permission was revoked. The UI shows the
    /// recovery screen with the root's display name.
    PermissionLost(LibraryRoot),
    /// Storage failed in a way that is not recoverable without user
    /// intervention; the UI shows an error message.
    Error(StorageError),
}

impl StorageState {
    /// Builds the initial state.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self::Unconfigured
    }

    /// Reports the state without taking the contained source. Used by
    /// tests and by the UI when it wants a quick classifier.
    #[must_use]
    pub fn kind(&self) -> StorageStateKind {
        match self {
            Self::Unconfigured => StorageStateKind::Unconfigured,
            Self::Ready(_) => StorageStateKind::Ready,
            Self::PermissionLost(_) => StorageStateKind::PermissionLost,
            Self::Error(_) => StorageStateKind::Error,
        }
    }
}

/// A classifier for [`StorageState`] that drops the contained payload.
/// Useful for UI code that only needs to decide which screen to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageStateKind {
    Unconfigured,
    Ready,
    PermissionLost,
    Error,
}

/// What the rescan logic reads PDFs through.
///
/// `LibrarySource` wraps a platform-specific [`PdfCatalog`] so the
/// rest of the app does not have to know whether the PDFs come from a
/// `Path` (Linux) or a `tree:` URI (Android). The wrapped catalog is
/// `Send` so the source can move to a worker thread; it is not `Sync`
/// because both reference implementations have non-`Sync` interior
/// state (`RefCell` on the host, an open `ContentResolver` stream on
/// Android).
pub struct LibrarySource {
    root: LibraryRoot,
    catalog: Box<dyn PdfCatalog + Send>,
}

impl LibrarySource {
    /// Wraps an already-opened catalog under `root`.
    #[must_use]
    pub fn new(root: LibraryRoot, catalog: Box<dyn PdfCatalog + Send>) -> Self {
        Self { root, catalog }
    }

    /// The root this source was opened against.
    #[must_use]
    pub fn root(&self) -> &LibraryRoot {
        &self.root
    }
}

impl fmt::Debug for LibrarySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The wrapped catalog's `Debug` impl is not required (and the
        // trait object erases the concrete type), so we print only the
        // root. Logging the handle alone is enough to identify the
        // source in a trace.
        formatter
            .debug_struct("LibrarySource")
            .field("root", &self.root)
            .finish()
    }
}

impl PdfCatalog for LibrarySource {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        self.catalog.enumerate()
    }
}

/// The provider that knows how to ask the user for a library root and
/// how to reopen one.
///
/// The trait has one async method, [`select_root`](Self::select_root),
/// and one synchronous method, [`reopen`](Self::reopen). `select_root`
/// is async because on Android the Storage Access Framework hands the
/// result back through the Activity Result callback, which the host
/// runtime models as a future; on the host test, the future completes
/// synchronously against an in-memory queue.
///
/// The returned [`LibrarySource`] is `Send`, so the UI can move it to a
/// worker thread for the rescan without reshuffling its ownership.
pub trait LibraryRootProvider {
    /// Launches the platform's library-root picker and returns the
    /// root the user chose.
    ///
    /// On Android this is the `ACTION_OPEN_DOCUMENT_TREE` intent and
    /// the persisted permission is taken before this method returns.
    /// On the host test this is a synchronous pop off a
    /// [`FilesystemLibraryRootProvider`]'s internal queue.
    ///
    /// # Errors
    /// - [`StorageError::Cancelled`] when the user dismisses the
    ///   picker.
    /// - [`StorageError::NotADirectory`] when the selection is not a
    ///   directory.
    /// - [`StorageError::Other`] for any other platform failure.
    fn select_root(&self) -> impl Future<Output = Result<LibraryRoot, StorageError>> + Send;

    /// Re-opens a previously-selected root.
    ///
    /// On Android this re-checks the persisted permission flag and
    /// the underlying tree URI's continued existence. On the host test
    /// it stats the path the [`LibraryRoot`] was built from.
    ///
    /// # Errors
    /// - [`StorageError::PermissionLost`] when the platform can no
    ///   longer reach the root — the user has to re-select it.
    /// - [`StorageError::Other`] for any other failure.
    fn reopen(&self, root: &LibraryRoot) -> Result<LibrarySource, StorageError>;
}

// ---------------------------------------------------------------------------
// Host-only Linux implementation.
//
// `FilesystemLibraryRootProvider` is what the host tests drive. It
// adapts `notae-index-cli`'s `FilesystemCatalog` (the same catalog the
// Linux CLI uses) to the `LibraryRootProvider` trait, with a test
// queue standing in for the SAF picker.
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "android"))]
mod host {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    use notae_index_cli::catalog::{FilesystemCatalog, ValidatedRoot};

    use super::{LibraryRoot, LibraryRootId, LibrarySource, StorageError};

    /// What the next
    /// [`select_root`](super::LibraryRootProvider::select_root)
    /// call should return. Tests queue a path before calling
    /// `select_root`; an empty queue produces
    /// [`StorageError::Cancelled`].
    #[derive(Debug)]
    enum QueuedSelection {
        /// Pick a real directory.
        Directory(PathBuf),
        /// Pick a path that is not a directory, to assert
        /// [`StorageError::NotADirectory`]. The payload is the path
        /// the user "selected" so the error variant carries the
        /// context the production impl would attach; the path itself
        /// is not consumed by `select_root` because the error does not
        /// include it.
        #[allow(dead_code)]
        NotADirectory(PathBuf),
    }

    /// Internal state shared by `select_root` (which takes it mutably)
    /// and the test helpers.
    #[derive(Debug, Default)]
    struct Inner {
        next_selection: Option<QueuedSelection>,
        persisted: HashMap<LibraryRootId, LibraryRoot>,
    }

    /// The Linux-side [`LibraryRootProvider`].
    ///
    /// Drives `select_root` from an in-memory queue so the host test
    /// can exercise the state machine without a UI. The implementation
    /// wraps [`FilesystemCatalog`] from `notae-index-cli`, so the same
    /// catalog the Linux CLI ships is what the test re-uses — the host
    /// tests therefore re-hash files the same way production does, and
    /// the host test re-hashes them independently to assert the
    /// catalog's hashes are not fabricated.
    #[derive(Debug, Default)]
    pub struct FilesystemLibraryRootProvider {
        inner: Mutex<Inner>,
    }

    impl FilesystemLibraryRootProvider {
        /// Returns a provider with an empty selection queue.
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Locks the internal state. Exposed to the in-module
        /// `select_root` and to the test helpers.
        fn lock(&self) -> MutexGuard<'_, Inner> {
            self.inner.lock().expect("provider mutex poisoned")
        }

        /// Queues a directory for the next `select_root` call. The
        /// directory must exist and be readable; the provider
        /// verifies both at the moment of selection.
        pub fn queue_directory(&self, path: impl Into<PathBuf>) {
            self.lock().next_selection = Some(QueuedSelection::Directory(path.into()));
        }

        /// Queues a non-directory path (typically a single file) for
        /// the next `select_root` call. The provider rejects it with
        /// [`StorageError::NotADirectory`] instead of treating it as a
        /// usable root.
        pub fn queue_non_directory(&self, path: impl Into<PathBuf>) {
            self.lock().next_selection = Some(QueuedSelection::NotADirectory(path.into()));
        }

        /// Persists a root as if the platform had granted its
        /// permission. The provider's `reopen` will accept it
        /// afterwards; [`revoke_persisted`](Self::revoke_persisted)
        /// removes the entry so the provider reports `PermissionLost`.
        pub fn remember(&self, root: LibraryRoot) {
            self.lock().persisted.insert(root.id().clone(), root);
        }

        /// Drops the persisted entry for `root`. The next
        /// `reopen(root)` will report [`StorageError::PermissionLost`].
        pub fn revoke_persisted(&self, root: &LibraryRoot) {
            self.lock().persisted.remove(root.id());
        }
    }

    impl super::LibraryRootProvider for FilesystemLibraryRootProvider {
        async fn select_root(&self) -> Result<LibraryRoot, StorageError> {
            let queued = self.lock().next_selection.take();
            let path = match queued {
                Some(QueuedSelection::Directory(path)) => path,
                Some(QueuedSelection::NotADirectory(_)) => {
                    return Err(StorageError::NotADirectory);
                }
                None => return Err(StorageError::Cancelled),
            };
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|error| StorageError::Other(error.to_string()))?;
            if metadata.file_type().is_symlink() {
                return Err(StorageError::Other(format!(
                    "{} is a symbolic link, which is never followed",
                    path.display()
                )));
            }
            if !metadata.is_dir() {
                return Err(StorageError::NotADirectory);
            }
            let display_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            let root = LibraryRoot::new(display_name, path.to_string_lossy());
            self.remember(root.clone());
            Ok(root)
        }

        fn reopen(&self, root: &LibraryRoot) -> Result<LibrarySource, StorageError> {
            // The host provider requires that `remember` was called
            // for `root`. On a real device the persisted permission
            // flag plays the same role.
            if !self.lock().persisted.contains_key(root.id()) {
                return Err(StorageError::PermissionLost);
            }
            let path = PathBuf::from(root.handle());
            // The root boundary: refuse if the path no longer
            // resolves to a directory. `ValidatedRoot::open` does the
            // symlink-and-identity checks the CLI relies on, so the
            // host adapter inherits the production root boundary.
            //
            // A name that is gone, is not a directory, or cannot be
            // listed is collapsed to `PermissionLost`: from the UI's
            // point of view, every such failure has the same remedy
            // (re-select the folder), and mapping them all to the same
            // variant is what lets the recovery screen offer one
            // affordance rather than three.
            let validated = ValidatedRoot::open(&path).map_err(|rejection| {
                let message = rejection.to_string();
                if message.contains("No such file") || message.contains("not a directory") {
                    StorageError::PermissionLost
                } else {
                    StorageError::Other(message)
                }
            })?;
            // `FilesystemCatalog` is itself `Send` (its `RefCell` holds
            // `Vec<CatalogFailure>` which is `Send`), so the `+ Send`
            // bound the trait object carries is satisfied without an
            // adapter wrapper.
            let catalog = FilesystemCatalog::new(validated);
            Ok(LibrarySource::new(root.clone(), Box::new(catalog)))
        }
    }
}

#[cfg(not(target_os = "android"))]
pub use host::FilesystemLibraryRootProvider;

// ---------------------------------------------------------------------------
// Android implementation lives in `crate::android::saf`.
// ---------------------------------------------------------------------------