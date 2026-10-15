//! Storage Access Framework provider for Android.
//!
//! This module owns the Android storage/document-provider integration:
//! `ContentResolver`, tree URIs, persisted read permission, and the
//! `ACTION_OPEN_DOCUMENT_TREE` intent that opens the system file picker.
//! External PDF `ACTION_VIEW` handoff lives in `android::pdf_intent`.
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
//! The Java `NotaeActivity` delivers Activity results to this module
//! through JNI. This file also uses JNI for `ContentResolver` iteration
//! and permission persistence. The `unsafe` is contained to the JNI
//! helpers and the documented local-reference conversion; the
//! `LibraryRootProvider` impl, catalog, and persistence logic are safe
//! Rust. The same `#![allow(unsafe_code)]` policy as `android_main.rs`
//! keeps `#![forbid(unsafe_code)]` active elsewhere in the crate.
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
use std::io;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context, Poll};

use android_activity::AndroidApp;
use atomic_waker::AtomicWaker;
use jni::errors::Error as JniError;
use jni::objects::{JObject, JString};
use jni::{EnvUnowned, JValue, jni_sig, jni_str};

use notae_index::{CatalogError, PdfCatalog, PdfFile, SidecarManifest};

use crate::saf_tree::{
    DocumentRow, DocumentTreeReader, discover_sidecar_manifest, enumerate_document_tree,
};

use crate::picker_fut::{
    FLAG_GRANT_READ_URI_PERMISSION, PickerOutcome, PickerState, decode_activity_result,
    new_picker_state, poll_outcome,
};
use crate::storage::{
    LibraryCatalog, LibraryRoot, LibraryRootId, LibraryRootProvider, LibrarySource, StorageError,
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
// `NotaeActivity.onActivityResult` dispatches through this same state
// after validating and persisting the selected tree.
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

/// Android's `DocumentsContract.Document.COLUMN_DISPLAY_NAME`.
const DOCUMENT_DISPLAY_NAME: &str = "_display_name";

// ---------------------------------------------------------------------------
// JNI helpers.
//
// Every JNI call in this module goes through one of the helpers in
// this section. The `unsafe` blocks are localised; the safe wrappers
// above can be reasoned about without thinking about JNI.
// ---------------------------------------------------------------------------

/// Attaches to the Java VM and runs `f` with an `Env`. Any pending
/// Java exception is cleared when `f` returns an error, because callers
/// convert JNI failures into Rust storage or picker outcomes.
pub(super) fn with_env<F, R>(app: &AndroidApp, f: F) -> Result<R, JniError>
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
    vm.attach_current_thread(|env| {
        let result = f(env);
        if result.is_err() && env.exception_check() {
            env.exception_clear();
        }
        result
    })
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
pub(super) fn activity_as_jobject<'local>(
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
        let intent = env.new_object(jni_str!("android/content/Intent"), jni_sig!("()V"), &[])?;

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

        match read_persisted_root_jni(app) {
            Ok(Some(initial)) if !initial.tree_uri.is_empty() => {
                let key_string = env.new_string("android.provider.extra.INITIAL_URI")?;
                let value_string = env.new_string(&initial.tree_uri)?;
                let _ = env.call_method(
                    intent.as_ref(),
                    jni_str!("putExtra"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
                    &[JValue::Object(&key_string), JValue::Object(&value_string)],
                );
            }
            Ok(Some(_)) | Ok(None) => {}
            Err(_) => {
                if env.exception_check() {
                    env.exception_clear();
                }
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
                &[JValue::Object(&key_key), JValue::Object(&default_key)],
            )?;
            let value_obj = value.l()?;
            if value_obj.is_null() {
                return Ok(String::new());
            }
            // SAFETY: `value_obj` is a Java String the platform
            // returned for our `getString` call. `JString::from_raw`
            // casts the raw `jstring` as a `JString<'_>` local
            // reference bound to the current `Env`.
            let value_str =
                unsafe { JString::from_raw(env, value_obj.as_raw() as jni::sys::jstring) };
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

        // Reacquiring a revoked tree grant throws SecurityException. Propagate
        // the JNI error so `reopen` maps it to PermissionLost; `with_env`
        // clears the pending exception before detaching.
        env.call_method(
            resolver.as_ref(),
            jni_str!("takePersistableUriPermission"),
            jni_sig!("(Landroid/net/Uri;I)V"),
            &[JValue::Object(parsed_uri.as_ref()), JValue::Int(0x00000001)],
        )?;

        let root = LibraryRoot::from_parts(
            LibraryRootId::from_string(root_id_str),
            display_name,
            tree_uri.clone(),
        );
        Ok(Some(PersistedRoot { root, tree_uri }))
    })
}

fn content_resolver<'local>(
    env: &mut jni::Env<'local>,
    activity: &JObject<'local>,
) -> Result<JObject<'local>, JniError> {
    env.call_method(
        activity,
        jni_str!("getContentResolver"),
        jni_sig!("()Landroid/content/ContentResolver;"),
        &[],
    )?
    .l()
}

fn selected_tree_outcome<'local>(
    env: &mut jni::Env<'local>,
    activity: &JObject<'local>,
    tree_uri: String,
) -> Result<PickerOutcome, JniError> {
    let resolver = content_resolver(env, activity)?;
    let uri_string = env.new_string(&tree_uri)?;
    let tree_uri_object = env
        .call_static_method(
            jni_str!("android/net/Uri"),
            jni_str!("parse"),
            jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
            &[JValue::Object(&uri_string)],
        )?
        .l()?;

    env.call_method(
        &resolver,
        jni_str!("takePersistableUriPermission"),
        jni_sig!("(Landroid/net/Uri;I)V"),
        &[
            JValue::Object(&tree_uri_object),
            JValue::Int(FLAG_GRANT_READ_URI_PERMISSION),
        ],
    )?;

    let document_id = env
        .call_static_method(
            jni_str!("android/provider/DocumentsContract"),
            jni_str!("getTreeDocumentId"),
            jni_sig!("(Landroid/net/Uri;)Ljava/lang/String;"),
            &[JValue::Object(&tree_uri_object)],
        )?
        .l()?;
    let document_uri = env
        .call_static_method(
            jni_str!("android/provider/DocumentsContract"),
            jni_str!("buildDocumentUriUsingTree"),
            jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;"),
            &[
                JValue::Object(&tree_uri_object),
                JValue::Object(&document_id),
            ],
        )?
        .l()?;
    let null = JObject::null();
    let cursor = env
        .call_method(
            &resolver,
            jni_str!("query"),
            jni_sig!(
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;"
            ),
            &[
                JValue::Object(&document_uri),
                JValue::Object(&null),
                JValue::Object(&null),
                JValue::Object(&null),
                JValue::Object(&null),
            ],
        )?
        .l()?;
    if cursor.is_null() {
        return Err(JniError::NullPtr("ContentResolver.query cursor"));
    }

    let display_name_result = (|| -> Result<String, JniError> {
        let has_row = env
            .call_method(&cursor, jni_str!("moveToFirst"), jni_sig!("()Z"), &[])?
            .z()?;
        if !has_row {
            return Err(JniError::NullPtr("tree document row"));
        }
        let column_name = env.new_string(DOCUMENT_DISPLAY_NAME)?;
        let column_index = env
            .call_method(
                &cursor,
                jni_str!("getColumnIndex"),
                jni_sig!("(Ljava/lang/String;)I"),
                &[JValue::Object(&column_name)],
            )?
            .i()?;
        if column_index < 0 {
            return Err(JniError::NullPtr("tree document display-name column"));
        }
        let display_name = env
            .call_method(
                &cursor,
                jni_str!("getString"),
                jni_sig!("(I)Ljava/lang/String;"),
                &[JValue::Int(column_index)],
            )?
            .l()?;
        if display_name.is_null() {
            return Err(JniError::NullPtr("tree document display name"));
        }
        // SAFETY: Cursor.getString returned this local reference as a Java String.
        let display_name =
            unsafe { JString::from_raw(env, display_name.as_raw() as jni::sys::jstring) };
        jstring_to_string(env, display_name)
    })();
    let close_result = env
        .call_method(&cursor, jni_str!("close"), jni_sig!("()V"), &[])
        .map(|_| ());
    let display_name = display_name_result?;
    close_result?;
    if display_name.is_empty() {
        return Err(JniError::NullPtr("empty tree document display name"));
    }

    let root = LibraryRoot::new(display_name.clone(), tree_uri.clone());
    if !write_persisted_root(env, activity, &tree_uri, &display_name, root.id())? {
        return Ok(PickerOutcome::Platform {
            error: "Could not save the selected library folder.".to_owned(),
        });
    }
    Ok(PickerOutcome::Ok {
        tree_uri,
        display_name,
        root_id: root.id().clone(),
    })
}

fn write_persisted_root(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    tree_uri: &str,
    display_name: &str,
    root_id: &LibraryRootId,
) -> Result<bool, JniError> {
    let prefs_name = env.new_string(PREFS_FILE)?;
    let prefs = env.call_method(
        activity,
        jni_str!("getSharedPreferences"),
        jni_sig!("(Ljava/lang/String;I)Landroid/content/SharedPreferences;"),
        &[JValue::Object(&prefs_name), JValue::Int(0)],
    )?;
    let prefs = prefs.l()?;

    let editor = env.call_method(
        prefs.as_ref(),
        jni_str!("edit"),
        jni_sig!("()Landroid/content/SharedPreferences$Editor;"),
        &[],
    )?;
    let editor = editor.l()?;

    let mut put = |key: &str, value: &str| -> Result<(), JniError> {
        let key_obj = env.new_string(key)?;
        let value_obj = env.new_string(value)?;
        env.call_method(
            editor.as_ref(),
            jni_str!("putString"),
            jni_sig!(
                "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;"
            ),
            &[JValue::Object(&key_obj), JValue::Object(&value_obj)],
        )?;
        Ok(())
    };
    put(KEY_TREE_URI, tree_uri)?;
    put(KEY_DISPLAY_NAME, display_name)?;
    put(KEY_ROOT_ID, root_id.as_str())?;

    // Success is dispatched only after SharedPreferences confirms the write.
    env.call_method(&editor, jni_str!("commit"), jni_sig!("()Z"), &[])?
        .z()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeActivity_nativeOnActivityResult<'local>(
    mut unowned_env: EnvUnowned<'local>,
    activity: JObject<'local>,
    request_code: jni::sys::jint,
    result_code: jni::sys::jint,
    tree_uri: JString<'local>,
    grant_flags: jni::sys::jint,
) {
    unowned_env
        .with_env(|env| -> Result<(), JniError> {
            let tree_uri = if tree_uri.is_null() {
                None
            } else {
                match jstring_to_string(env, tree_uri) {
                    Ok(uri) => Some(uri),
                    Err(error) => {
                        if env.exception_check() {
                            env.exception_clear();
                        }
                        picker_state().dispatch(
                            request_code,
                            PickerOutcome::Platform {
                                error: error.to_string(),
                            },
                        );
                        return Ok(());
                    }
                }
            };

            let outcome = match decode_activity_result(result_code, tree_uri, grant_flags) {
                Ok(None) => PickerOutcome::Cancelled,
                Err(error) => PickerOutcome::Platform {
                    error: error.to_string(),
                },
                Ok(Some(tree_uri)) => match selected_tree_outcome(env, &activity, tree_uri) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        if env.exception_check() {
                            env.exception_clear();
                        }
                        PickerOutcome::Platform {
                            error: error.to_string(),
                        }
                    }
                },
            };
            picker_state().dispatch(request_code, outcome);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
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

impl SafLibraryRootProvider {
    /// The root saved by a previous run, if its persistable grant is still
    /// valid. A revoked grant or unreadable preferences yield `None`, so the
    /// caller falls back to the first-run picker.
    #[must_use]
    pub fn persisted_root(&self) -> Option<LibraryRoot> {
        match read_persisted_root_jni(&self.app) {
            Ok(Some(persisted)) => Some(persisted.root),
            Ok(None) | Err(_) => None,
        }
    }
}

impl LibraryRootProvider for SafLibraryRootProvider {
    fn select_root(&self) -> impl Future<Output = Result<LibraryRoot, StorageError>> + Send {
        SelectRootFuture::new(self.app.clone())
    }

    fn reopen(&self, _root: &LibraryRoot) -> Result<LibrarySource, StorageError> {
        let persisted = match read_persisted_root_jni(&self.app) {
            Ok(Some(persisted)) => persisted,
            Ok(None) | Err(JniError::JavaException) => {
                return Err(StorageError::PermissionLost);
            }
            Err(error) => {
                return Err(StorageError::Other(format!("read_persisted_root: {error}")));
            }
        };

        let catalog = SafPdfCatalog::new(self.app.clone(), persisted.tree_uri);
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
        let Some(request_code) = self.request_code else {
            return Poll::Ready(Err(StorageError::PermissionDenied));
        };

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
// SAF document-tree catalog.
// ---------------------------------------------------------------------------

const DOCUMENT_ID: &str = "document_id";
const DOCUMENT_MIME_TYPE: &str = "mime_type";
const INPUT_STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// The Android [`PdfCatalog`] over a persisted SAF tree URI.
pub struct SafPdfCatalog {
    app: AndroidApp,
    tree_uri: String,
}

impl SafPdfCatalog {
    /// Builds a catalog over an opaque tree URI.
    #[must_use]
    pub fn new(app: AndroidApp, tree_uri: String) -> Self {
        Self { app, tree_uri }
    }

    fn root_document_id(&self) -> Result<String, JniError> {
        with_env(&self.app, |env| {
            let tree_uri = parse_uri(env, &self.tree_uri)?;
            let document_id = env
                .call_static_method(
                    jni_str!("android/provider/DocumentsContract"),
                    jni_str!("getTreeDocumentId"),
                    jni_sig!("(Landroid/net/Uri;)Ljava/lang/String;"),
                    &[JValue::Object(&tree_uri)],
                )?
                .l()?;
            if document_id.is_null() {
                return Err(JniError::NullPtr("DocumentsContract.getTreeDocumentId"));
            }
            // SAFETY: DocumentsContract returned this local reference as a Java String.
            let document_id =
                unsafe { JString::from_raw(env, document_id.as_raw() as jni::sys::jstring) };
            jstring_to_string(env, document_id)
        })
    }

    fn children_jni(&self, parent_id: &str) -> Result<Vec<DocumentRow>, JniError> {
        let activity_ptr = self.app.activity_as_ptr();
        with_env(&self.app, move |env| {
            let activity = activity_as_jobject(env, activity_ptr)?;
            let resolver = content_resolver(env, &activity)?;
            let tree_uri = parse_uri(env, &self.tree_uri)?;
            let parent_id = env.new_string(parent_id)?;
            let children_uri = env
                .call_static_method(
                    jni_str!("android/provider/DocumentsContract"),
                    jni_str!("buildChildDocumentsUriUsingTree"),
                    jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;"),
                    &[JValue::Object(&tree_uri), JValue::Object(&parent_id)],
                )?
                .l()?;
            let null = JObject::null();
            let cursor = env
                .call_method(
                    resolver.as_ref(),
                    jni_str!("query"),
                    jni_sig!(
                        "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;"
                    ),
                    &[
                        JValue::Object(&children_uri),
                        JValue::Object(&null),
                        JValue::Object(&null),
                        JValue::Object(&null),
                        JValue::Object(&null),
                    ],
                )?
                .l()?;
            if cursor.is_null() {
                return Err(JniError::NullPtr("ContentResolver.query children cursor"));
            }

            let rows = (|| -> Result<Vec<DocumentRow>, JniError> {
                let mut rows = Vec::new();
                loop {
                    // SAF cursor values and column-name strings are local
                    // references. Bound them to one row so large directories
                    // cannot exhaust the attachment's local-reference frame.
                    let row =
                        env.with_local_frame(16, |env| -> Result<Option<DocumentRow>, JniError> {
                            let has_row = env
                                .call_method(&cursor, jni_str!("moveToNext"), jni_sig!("()Z"), &[])?
                                .z()?;
                            if !has_row {
                                return Ok(None);
                            }
                            Ok(Some(DocumentRow {
                                document_id: cursor_string(env, &cursor, DOCUMENT_ID)?,
                                display_name: cursor_string(env, &cursor, DOCUMENT_DISPLAY_NAME)?,
                                mime_type: cursor_string(env, &cursor, DOCUMENT_MIME_TYPE)?,
                            }))
                        })?;
                    match row {
                        Some(row) => rows.push(row),
                        None => break,
                    }
                }
                Ok(rows)
            })();
            if rows.is_err() && env.exception_check() {
                env.exception_clear();
            }
            let close_result = env
                .call_method(&cursor, jni_str!("close"), jni_sig!("()V"), &[])
                .map(|_| ());
            if close_result.is_err() && env.exception_check() {
                env.exception_clear();
            }
            match rows {
                Err(error) => Err(error),
                Ok(rows) => {
                    close_result?;
                    Ok(rows)
                }
            }
        })
    }

    fn read_pdf_jni(&self, document_id: &str) -> Result<Vec<u8>, JniError> {
        let activity_ptr = self.app.activity_as_ptr();
        with_env(&self.app, move |env| {
            let activity = activity_as_jobject(env, activity_ptr)?;
            let resolver = content_resolver(env, &activity)?;
            let tree_uri = parse_uri(env, &self.tree_uri)?;
            let document_id = env.new_string(document_id)?;
            let document_uri = env
                .call_static_method(
                    jni_str!("android/provider/DocumentsContract"),
                    jni_str!("buildDocumentUriUsingTree"),
                    jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;"),
                    &[JValue::Object(&tree_uri), JValue::Object(&document_id)],
                )?
                .l()?;
            let stream = env
                .call_method(
                    resolver.as_ref(),
                    jni_str!("openInputStream"),
                    jni_sig!("(Landroid/net/Uri;)Ljava/io/InputStream;"),
                    &[JValue::Object(&document_uri)],
                )?
                .l()?;
            if stream.is_null() {
                return Err(JniError::NullPtr("ContentResolver.openInputStream"));
            }
            read_input_stream(env, &stream)
        })
    }
}

impl PdfCatalog for SafPdfCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        let root_id = self
            .root_document_id()
            .map_err(|error| CatalogError::Failed(format!("cannot identify SAF root: {error}")))?;
        enumerate_document_tree(self, &root_id)
    }
}

impl LibraryCatalog for SafPdfCatalog {
    fn sidecar_manifest(&self) -> Result<Option<SidecarManifest>, CatalogError> {
        let root_id = self
            .root_document_id()
            .map_err(|error| CatalogError::Failed(format!("cannot identify SAF root: {error}")))?;
        discover_sidecar_manifest(self, &root_id)
    }
    fn extract_text_layer(
        &self,
        path: &notae_domain::RelativePdfPath,
    ) -> Result<Option<String>, crate::pdf_pages::PdfError> {
        let uri = document_content_uri(self.app.clone(), &self.tree_uri, path.as_str())
            .map_err(|error| crate::pdf_pages::PdfError::Platform(error.to_string()))?;
        let document = crate::pdf_pages::AndroidPdfDocument::open(self.app.clone(), uri)?;
        Ok(document.extract_text().map(str::to_owned))
    }
    fn render_thumbnail(
        &self,
        path: &notae_domain::RelativePdfPath,
        out: &std::path::Path,
    ) -> Result<bool, crate::pdf_pages::PdfError> {
        let uri = document_content_uri(self.app.clone(), &self.tree_uri, path.as_str())
            .map_err(|error| crate::pdf_pages::PdfError::Platform(error.to_string()))?;
        let uri = uri.as_str().to_owned();
        let out = out.to_string_lossy().into_owned();
        let activity_ptr = self.app.activity_as_ptr();
        with_env(&self.app, move |env| {
            let activity = activity_as_jobject(env, activity_ptr)?;
            let uri = env.new_string(&uri)?;
            let out = env.new_string(&out)?;
            env.call_method(
                &activity,
                jni_str!("renderNotaeThumbnail"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)Z"),
                &[
                    JValue::Object(&uri),
                    JValue::Object(&out),
                    JValue::Int(crate::thumbnails::THUMBNAIL_WIDTH_PX),
                ],
            )?
            .z()
        })
        .map_err(|error| crate::pdf_pages::PdfError::Platform(error.to_string()))
    }
}

impl DocumentTreeReader for SafPdfCatalog {
    fn children(&self, parent_id: &str) -> Result<Vec<DocumentRow>, CatalogError> {
        self.children_jni(parent_id).map_err(|error| {
            CatalogError::Failed(format!(
                "cannot list SAF children of document {parent_id}: {error}"
            ))
        })
    }

    fn read_pdf(&self, document_id: &str) -> io::Result<Vec<u8>> {
        self.read_pdf_jni(document_id)
            .map_err(|error| io::Error::other(error.to_string()))
    }
}

/// Resolves a root-relative library path to the SAF document URI shared with viewers.
pub fn document_content_uri(
    app: AndroidApp,
    tree_uri: &str,
    relative_path: &str,
) -> Result<crate::pdf_intent::ContentUri, crate::pdf_intent::ViewerError> {
    let catalog = SafPdfCatalog::new(app.clone(), tree_uri.to_owned());
    let root_id = catalog
        .root_document_id()
        .map_err(|error| crate::pdf_intent::ViewerError::Platform(error.to_string()))?;
    let document_id = crate::saf_tree::find_document_id(&catalog, &root_id, relative_path)
        .map_err(|error| crate::pdf_intent::ViewerError::Platform(error.to_string()))?;
    let value = with_env(&app, |env| {
        let tree_uri = parse_uri(env, tree_uri)?;
        let document_id = env.new_string(document_id)?;
        let document_uri = env
            .call_static_method(
                jni_str!("android/provider/DocumentsContract"),
                jni_str!("buildDocumentUriUsingTree"),
                jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;"),
                &[JValue::Object(&tree_uri), JValue::Object(&document_id)],
            )?
            .l()?;
        if document_uri.is_null() {
            return Err(JniError::NullPtr(
                "DocumentsContract.buildDocumentUriUsingTree",
            ));
        }
        let value = env
            .call_method(
                &document_uri,
                jni_str!("toString"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        if value.is_null() {
            return Err(JniError::NullPtr("Uri.toString"));
        }
        // SAFETY: Uri.toString returns a local reference to a Java String.
        let value = unsafe { JString::from_raw(env, value.as_raw() as jni::sys::jstring) };
        jstring_to_string(env, value)
    })
    .map_err(|error| crate::pdf_intent::ViewerError::Platform(error.to_string()))?;
    crate::pdf_intent::ContentUri::new(value)
}

pub(super) fn parse_uri<'local>(
    env: &mut jni::Env<'local>,
    uri: &str,
) -> Result<JObject<'local>, JniError> {
    let uri_string = env.new_string(uri)?;
    let parsed_uri = env
        .call_static_method(
            jni_str!("android/net/Uri"),
            jni_str!("parse"),
            jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
            &[JValue::Object(&uri_string)],
        )?
        .l()?;
    if parsed_uri.is_null() {
        return Err(JniError::NullPtr("Uri.parse"));
    }
    Ok(parsed_uri)
}

fn cursor_string(
    env: &mut jni::Env<'_>,
    cursor: &JObject<'_>,
    column_name: &str,
) -> Result<String, JniError> {
    let column_name_java = env.new_string(column_name)?;
    let column_index = env
        .call_method(
            cursor,
            jni_str!("getColumnIndex"),
            jni_sig!("(Ljava/lang/String;)I"),
            &[JValue::Object(&column_name_java)],
        )?
        .i()?;
    if column_index < 0 {
        return Err(JniError::ParseFailed(format!(
            "SAF cursor is missing column {column_name}"
        )));
    }
    let value = env
        .call_method(
            cursor,
            jni_str!("getString"),
            jni_sig!("(I)Ljava/lang/String;"),
            &[JValue::Int(column_index)],
        )?
        .l()?;
    if value.is_null() {
        return Err(JniError::ParseFailed(format!(
            "SAF cursor returned null for column {column_name}"
        )));
    }
    // SAFETY: Cursor.getString returned this local reference as a Java String.
    let value = unsafe { JString::from_raw(env, value.as_raw() as jni::sys::jstring) };
    jstring_to_string(env, value)
}

fn read_input_stream(env: &mut jni::Env<'_>, stream: &JObject<'_>) -> Result<Vec<u8>, JniError> {
    let read_result = (|| -> Result<Vec<u8>, JniError> {
        let buffer = env.new_byte_array(INPUT_STREAM_BUFFER_SIZE)?;
        let mut chunk = vec![0_i8; INPUT_STREAM_BUFFER_SIZE];
        let mut bytes = Vec::new();
        loop {
            let count = env
                .call_method(
                    stream,
                    jni_str!("read"),
                    jni_sig!("([B)I"),
                    &[JValue::Object(buffer.as_ref())],
                )?
                .i()?;
            if count == -1 {
                break;
            }
            if count <= 0 {
                return Err(JniError::ParseFailed(
                    "InputStream.read returned zero before EOF".to_owned(),
                ));
            }
            let count = usize::try_from(count)
                .map_err(|_| JniError::ParseFailed("invalid InputStream byte count".to_owned()))?;
            if count > chunk.len() {
                return Err(JniError::IndexOutOfBounds);
            }
            buffer.get_region(env, 0, &mut chunk[..count])?;
            bytes.extend(chunk[..count].iter().map(|byte| *byte as u8));
        }
        Ok(bytes)
    })();
    if read_result.is_err() && env.exception_check() {
        env.exception_clear();
    }
    let close_result = env
        .call_method(stream, jni_str!("close"), jni_sig!("()V"), &[])
        .map(|_| ());
    if close_result.is_err() && env.exception_check() {
        env.exception_clear();
    }
    match read_result {
        Err(error) => Err(error),
        Ok(bytes) => {
            close_result?;
            Ok(bytes)
        }
    }
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
