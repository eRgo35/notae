//! "Export sets": Android's save-file picker (`ACTION_CREATE_DOCUMENT`) and the write to the
//! chosen document through `ContentResolver`.
//!
//! The picker result arrives in `NotaeActivity.onActivityResult`, which hands every non-voice
//! result to `saf::nativeOnActivityResult`; that callback gives this module first look through
//! [`finish_export`]. The picker uses the reserved [`EXPORT_REQUEST_CODE`], which the folder
//! picker's counter never allocates. All raw JNI conversions stay in `saf.rs`.

#![forbid(unsafe_code)]

use std::sync::Mutex;

use android_activity::AndroidApp;
use jni::errors::Error as JniError;
use jni::objects::JObject;
use jni::{JValue, jni_sig, jni_str};

use crate::AppWindow;
use crate::backup::{EXPORT_REQUEST_CODE, MIME_TYPE};
use crate::language::pick;

const RESULT_OK: i32 = -1;
const RESULT_CANCELED: i32 = 0;

/// The export waiting for the save-file picker.
struct PendingExport {
    text: String,
    window: slint::Weak<AppWindow>,
}

static PENDING: Mutex<Option<PendingExport>> = Mutex::new(None);

/// "Could not export sets: …", for the Settings status text.
#[must_use]
pub fn export_failed_text(error: &str) -> String {
    pick(
        format!("Could not export sets: {error}"),
        format!("Nie udało się wyeksportować zestawów: {error}"),
    )
}

/// Opens the save-file picker for `text`, suggesting `file_name`. The result is reported in
/// the window's Settings status text; a cancelled picker reports nothing.
///
/// # Errors
///
/// The JNI error when the picker cannot be launched; nothing is left pending.
pub fn start_export(
    app: &AndroidApp,
    window: slint::Weak<AppWindow>,
    file_name: &str,
    text: String,
) -> Result<(), JniError> {
    set_pending(Some(PendingExport { text, window }));
    let launched = launch_create_document(app, file_name);
    if launched.is_err() {
        set_pending(None);
    }
    launched
}

fn set_pending(export: Option<PendingExport>) {
    if let Ok(mut pending) = PENDING.lock() {
        *pending = export;
    }
}

fn launch_create_document(app: &AndroidApp, file_name: &str) -> Result<(), JniError> {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        let action = env.new_string("android.intent.action.CREATE_DOCUMENT")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&action)],
        )?;
        let category = env.new_string("android.intent.category.OPENABLE")?;
        env.call_method(
            &intent,
            jni_str!("addCategory"),
            jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&category)],
        )?;
        let mime = env.new_string(MIME_TYPE)?;
        env.call_method(
            &intent,
            jni_str!("setType"),
            jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&mime)],
        )?;
        let title_key = env.new_string("android.intent.extra.TITLE")?;
        let title = env.new_string(file_name)?;
        env.call_method(
            &intent,
            jni_str!("putExtra"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&title_key), JValue::Object(&title)],
        )?;
        env.call_method(
            &activity,
            jni_str!("startActivityForResult"),
            jni_sig!("(Landroid/content/Intent;I)V"),
            &[JValue::Object(&intent), JValue::Int(EXPORT_REQUEST_CODE)],
        )?;
        Ok(())
    })
}

/// Handles the activity result if it belongs to the export picker; returns whether it did.
/// `document_uri` is the chosen document, if the result carried one. A result with no export
/// waiting (Android restarted Notae while the picker was open) writes nothing.
pub(super) fn finish_export(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    request_code: i32,
    result_code: i32,
    document_uri: Option<&str>,
) -> bool {
    if request_code != EXPORT_REQUEST_CODE {
        return false;
    }
    let Some(export) = PENDING.lock().ok().and_then(|mut pending| pending.take()) else {
        return true;
    };
    let status = match (result_code, document_uri) {
        (RESULT_CANCELED, _) => return true,
        (RESULT_OK, Some(uri)) => match write_document(env, activity, uri, &export.text) {
            Ok(()) => pick(
                "Favorites and sets exported",
                "Wyeksportowano ulubione i zestawy",
            )
            .to_owned(),
            Err(error) => {
                if env.exception_check() {
                    env.exception_clear();
                }
                export_failed_text(&error.to_string())
            }
        },
        (RESULT_OK, None) => export_failed_text(pick("no file was chosen", "nie wybrano pliku")),
        (code, _) => export_failed_text(&format!("result code {code}")),
    };
    let _ = export.window.upgrade_in_event_loop(move |window| {
        window.set_settings_backup_status(status.into());
    });
    true
}

/// Replaces the document's contents with `text`.
fn write_document(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    uri: &str,
    text: &str,
) -> Result<(), JniError> {
    let resolver = env
        .call_method(
            activity,
            jni_str!("getContentResolver"),
            jni_sig!("()Landroid/content/ContentResolver;"),
            &[],
        )?
        .l()?;
    let uri = super::saf::parse_uri(env, uri)?;
    // "wt" truncates, so writing over an older export leaves no tail of it.
    let mode = env.new_string("wt")?;
    let stream = env
        .call_method(
            &resolver,
            jni_str!("openOutputStream"),
            jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Ljava/io/OutputStream;"),
            &[JValue::Object(&uri), JValue::Object(&mode)],
        )?
        .l()?;
    if stream.is_null() {
        return Err(JniError::NullPtr("ContentResolver.openOutputStream"));
    }
    let written = env
        .byte_array_from_slice(text.as_bytes())
        .and_then(|bytes| {
            env.call_method(
                &stream,
                jni_str!("write"),
                jni_sig!("([B)V"),
                &[JValue::Object(&bytes)],
            )
            .map(|_| ())
        });
    if written.is_err() && env.exception_check() {
        env.exception_clear();
    }
    let closed = env
        .call_method(&stream, jni_str!("close"), jni_sig!("()V"), &[])
        .map(|_| ());
    written.and(closed)
}
