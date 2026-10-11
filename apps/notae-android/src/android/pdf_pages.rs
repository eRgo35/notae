//! Android `PdfRenderer` backend for the host-testable PDF page adapter.
//!
//! JNI calls and the single local-reference conversion live here; page and
//! renderer resources are closed on both success and error paths.

#![allow(unsafe_code)]

use android_activity::AndroidApp;
use jni::errors::Error as JniError;
use jni::objects::{JObject, JString};
use jni::{JValue, jni_sig, jni_str};

use crate::pdf_intent::ContentUri;
use crate::pdf_pages::{PdfDocumentMetadata, PdfError, PdfPageBackend};

const API_PDF_TEXT_CONTENTS: i32 = 35;

/// `PdfRenderer` operations bound to the current Android activity.
pub struct AndroidPdfPageBackend {
    app: AndroidApp,
}

impl AndroidPdfPageBackend {
    /// Creates a renderer backend for the supplied activity.
    #[must_use]
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }
}

impl PdfPageBackend for AndroidPdfPageBackend {
    fn inspect(&mut self, uri: &ContentUri) -> Result<PdfDocumentMetadata, PdfError> {
        let activity_ptr = self.app.activity_as_ptr();
        let uri = uri.as_str().to_owned();
        let result = super::saf::with_env(&self.app, move |env| {
            let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
            let uri = super::saf::parse_uri(env, &uri)?;
            with_pdf_renderer(env, &activity, &uri, |env, renderer| {
                let page_count = env
                    .call_method(renderer, jni_str!("getPageCount"), jni_sig!("()I"), &[])?
                    .i()?;
                let page_count = u32::try_from(page_count).map_err(|_| {
                    JniError::ParseFailed("PdfRenderer returned a negative page count".to_owned())
                })?;
                let text_layer = if android_api_level(env)? >= API_PDF_TEXT_CONTENTS {
                    Some(extract_text_layer(env, renderer, page_count)?)
                } else {
                    None
                };
                Ok(PdfDocumentMetadata {
                    page_count,
                    text_layer,
                })
            })
        });
        result.map_err(|error| PdfError::Platform(error.to_string()))
    }
}

fn android_api_level(env: &mut jni::Env<'_>) -> Result<i32, JniError> {
    env.get_static_field(
        jni_str!("android/os/Build$VERSION"),
        jni_str!("SDK_INT"),
        jni_sig!("I"),
    )?
    .i()
}

fn with_pdf_renderer<R>(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    uri: &JObject<'_>,
    operation: impl FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> Result<R, JniError>,
) -> Result<R, JniError> {
    let resolver = env
        .call_method(
            activity,
            jni_str!("getContentResolver"),
            jni_sig!("()Landroid/content/ContentResolver;"),
            &[],
        )?
        .l()?;
    if resolver.is_null() {
        return Err(JniError::NullPtr("Activity.getContentResolver"));
    }
    let mode = env.new_string("r")?;
    let descriptor = env
        .call_method(
            &resolver,
            jni_str!("openFileDescriptor"),
            jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;"),
            &[JValue::Object(uri), JValue::Object(&mode)],
        )?
        .l()?;
    if descriptor.is_null() {
        return Err(JniError::NullPtr("ContentResolver.openFileDescriptor"));
    }
    let renderer = match env.new_object(
        jni_str!("android/graphics/pdf/PdfRenderer"),
        jni_sig!("(Landroid/os/ParcelFileDescriptor;)V"),
        &[JValue::Object(&descriptor)],
    ) {
        Ok(renderer) => renderer,
        Err(error) => {
            clear_pending_exception(env);
            let _ = env.call_method(&descriptor, jni_str!("close"), jni_sig!("()V"), &[]);
            clear_pending_exception(env);
            return Err(error);
        }
    };

    let result = operation(env, &renderer);
    let close_result = close_resource(env, &renderer, "PdfRenderer.close");
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            close_result?;
            Ok(value)
        }
    }
}

fn with_pdf_page<R>(
    env: &mut jni::Env<'_>,
    renderer: &JObject<'_>,
    page: u32,
    operation: impl FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> Result<R, JniError>,
) -> Result<R, JniError> {
    let page = i32::try_from(page)
        .map_err(|_| JniError::ParseFailed("PDF page index exceeds Android's range".to_owned()))?;
    let page_object = env
        .call_method(
            renderer,
            jni_str!("openPage"),
            jni_sig!("(I)Landroid/graphics/pdf/PdfRenderer$Page;"),
            &[JValue::Int(page)],
        )?
        .l()?;
    if page_object.is_null() {
        return Err(JniError::NullPtr("PdfRenderer.openPage"));
    }

    let result = operation(env, &page_object);
    let close_result = close_resource(env, &page_object, "PdfRenderer.Page.close");
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            close_result?;
            Ok(value)
        }
    }
}

fn close_resource(
    env: &mut jni::Env<'_>,
    resource: &JObject<'_>,
    label: &'static str,
) -> Result<(), JniError> {
    clear_pending_exception(env);
    let result = env
        .call_method(resource, jni_str!("close"), jni_sig!("()V"), &[])
        .map(|_| ());
    if result.is_err() {
        clear_pending_exception(env);
        Err(JniError::ParseFailed(label.to_owned()))
    } else {
        Ok(())
    }
}

fn clear_pending_exception(env: &mut jni::Env<'_>) {
    if env.exception_check() {
        env.exception_clear();
    }
}

fn extract_text_layer(
    env: &mut jni::Env<'_>,
    renderer: &JObject<'_>,
    page_count: u32,
) -> Result<String, JniError> {
    let mut text = String::new();
    for page in 0..page_count {
        let page_text = with_pdf_page(env, renderer, page, |env, page| {
            let contents = env
                .call_method(
                    page,
                    jni_str!("getTextContents"),
                    jni_sig!("()Ljava/util/List;"),
                    &[],
                )?
                .l()?;
            if contents.is_null() {
                return Err(JniError::NullPtr("PdfRenderer.Page.getTextContents"));
            }
            let count = env
                .call_method(&contents, jni_str!("size"), jni_sig!("()I"), &[])?
                .i()?;
            let mut page_text = String::new();
            for index in 0..count {
                let value = env.with_local_frame(8, |env| -> Result<Option<String>, JniError> {
                    let item = env
                        .call_method(
                            &contents,
                            jni_str!("get"),
                            jni_sig!("(I)Ljava/lang/Object;"),
                            &[JValue::Int(index)],
                        )?
                        .l()?;
                    if item.is_null() {
                        return Ok(None);
                    }
                    let value = env
                        .call_method(
                            &item,
                            jni_str!("getText"),
                            jni_sig!("()Ljava/lang/String;"),
                            &[],
                        )?
                        .l()?;
                    if value.is_null() {
                        return Ok(None);
                    }
                    // SAFETY: PdfPageTextContent.getText() returns a local Java String reference.
                    let value =
                        unsafe { JString::from_raw(env, value.as_raw() as jni::sys::jstring) };
                    value.try_to_string(env).map(Some)
                })?;
                if let Some(value) = value {
                    if !page_text.is_empty() && !value.is_empty() {
                        page_text.push(' ');
                    }
                    page_text.push_str(&value);
                }
            }
            Ok(page_text)
        })?;
        if !text.is_empty() && !page_text.is_empty() {
            text.push('\n');
        }
        text.push_str(&page_text);
    }
    Ok(text)
}
