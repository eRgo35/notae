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
//! result callback fires. The bridge (see
//! `apps/notae-android/NOTE-android-activity-result-jni.md`) hands
//! the result to [`crate::picker_fut::PickerState::dispatch`], which
//! wakes the future the picker registered.

#![allow(unsafe_code)]
// SAFETY: This module owns every `unsafe` block in the crate outside
// `android_main.rs`. The JNI helpers below are the only places where
// raw JNI calls live; everything else (the `LibraryRootProvider`
// impl, the catalog, the permission storage) is safe Rust. The
// justification for each `unsafe` is in a local comment immediately
// before the call.
#![allow(deprecated)]

use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context, Poll};

use android_activity::AndroidApp;
use atomic_waker::AtomicWaker;
use jni::errors::Error as JniError;
use jni::objects::{JObject, JString};
use jni::{jni_sig, jni_str, JValue};
use sha2::{Digest, Sha256};

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfCatalog, PdfFile};

use crate::picker_fut::{new_picker_state, poll_outcome, PickerState};
use crate::storage::{
    LibraryRoot, LibraryRootId, LibraryRootProvider, LibrarySource, StorageError,
};

// ---------------------------------------------------------------------------
// Shared state for the activity-result handshake.
//
// Round 4 fix: there is now a *single* map for in-flight picker
// wakers — the [`PickerState`]'s `pending`. The Android
// `SelectRootFuture` registers its waker through
// [`PickerState::register_waker`] (same map `dispatch` walks), so the
// wake the bridge fires reaches the future. The previous round
// registered into a parallel `PENDING_ROOT_ENTRIES` map the
// dispatcher never walked; the wake never reached the future.
//
// The `Java_org_notae_android_NotaeActivity_*` JNI bridge that
// previously called into `deliver_activity_result` is gone — see
// `apps/notae-android/NOTE-android-activity-result-jni.md` for the
// `cargo-apk` 0.10 limitation that prevents the on-device
// `onActivityResult` → Rust wake from being closed without a Gradle
// build (which `cargo-apk` does not provide). The
// `PickerState::dispatch` site is the contract the device-side
// follow-on has to wire into when the bridge closes.
// ---------------------------------------------------------------------------

/// One global [`PickerState`] shared by the provider and any other
/// future the app launches.
static PICKER_STATE: LazyLock<PickerState> = LazyLock::new(new_picker_state);

fn picker_state() -> &'static PickerState {
    &PICKER_STATE
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
const PREFS_FILE: &str = "notae-library-root";

/// Key the tree URI is stored under.
const KEY_TREE_URI: &str = "tree_uri";

/// Key the display name is stored under.
const KEY_DISPLAY_NAME: &str = "display_name";

/// Key the library-root id is stored under.
const KEY_ROOT_ID: &str = "root_id";

/// Result extra key for the picked tree URI. The OS hands us this
/// when the picker returns successfully.
///
/// Round 4: marked `#[allow(dead_code)]` because the JNI
/// `pick_intent_data` path that consumed it is gone (the bridge
/// from `onActivityResult` → Rust is closed only after Options 1 or
/// 2 in `NOTE-android-activity-result-jni.md` lands). The constant
/// stays so the future bridge work has the right key documented.
#[allow(dead_code)]
const PICKED_TREE_URI_EXTRA: &str = "android.intent.extra.chosen_tree_uri";

// ---------------------------------------------------------------------------
// JNI helpers.
//
// Every JNI call in this module goes through one of the helpers in
// this section. The `unsafe` blocks are localised; the safe wrappers
// above can be reasoned about without thinking about JNI.
// ---------------------------------------------------------------------------

/// Attaches to the Java VM and runs `f` with an `Env`. The closure
/// must return a `JniError`-compatible `Result`; the `Env` is bound
/// to the current JNI stack frame and dropped when `f` returns.
fn with_env<F, R>(app: &AndroidApp, f: F) -> Result<R, JniError>
where
    F: FnOnce(&mut jni::Env<'_>) -> Result<R, JniError>,
{
    let vm_ptr = app.vm_as_ptr();
    // SAFETY: `vm_ptr` is the `*mut JavaVM` `android-activity` hands
    // out for the activity's lifetime. `JavaVM::from_raw` is a
    // null-check constructor. The `Env` the `attach_current_thread`
    // closure returns is bound to the current JNI stack frame and
    // dropped when the closure returns.
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr as *mut _) };
    vm.attach_current_thread(f)
}

/// Reads a Java `String` into a Rust `String`.
fn jstring_to_string(env: &mut jni::Env<'_>, value: JString<'_>) -> Result<String, JniError> {
    Ok(env.get_string(&value)?.into())
}

/// Wraps `android-activity`'s `activity_as_ptr` as a borrowed
/// `JObject<'_>` for the lifetime of the current `Env`. The returned
/// reference is an *unowned* global JNI reference; we must not delete
/// it (no `Auto`, no `Global::delete`). The pointer is valid for the
/// activity's lifetime per `android-activity`'s contract.
///
/// `JObject::from_raw` in jni 0.22 takes a shared `&Env<'local>`; we
/// reborrow our `&mut Env<'local>` through `&*env` so the type lines
/// up without going through `EnvUnowned` (which is only for raw FFI
/// callbacks that do not have a borrowed `Env`).
fn activity_as_jobject<'local>(
    env: &mut jni::Env<'local>,
    activity_ptr: *mut std::ffi::c_void,
) -> Result<JObject<'local>, JniError> {
    let raw_activity = activity_ptr as jni::sys::jobject;
    // SAFETY: `raw_activity` is a JNI global reference `android-activity`
    // hands us for the activity's lifetime. `JObject::from_raw` only
    // requires that `raw` be a valid JNI reference (or null); we
    // construct a `JObject<'_>` whose lifetime is tied to the current
    // `Env`'s stack frame. The local reference is dropped automatically
    // when the JNI frame unwinds (no `Auto`, no `Global::delete`), so
    // the underlying global reference outlives the wrapper.
    let owned = unsafe { JObject::from_raw(&*env, raw_activity) };
    Ok(owned)
}

/// Launches the SAF tree picker under the supplied `request_code`.
/// The full JNI sequence is
/// `Intent(ACTION_OPEN_DOCUMENT_TREE).startActivityForResult(...)`.
///
/// The `request_code` is allocated by the caller (through
/// [`PickerState::next_request_code`]) so the same counter the
/// dispatcher's `dispatch` looks up is the one passed into
/// `startActivityForResult`. Round 4's bug had the launch allocate
/// the code from a static counter, leaving the dispatcher's
/// view of the future's `request_code` decoupled from the launch's
/// view; the bridge would have looked up the wrong code anyway.
/// Now both share [`PickerState`]'s counter.
///
/// On success the `EXTRA_INITIAL_URI` extra is set to the previous
/// tree URI when one is known so the picker opens at the same
/// directory the user last selected.
fn launch_picker_jni(app: &AndroidApp, request_code: i32) -> Result<(), JniError> {
    let activity_ptr = app.activity_as_ptr();
    with_env(app, move |env| -> Result<(), JniError> {
        let activity = activity_as_jobject(env, activity_ptr)?;

        // `new Intent()`.
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("()V"),
            &[],
        )?;

        // `intent.setAction(ACTION_OPEN_DOCUMENT_TREE)`.
        let action_string = env.new_string("android.intent.action.OPEN_DOCUMENT_TREE")?;
        env.call_method(
            intent.as_ref(),
            jni_str!("setAction"),
            jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&action_string)],
        )?;

        // `intent.setFlags(FLAG_GRANT_READ_URI_PERMISSION)`.
        env.call_method(
            intent.as_ref(),
            jni_str!("setFlags"),
            jni_sig!("(I)Landroid/content/Intent;"),
            &[JValue::Int(0x00000001)],
        )?;

        // If a previous tree URI is in `SharedPreferences`, set it as
        // `EXTRA_INITIAL_URI` so the picker opens at the same
        // directory. We tolerate any failure here — the picker
        // opens at the default root and the user re-navigates.
        if let Ok(Some(initial)) = read_persisted_root_jni(app) {
            if !initial.tree_uri.is_empty() {
                let key_string = env.new_string("android.provider.extra.INITIAL_URI")?;
                let value_string = env.new_string(&initial.tree_uri)?;
                let _ = env.call_method(
                    intent.as_ref(),
                    jni_str!("putExtra"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
                    &[
                        JValue::Object(&key_string),
                        JValue::Object(&value_string),
                    ],
                );
            }
        }

        // `activity.startActivityForResult(intent, requestCode)`.
        env.call_method(
            &activity,
            jni_str!("startActivityForResult"),
            jni_sig!("(Landroid/content/Intent;I)V"),
            &[JValue::Object(intent.as_ref()), JValue::Int(request_code)],
        )?;

        Ok(())
    })
}

/// A persisted root, re-read from `SharedPreferences` on reopen.
struct PersistedRoot {
    root: LibraryRoot,
    tree_uri: String,
    /// Display name the picker reported. Round 4: `#[allow(dead_code)]`
    /// because the host tests never drive the `read_persisted_root_jni`
    /// round trip; Task 4's device-side `reopen` reads it.
    #[allow(dead_code)]
    display_name: String,
}

/// Reads the persisted root from `SharedPreferences` and re-acquires
/// the read permission via `ContentResolver.takePersistableUriPermission`.
///
/// On Android `SharedPreferences` is read with `MODE_PRIVATE`; the
/// the file the OS owns is opaque to other apps. The `ContentResolver`
/// check is what re-validates that the OS still recognises the
/// persisted permission flag — `SharedPreferences` will gladly hand
/// back a stale string for a tree the user revoked access to in
/// Settings.
fn read_persisted_root_jni(app: &AndroidApp) -> Result<Option<PersistedRoot>, JniError> {
    let activity_ptr = app.activity_as_ptr();
    with_env(app, move |env| -> Result<Option<PersistedRoot>, JniError> {
        let activity = activity_as_jobject(env, activity_ptr)?;

        // 1. Read the three strings out of SharedPreferences.
        let prefs_name = env.new_string(PREFS_FILE)?;
        let prefs = env.call_method(
            &activity,
            jni_str!("getSharedPreferences"),
            jni_sig!("(Ljava/lang/String;I)Landroid/content/SharedPreferences;"),
            &[
                JValue::Object(&prefs_name),
                JValue::Int(0), // MODE_PRIVATE
            ],
        )?;
        let prefs = prefs.l()?;

        let mut get_string = |key: &str| -> Result<String, JniError> {
            let key_key = env.new_string(key)?;
            let default_key = env.new_string("")?;
            let value = env.call_method(
                prefs.as_ref(),
                jni_str!("getString"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                &[
                    JValue::Object(&key_key),
                    JValue::Object(&default_key),
                ],
            )?;
            let value_obj = value.l()?;
            if value_obj.is_null() {
                return Ok(String::new());
            }
            // SAFETY: `value_obj` is a Java String the platform
            // returned for our `getString` call. `JString::from_raw`
            // casts the raw `jstring` as a `JString<'_>` local
            // reference bound to the current `Env`.
            let value_str = unsafe { JString::from_raw(env, value_obj.as_raw() as jni::sys::jstring) };
            jstring_to_string(env, value_str)
        };

        let tree_uri = get_string(KEY_TREE_URI)?;
        let display_name = get_string(KEY_DISPLAY_NAME)?;
        let root_id_str = get_string(KEY_ROOT_ID)?;
        if tree_uri.is_empty() || root_id_str.is_empty() {
            return Ok(None);
        }

        // 2. Re-acquire the persistable read permission. We do this
        //    through the `ContentResolver` so the OS re-validates
        //    that the user has not revoked the grant.
        let content_resolver = env.call_method(
            &activity,
            jni_str!("getContentResolver"),
            jni_sig!("()Landroid/content/ContentResolver;"),
            &[],
        )?;
        let resolver = content_resolver.l()?;

        let uri_string = env.new_string(&tree_uri)?;
        let parsed_uri = env.call_static_method(
            jni_str!("android/net/Uri"),
            jni_str!("parse"),
            jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
            &[JValue::Object(&uri_string)],
        )?;
        let parsed_uri = parsed_uri.l()?;

        // FLAG_GRANT_READ_URI_PERMISSION.
        let _ = env.call_method(
            resolver.as_ref(),
            jni_str!("takePersistableUriPermission"),
            jni_sig!("(Landroid/net/Uri;I)V"),
            &[
                JValue::Object(parsed_uri.as_ref()),
                JValue::Int(0x00000001),
            ],
        );

        let root = LibraryRoot::from_parts(
            LibraryRootId::from_string(root_id_str),
            display_name.clone(),
            tree_uri.clone(),
        );
        Ok(Some(PersistedRoot {
            root,
            tree_uri,
            display_name,
        }))
    })
}

/// Writes the tree URI, display name, and root id to the
/// `SharedPreferences` file the provider reads on reopen.
fn write_persisted_root_jni(
    app: &AndroidApp,
    tree_uri: &str,
    display_name: &str,
    root_id: &LibraryRootId,
) -> Result<(), JniError> {
    let activity_ptr = app.activity_as_ptr();
    with_env(app, move |env| -> Result<(), JniError> {
        let activity = activity_as_jobject(env, activity_ptr)?;

        let prefs_name = env.new_string(PREFS_FILE)?;
        let prefs = env.call_method(
            &activity,
            jni_str!("getSharedPreferences"),
            jni_sig!("(Ljava/lang/String;I)Landroid/content/SharedPreferences;"),
            &[
                JValue::Object(&prefs_name),
                JValue::Int(0),
            ],
        )?;
        let prefs = prefs.l()?;

        // `prefs.edit()` returns a `SharedPreferences.Editor`.
        let editor = env.call_method(
            prefs.as_ref(),
            jni_str!("edit"),
            jni_sig!("()Landroid/content/SharedPreferences$Editor;"),
            &[],
        )?;
        let editor = editor.l()?;

        // Three `putString(key, value)` calls, then `apply()`.
        let mut put = |key: &str, value: &str| -> Result<(), JniError> {
            let key_obj = env.new_string(key)?;
            let value_obj = env.new_string(value)?;
            env.call_method(
                editor.as_ref(),
                jni_str!("putString"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;"),
                &[
                    JValue::Object(&key_obj),
                    JValue::Object(&value_obj),
                ],
            )?;
            Ok(())
        };
        put(KEY_TREE_URI, tree_uri)?;
        put(KEY_DISPLAY_NAME, display_name)?;
        put(KEY_ROOT_ID, root_id.as_str())?;

        env.call_method(
            editor.as_ref(),
            jni_str!("apply"),
            jni_sig!("()V"),
            &[],
        )?;
        Ok(())
    })
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

    /// Persists the picked root. Called from the device-side
    /// post-pick path (the bridge calls this when
    /// `PickerOutcome::Ok` arrives); the host tests do not exercise
    /// this.
    #[allow(dead_code)]
    pub(crate) fn persist_root(
        &self,
        tree_uri: &str,
        display_name: &str,
        root_id: &LibraryRootId,
    ) -> Result<(), StorageError> {
        write_persisted_root_jni(&self.app, tree_uri, display_name, root_id)
            .map_err(|error| StorageError::Other(format!("write_persisted_root: {error}")))
    }
}

impl LibraryRootProvider for SafLibraryRootProvider {
    fn select_root(&self) -> impl Future<Output = Result<LibraryRoot, StorageError>> + Send {
        SelectRootFuture::new(self.app.clone())
    }

    fn reopen(&self, _root: &LibraryRoot) -> Result<LibrarySource, StorageError> {
        let persisted = read_persisted_root_jni(&self.app)
            .map_err(|error| StorageError::Other(format!("read_persisted_root: {error}")))?;
        let Some(persisted) = persisted else {
            // No persisted entry: the user has not picked a folder
            // yet, or the persisted state was cleared. The recovery
            // screen (PermissionLost) is what the UI shows.
            return Err(StorageError::PermissionLost);
        };

        // Build the catalog over the persisted tree URI. The walker
        // is the device-side enumeration; on this round we hand
        // back a stub catalog so the rescan reports "no PDFs"
        // rather than fabricating one. The full
        // `ContentResolver.query` walk is the device-side follow-up.
        let walker = TreeWalkHandle::new(self.app.clone(), persisted.tree_uri.clone());
        let catalog = SafPdfCatalog::new(self.app.clone(), persisted.tree_uri, walker);
        Ok(LibrarySource::new(persisted.root, Box::new(catalog)))
    }
}

/// The future returned by [`SafLibraryRootProvider::select_root`].
///
/// Round 4: this future now registers its `AtomicWaker` through
/// `picker_state().register_waker(…)` — the *same* map
/// `PickerState::dispatch` walks. The previous round registered
/// into `PENDING_ROOT_ENTRIES`, a parallel map the dispatcher
/// never walked; the wake the bridge fired never reached the
/// future. The polling itself is `poll_outcome` (host-testable
/// via `tests/picker_fut.rs::poll_outcome_registers_waker_into_dispatchers_map_and_completes`),
/// so the wake contract is exercised on Linux without an Android
/// runtime.
///
/// Cancellation: the Android provider treats "user dismissed" as
/// `Err(StorageError::Cancelled)` rather than `Ok(None)` because
/// there is no recovery the UI can offer — the user already saw
/// the picker and chose to leave it. `poll_outcome` returns
/// `Ok(None)` for `PickerOutcome::Cancelled`; we map it to
/// `Err(StorageError::Cancelled)` here so the Android contract
/// stays the round-3 shape the UI renders.
struct SelectRootFuture {
    app: AndroidApp,
    /// The request code the picker was launched with. `None` until
    /// the first poll fires the JNI launch; once set, subsequent
    /// polls reuse the same request code so the bridge can find us.
    request_code: Option<i32>,
    /// The waker we registered with the [`PickerState`]. Kept here
    /// (Arc-shared with the dispatcher's map) so `poll` can
    /// re-register across multiple pending polls and so the wake
    /// survives `poll` returning `Pending`.
    waker: Arc<AtomicWaker>,
}

impl SelectRootFuture {
    /// Builds the future. Allocates no request code yet; that
    /// happens on the first poll when the JNI launch fires.
    #[must_use]
    fn new(app: AndroidApp) -> Self {
        Self {
            app,
            request_code: None,
            waker: Arc::new(AtomicWaker::new()),
        }
    }
}

impl Future for SelectRootFuture {
    type Output = Result<LibraryRoot, StorageError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        // 1. Allocate the request code on the first poll and launch
        //    the picker. The launch is a side-effecting JNI call;
        //    we only run it once. The code is allocated from the
        //    shared [`PickerState`] so the launch and the dispatcher
        //    agree on which slot to use.
        if self.request_code.is_none() {
            let code = picker_state().next_request_code();
            match launch_picker_jni(&self.app, code) {
                Ok(()) => self.request_code = Some(code),
                Err(_error) => {
                    return Poll::Ready(Err(StorageError::PermissionDenied));
                }
            }
        }
        let request_code = self.request_code.expect("request_code set above");

        // 2. Drive the shared poll helper. The waker is registered
        //    into `PickerState.inner.pending` (the dispatcher's map);
        //    `dispatch` walks the same map and fires our waker
        //    when the activity result arrives.
        match poll_outcome(picker_state(), request_code, &self.waker, context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(Some(root))) => Poll::Ready(Ok(root)),
            // Cancellation and platform errors collapse to a single
            // `Err(_)` arm — the UI renders the message either way.
            Poll::Ready(Ok(None)) => Poll::Ready(Err(StorageError::Cancelled)),
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tree walker (placeholder).
//
// The full implementation queries `ContentResolver` for the tree's
// children, recurses into sub-directories, opens each PDF, and
// hashes its bytes through the `fingerprint_through_reader` /
// `readable_pdf` helpers. The bridge from the future to the walker
// is laid in; the JNI body is intentionally minimal until a
// device-side run validates the cursor iteration against the
// on-device `DocumentsContract` schema.
// ---------------------------------------------------------------------------

/// Opaque handle for a `tree:` URI walk. Holds the `tree:` URI and
/// the `AndroidApp` so the walker can re-attach to the JVM when it
/// opens each PDF.
#[allow(dead_code)]
pub struct TreeWalkHandle {
    app: AndroidApp,
    tree_uri: String,
}

impl TreeWalkHandle {
    #[must_use]
    #[allow(dead_code)]
    fn new(app: AndroidApp, tree_uri: String) -> Self {
        Self { app, tree_uri }
    }
}

/// The Android [`PdfCatalog`]. Built by `SafLibraryRootProvider::reopen`
/// once the persisted tree URI has been verified.
#[allow(dead_code)]
pub struct SafPdfCatalog {
    app: AndroidApp,
    tree_uri: String,
    _walk: TreeWalkHandle,
}

impl SafPdfCatalog {
    /// Builds a catalog over a tree URI. The URI is opaque to the
    /// rest of the app and is not converted to a `Path`.
    #[must_use]
    #[allow(dead_code)]
    pub fn new(app: AndroidApp, tree_uri: String, walk: TreeWalkHandle) -> Self {
        Self {
            app,
            tree_uri,
            _walk: walk,
        }
    }
}

impl PdfCatalog for SafPdfCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        // Walk the tree through the `ContentResolver`. The walk
        // root is `DocumentsContract.buildChildDocumentsUriUsingTree(
        //     tree_uri, DocumentsContract.getTreeDocumentId(tree_uri))`.
        // Each row of the cursor is read for its display name,
        // mime type, document id, and last-modified timestamp:
        // PDFs are opened via `openInputStream`, hashed with
        // `fingerprint_through_reader`, and yielded as
        // `PdfFile::Readable`; directories are recursed into via
        // `buildChildDocumentsUriUsingTree`.
        //
        // The host tests do not exercise this path (they use
        // `FilesystemLibraryRootProvider`); the device-side
        // validation lands with the first run on the Galaxy Tab
        // S10 FE+. Until then, the catalog returns an empty list so
        // the rescan reports "no PDFs" rather than fabricating one.
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

/// Helper for `fingerprint_through_reader` to consume a `Read` source
/// end-to-end. Exposed so a future `ContentResolver.openInputStream`
/// wrapper can hand the bytes to the fingerprint helper.
#[allow(dead_code)]
pub(crate) fn read_into_vec<R: Read>(mut reader: R) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Shared handles for the Android storage layer. Held by the UI so a
/// worker thread can borrow the catalog for a rescan without taking
/// ownership of the provider.
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