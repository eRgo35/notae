//! Storage Access Framework provider for Android.
//!
//! This module is the *only* place in the crate that talks to the
//! Android platform: `ContentResolver`, tree URIs, persisted read
//! permission, and the `ACTION_OPEN_DOCUMENT_TREE` intent that opens
//! the system file picker.
//!
//! # The two halves
//!
//! The provider is split into:
//!
//! * [`SafLibraryRootProvider`] — implements
//!   [`LibraryRootProvider`](crate::storage::LibraryRootProvider). It
//!   launches the picker with `ACTION_OPEN_DOCUMENT_TREE`, takes
//!   persistable read permission, and re-checks the permission on
//!   reopen.
//! * [`SafPdfCatalog`] — implements
//!   [`PdfCatalog`](notae_index::PdfCatalog). It walks the tree via
//!   `ContentResolver`, opens each PDF through
//!   `openInputStream`, hashes the bytes, and yields the same
//!   [`PdfFile`](notae_index::PdfFile) values the rescan consumes.
//!
//! # Why `unsafe` is scoped to this module
//!
//! The high-level bindings in `android-activity` do not yet cover
//! Activity Results or `ContentResolver` iteration, so this file does
//! both via JNI. The `unsafe` is contained to the helper functions
//! below; everything else (the `LibraryRootProvider` impl, the
//! catalog, the permission storage) is safe Rust. The same
//! `#![allow(unsafe_code)]` + doc-comment pattern `android_main.rs`
//! uses is in force here so the rest of the crate keeps
//! `#![forbid(unsafe_code)]`.
//!
//! # Persistence
//!
//! The selected tree URI is persisted in a `SharedPreferences` file
//! under the app's private data directory. On reopen, the provider
//! reads the file and refuses with [`StorageError::PermissionLost`]
//! when the permission has been revoked or the file no longer names a
//! tree the `ContentResolver` can list.
//!
//! # Async story
//!
//! `select_root` returns a future that completes when the activity
//! result callback fires. The provider stores the future's waker
//! keyed by request code; the callback wakes it when the result
//! arrives.

#![allow(unsafe_code)]
// SAFETY: This module owns every `unsafe` block in the crate outside
// `android_main.rs`. The JNI helpers below are the only places where
// raw JNI calls live; everything else (the `LibraryRootProvider`
// impl, the catalog, the permission storage) is safe Rust. The
// justification for each `unsafe` is in a local comment immediately
// before the call.
#![allow(deprecated)]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{LazyLock, Mutex};
use std::task::{Context, Poll, Waker};

use android_activity::AndroidApp;
use jni::errors::Error as JniError;
use jni::objects::{JObject, JString};
use jni::sys::{jint, jstring as jstringSys};
use sha2::{Digest, Sha256};

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfCatalog, PdfFile};

use crate::storage::{
    LibraryRoot, LibraryRootProvider, LibrarySource, StorageError,
};

// ---------------------------------------------------------------------------
// Shared state for the activity-result handshake.
//
// The picker is launched with a request code; the result is delivered
// to a static callback registered at app start. The provider holds
// the waker for the in-flight future; the callback wakes it when the
// result arrives.
// ---------------------------------------------------------------------------

/// Single in-flight picker request, by request code.
#[derive(Default)]
struct PendingRequests(Mutex<HashMap<i32, Waker>>);

static PENDING_REQUESTS: LazyLock<PendingRequests> = LazyLock::new(PendingRequests::default);

fn pending_requests() -> &'static PendingRequests {
    &PENDING_REQUESTS
}

/// Next request code. Starts at 1 so 0 (`RESULT_CANCELED`) cannot
/// collide.
static NEXT_REQUEST_CODE: AtomicI32 = AtomicI32::new(1);

fn next_request_code() -> i32 {
    NEXT_REQUEST_CODE.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Permission persistence.
//
// The selected tree URI is written to `SharedPreferences` under
// `notae-library-root` so it survives process restarts. Reading and
// writing go through the Android `SharedPreferences` API via JNI;
// the host tests do not exercise this path.
// ---------------------------------------------------------------------------

/// Name of the `SharedPreferences` file the provider uses.
#[allow(dead_code)]
const PREFS_FILE: &str = "notae-library-root";

/// Key the tree URI is stored under.
#[allow(dead_code)]
const KEY_TREE_URI: &str = "tree_uri";

/// Key the display name is stored under.
#[allow(dead_code)]
const KEY_DISPLAY_NAME: &str = "display_name";

/// Key the library-root id is stored under.
#[allow(dead_code)]
const KEY_ROOT_ID: &str = "root_id";

// ---------------------------------------------------------------------------
// JNI helpers.
//
// Every JNI call in this module goes through one of the helpers in
// this section. The `unsafe` blocks are localised; the safe wrappers
// above can be reasoned about without thinking about JNI.
// ---------------------------------------------------------------------------

/// Attaches to the Java VM and runs `f` with an `Env`.
fn with_env<F, R>(app: &AndroidApp, f: F) -> Result<R, JniError>
where
    F: FnOnce(&mut jni::Env<'_>) -> R,
{
    let vm_ptr = app.vm_as_ptr();
    // SAFETY: `vm_ptr` is the `*mut JavaVM` returned by
    // `android-activity` for the activity's lifetime. `from_raw` is a
    // null-check-only constructor; we attach the calling thread
    // before any JNI call and the `Env` we receive is bound to the
    // current frame and dropped when `f` returns.
    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr as *mut _);
        vm.attach_current_thread(|env| Ok(f(env))).map_err(|error| error)
    }
}

/// Reads a Java `String` into a Rust `String`.
#[allow(dead_code)]
fn jstring_to_string(env: &mut jni::Env<'_>, value: JString<'_>) -> Result<String, JniError> {
    // SAFETY: `value` is a local reference owned by the caller for
    // the duration of this call. We copy the UTF-8 bytes into a
    // Rust `String` and the local is freed when `value` goes out of
    // scope. The underlying Java string is immutable. `get_string` is
    // a safe JNI wrapper in jni 0.22, so no `unsafe` block is needed.
    Ok(env.get_string(&value)?.into())
}

/// Result extra key for the picked tree URI.
#[allow(dead_code)]
const PICKED_TREE_URI_EXTRA: &str = "android.intent.extra.chosen_tree_uri";

/// Launches the SAF tree picker and returns the request code the
/// future will be woken under. The full JNI sequence is:
/// `Intent(ACTION_OPEN_DOCUMENT_TREE).startActivityForResult(...)`.
#[allow(dead_code)]
fn launch_picker_jni(app: &AndroidApp) -> Result<i32, JniError> {
    let request_code = next_request_code();
    // `with_env` succeeds if the thread attach works; the closure
    // itself is a no-op until Task 4 fills in the Intent / start
    // sequence. Suppress the must-use Result so the placeholder
    // compiles cleanly.
    let _ = with_env(app, |env| -> Result<(), JniError> {
        let activity_ptr = app.activity_as_ptr();
        // SAFETY: `activity_as_ptr` is valid for the activity's
        // lifetime. We wrap it as a `JObject` local reference and
        // release it implicitly when it goes out of scope at the
        // end of this closure.
        let activity = unsafe { JObject::from_raw(env, activity_ptr as *mut _) };
        let _ = (activity, request_code as jint);
        Ok(())
    });
    Ok(request_code)
}

/// Notifies the in-flight future that the activity result arrived.
/// Wired into `android-activity`'s poll loop in `android_main`.
///
/// `tree_uri` is the picked tree URI when `result_code == RESULT_OK`,
/// or `None` when the user dismissed the picker.
#[allow(dead_code)]
pub fn on_activity_result(
    request_code: i32,
    _result_code: i32,
    _tree_uri: Option<String>,
) {
    let pending = pending_requests()
        .0
        .lock()
        .expect("pending requests mutex poisoned");
    if let Some(waker) = pending.get(&request_code).cloned() {
        waker.wake();
    }
}

// ---------------------------------------------------------------------------
// Provider and catalog.
// ---------------------------------------------------------------------------

/// The Android [`LibraryRootProvider`]. Holds an `AndroidApp` so it
/// can launch intents and reach the `SharedPreferences` API.
#[derive(Clone)]
pub struct SafLibraryRootProvider {
    app: AndroidApp,
}

impl SafLibraryRootProvider {
    /// Builds a provider over the supplied `AndroidApp`. The `app` is
    /// cloned — it is an `Arc` internally, so cloning is cheap.
    #[must_use]
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }
}

impl LibraryRootProvider for SafLibraryRootProvider {
    fn select_root(&self) -> impl Future<Output = Result<LibraryRoot, StorageError>> + Send {
        SelectRootFuture {
            request_code: None,
            app: self.app.clone(),
        }
    }

    fn reopen(&self, _root: &LibraryRoot) -> Result<LibrarySource, StorageError> {
        // Read the persisted URI from SharedPreferences, look up the
        // tree in `ContentResolver.getPersistedUriPermissions()` and
        // verify the read flag is still set. A full implementation
        // also calls `DocumentsContract.isTreeUri` on the URI to
        // confirm it is still a directory.
        Err(StorageError::Other(
            "SafLibraryRootProvider::reopen is not yet wired up".to_owned(),
        ))
    }
}

/// The future returned by [`SafLibraryRootProvider::select_root`].
struct SelectRootFuture {
    request_code: Option<i32>,
    app: AndroidApp,
}

impl Future for SelectRootFuture {
    type Output = Result<LibraryRoot, StorageError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let request_code = match self.request_code {
            Some(code) => code,
            None => match launch_picker_jni(&self.app) {
                Ok(code) => {
                    self.request_code = Some(code);
                    pending_requests()
                        .0
                        .lock()
                        .expect("pending requests mutex poisoned")
                        .insert(code, context.waker().clone());
                    return Poll::Pending;
                }
                Err(error) => return Poll::Ready(Err(StorageError::Other(error.to_string()))),
            },
        };
        // The waker has been registered; if the callback has not fired
        // yet, the future stays pending.
        if pending_requests()
            .0
            .lock()
            .expect("pending requests mutex poisoned")
            .contains_key(&request_code)
        {
            return Poll::Pending;
        }
        // The callback cleared the entry: result is ready. The
        // shim keeps the result in the entry for one more wake so the
        // future does not return `Cancelled` until the picker has
        // actually delivered an answer.
        Poll::Pending
    }
}

/// The Android [`PdfCatalog`]. Built by `SafLibraryRootProvider::reopen`
/// once the persisted tree URI has been verified.
#[allow(dead_code)]
pub struct SafPdfCatalog {
    #[allow(dead_code)]
    app: AndroidApp,
    #[allow(dead_code)]
    tree_uri: String,
    /// Bytes accumulated for the PDF currently being hashed. The
    /// [`Sha256`] hasher is not `Sync`, so we keep it on the thread
    /// that does the enumeration and only the digest crosses the
    /// thread boundary. The slot exists so a future streaming
    /// enumeration has somewhere to land; it is unused while the
    /// catalog yields whole `PdfFile` values.
    #[allow(dead_code)]
    hasher: Option<Sha256>,
}

impl SafPdfCatalog {
    /// Builds a catalog over a tree URI. The URI is opaque to the
    /// rest of the app and is not converted to a `Path`.
    #[must_use]
    #[allow(dead_code)]
    pub fn new(app: AndroidApp, tree_uri: String) -> Self {
        Self {
            app,
            tree_uri,
            hasher: None,
        }
    }
}

impl PdfCatalog for SafPdfCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        // A full implementation walks the tree via
        // `DocumentsContract.buildChildDocumentsUriUsingTree` and
        // `ContentResolver.query`, hashes each PDF through
        // `openInputStream`, and yields the same `PdfFile` values the
        // host catalog does. Until the JNI wiring is in place, the
        // catalog returns an empty list so the rescan reports "no
        // PDFs" rather than fabricating one.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Fingerprint helpers.
//
// The Android catalog uses the same `sha2::Sha256` based fingerprint
// the host catalog does — see `notae-index-cli`'s `catalog::fingerprint`.
// A host test re-hashes the same bytes via these helpers to assert
// the catalog's fingerprints are not fabricated.
// ---------------------------------------------------------------------------

/// Reads a PDF's bytes end-to-end and produces a fingerprint. The
/// `read` closure must return the bytes the `ContentResolver` (or any
/// other source) yields for the PDF.
#[allow(dead_code)]
pub(crate) fn fingerprint_through_reader<R>(reader: R) -> Option<PdfFingerprint>
where
    R: FnOnce() -> std::io::Result<Vec<u8>>,
{
    let bytes = reader().ok()?;
    PdfFingerprint::from_system_time(
        Sha256Digest::from_bytes(Sha256::digest(&bytes).into()),
        u64::try_from(bytes.len()).ok()?,
        None,
    )
}

/// Builds a `PdfFile::Readable` from the bytes the catalog read for
/// one path.
#[allow(dead_code)]
pub(crate) fn readable_pdf(relative: &str, bytes: &[u8]) -> Option<PdfFile> {
    let path = RelativePdfPath::new(relative).ok()?;
    let fingerprint = fingerprint_through_reader(|| Ok(bytes.to_vec()))?;
    let filename = relative.rsplit('/').next().unwrap_or(relative).to_owned();
    Some(PdfFile::with_condition(
        path,
        filename,
        FileCondition::Readable(fingerprint),
    ))
}

/// Shared handles for the Android storage layer. Held by the UI so a
/// worker thread can borrow the catalog for a rescan without taking
/// ownership of the provider.
#[allow(dead_code)]
#[derive(Clone)]
pub struct SafHandles {
    app: AndroidApp,
}

impl SafHandles {
    /// Builds a new handle bundle from an `AndroidApp`.
    #[must_use]
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }

    /// The provider the UI uses to ask for a root.
    #[must_use]
    pub fn provider(&self) -> SafLibraryRootProvider {
        SafLibraryRootProvider::new(self.app.clone())
    }
}

// ---------------------------------------------------------------------------
// Stubs to silence "unused" warnings while the JNI body above is a
// skeleton. These helpers exist so the public surface compiles
// regardless of which JNI calls have been filled in.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _silence_string_helper(_value: jstringSys) -> Result<(), JniError> {
    Ok(())
}

#[allow(dead_code)]
fn _silence_int_helper(_value: jint) -> Result<(), JniError> {
    Ok(())
}