//! Storage Access Framework glue and the host-side equivalent.
//!
//! This module defines the contract every `LibraryRootProvider`
//! implementation satisfies. The Android implementation lives in the
//! app crate (`android::saf`); the host tests drive a directory-backed
//! provider in `tests/support/fs_catalog.rs`.
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
//! `LibraryRootProvider::select_root` is `async` because on Android the
//! Storage Access Framework returns its result through the Activity
//! Result callback, which the host-side `android-activity` runtime
//! bridges to an async future. We use native AFIT (Rust 1.75+ async
//! functions in traits), not `#[async_trait]`: the workspace is on
//! 1.98.1, the trait is consumed through `impl` (never `dyn`), and the
//! extra box per call that `#[async_trait]` would impose is irrelevant
//! given that selection happens once per app session. The returned
//! future is `Send` because the Android runtime hands the result back
//! on a different thread than the caller; host tests use
//! `pollster::block_on` to await it from the test thread.
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
//! safe to share (the SAF adapter streams documents off the
//! `ContentResolver`). Promoting one to `Sync` would force a `Mutex`
//! into a hot path that does not need it.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use notae_domain::RelativePdfPath;
use notae_index::{CatalogError, PdfCatalog, PdfFile};

use crate::pdf_pages::PdfError;

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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
/// `Ready` carries a shared, mutex-protected [`LibrarySource`]. The UI
/// reads only its ready state; the worker takes the source lock while it
/// enumerates PDFs, keeping the non-`Sync` platform catalog off the UI thread.
/// `PermissionLost` carries the original [`LibraryRoot`] so the UI can offer
/// reauthorization with the same handle. `Error` carries the [`StorageError`].
#[derive(Debug)]
pub enum StorageState {
    /// No library root is configured.
    Unconfigured,
    /// A selected root and the worker's live PDF source.
    Ready(Arc<Mutex<LibrarySource>>),
    /// The persisted permission was revoked.
    PermissionLost(LibraryRoot),
    /// Storage failed in a recoverable way.
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
    /// No library root chosen yet.
    Unconfigured,
    /// A library root is open.
    Ready,
    /// The chosen root can no longer be reached.
    PermissionLost,
    /// Storage failed.
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
    catalog: Box<dyn LibraryCatalog + Send>,
}

/// Catalog capabilities consumed by the library worker.
pub trait LibraryCatalog: PdfCatalog + Send {
    /// Extracts searchable PDF text available without local OCR execution.
    fn extract_text_layer(&self, _path: &RelativePdfPath) -> Result<Option<String>, PdfError> {
        Ok(None)
    }

    /// Renders the first page as a PNG at `out`; `Ok(false)` when the platform cannot.
    fn render_thumbnail(&self, _path: &RelativePdfPath, _out: &Path) -> Result<bool, PdfError> {
        Ok(false)
    }
}

impl LibrarySource {
    /// Wraps an already-opened catalog under `root`.
    #[must_use]
    pub fn new(root: LibraryRoot, catalog: Box<dyn LibraryCatalog + Send>) -> Self {
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

impl LibraryCatalog for LibrarySource {
    fn extract_text_layer(&self, path: &RelativePdfPath) -> Result<Option<String>, PdfError> {
        self.catalog.extract_text_layer(path)
    }
    fn render_thumbnail(&self, path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        self.catalog.render_thumbnail(path, out)
    }
}

/// A `PdfCatalog` adapter for the UI-owned shared library source.
///
/// The mutex permits the non-`Sync` platform catalog to move behind a
/// `Send` worker boundary. Only the worker enumerates through this adapter.
#[derive(Clone)]
pub struct SharedLibraryCatalog(pub Arc<Mutex<LibrarySource>>);

impl PdfCatalog for SharedLibraryCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        self.0
            .lock()
            .map_err(|_| CatalogError::Failed("library source lock poisoned".to_owned()))?
            .enumerate()
    }
}

impl LibraryCatalog for SharedLibraryCatalog {
    fn extract_text_layer(&self, path: &RelativePdfPath) -> Result<Option<String>, PdfError> {
        self.0
            .lock()
            .map_err(|_| PdfError::Platform("library source lock poisoned".to_owned()))?
            .extract_text_layer(path)
    }
    fn render_thumbnail(&self, path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        self.0
            .lock()
            .map_err(|_| PdfError::Platform("library source lock poisoned".to_owned()))?
            .render_thumbnail(path, out)
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
    /// On the host test this is a synchronous pop off the test
    /// provider's internal queue.
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
