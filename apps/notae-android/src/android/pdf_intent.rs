use android_activity::AndroidApp;
use jni::errors::Error as JniError;
use jni::objects::JObject;
use jni::{JValue, jni_sig, jni_str};

use notae_app_core::pdf_intent::{
    ContentUri, LaunchResult, PdfIntentSpec, ViewerError, launch_result, pdf_intent_spec,
};

const SAMSUNG_NOTES_PACKAGE: &str = "com.samsung.android.app.notes";
const MATCH_DEFAULT_ONLY: i32 = 0x0001_0000;

/// Launches SAF-backed PDFs in Samsung Notes or Android's system chooser.
pub struct PdfLauncher {
    app: AndroidApp,
}

impl PdfLauncher {
    /// Creates a launcher bound to the current NativeActivity.
    #[must_use]
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }

    /// Opens a PDF with read-only temporary access to its content URI.
    pub fn open(&self, uri: ContentUri) -> Result<LaunchResult, ViewerError> {
        let spec = pdf_intent_spec(uri);
        let uri = spec.uri().as_str().to_owned();
        let activity_ptr = self.app.activity_as_ptr();
        let result = super::saf::with_env(&self.app, move |env| {
            let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
            let content_uri = super::saf::parse_uri(env, &uri)?;
            let package_manager = env
                .call_method(
                    &activity,
                    jni_str!("getPackageManager"),
                    jni_sig!("()Landroid/content/pm/PackageManager;"),
                    &[],
                )?
                .l()?;
            if package_manager.is_null() {
                return Err(JniError::NullPtr("Activity.getPackageManager"));
            }

            let pdf_intent = pdf_view_intent(env, &content_uri, &spec)?;
            let samsung_intent = env.new_object(
                jni_str!("android/content/Intent"),
                jni_sig!("(Landroid/content/Intent;)V"),
                &[JValue::Object(&pdf_intent)],
            )?;
            let samsung_package = env.new_string(SAMSUNG_NOTES_PACKAGE)?;
            env.call_method(
                &samsung_intent,
                jni_str!("setPackage"),
                jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
                &[JValue::Object(&samsung_package)],
            )?;
            let samsung_component = env
                .call_method(
                    &samsung_intent,
                    jni_str!("resolveActivity"),
                    jni_sig!(
                        "(Landroid/content/pm/PackageManager;)Landroid/content/ComponentName;"
                    ),
                    &[JValue::Object(&package_manager)],
                )?
                .l()?;
            if !samsung_component.is_null() {
                start_activity(env, &activity, &samsung_intent)?;
                return Ok(Some(LaunchResult::SamsungNotes));
            }

            let compatible_activities = env
                .call_method(
                    &package_manager,
                    jni_str!("queryIntentActivities"),
                    jni_sig!(
                        "(Landroid/content/Intent;I)Ljava/util/List;"
                    ),
                    &[
                        JValue::Object(&pdf_intent),
                        JValue::Int(MATCH_DEFAULT_ONLY),
                    ],
                )?
                .l()?;
            if compatible_activities.is_null() {
                return Err(JniError::NullPtr("PackageManager.queryIntentActivities"));
            }
            let compatible_count = env
                .call_method(
                    &compatible_activities,
                    jni_str!("size"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            match launch_result(false, usize::try_from(compatible_count).unwrap_or(0)) {
                Ok(LaunchResult::SystemChooser) => {
                    let title = env.new_string("Open PDF with")?;
                    let chooser = env
                        .call_static_method(
                            jni_str!("android/content/Intent"),
                            jni_str!("createChooser"),
                            jni_sig!(
                                "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;"
                            ),
                            &[JValue::Object(&pdf_intent), JValue::Object(&title)],
                        )?
                        .l()?;
                    if chooser.is_null() {
                        return Err(JniError::NullPtr("Intent.createChooser"));
                    }
                    env.call_method(
                        &chooser,
                        jni_str!("setFlags"),
                        jni_sig!("(I)Landroid/content/Intent;"),
                        &[JValue::Int(spec.grant_flags())],
                    )?;
                    start_activity(env, &activity, &chooser)?;
                    Ok(Some(LaunchResult::SystemChooser))
                }
                Ok(LaunchResult::SamsungNotes) => Err(JniError::ParseFailed(
                    "Samsung Notes was resolved but not launched".to_owned(),
                )),
                Err(ViewerError::NoCompatibleViewer) => Ok(None),
                Err(error) => Err(JniError::ParseFailed(error.to_string())),
            }
        })
        .map_err(|error| ViewerError::Platform(error.to_string()))?;
        result.ok_or(ViewerError::NoCompatibleViewer)
    }
}

fn pdf_view_intent<'local>(
    env: &mut jni::Env<'local>,
    uri: &JObject<'local>,
    spec: &PdfIntentSpec,
) -> Result<JObject<'local>, JniError> {
    let intent = env.new_object(jni_str!("android/content/Intent"), jni_sig!("()V"), &[])?;
    let action = env.new_string("android.intent.action.VIEW")?;
    env.call_method(
        &intent,
        jni_str!("setAction"),
        jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
        &[JValue::Object(&action)],
    )?;
    let mime_type = env.new_string(spec.mime_type())?;
    env.call_method(
        &intent,
        jni_str!("setDataAndType"),
        jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;"),
        &[JValue::Object(uri), JValue::Object(&mime_type)],
    )?;
    env.call_method(
        &intent,
        jni_str!("setFlags"),
        jni_sig!("(I)Landroid/content/Intent;"),
        &[JValue::Int(spec.grant_flags())],
    )?;
    Ok(intent)
}

fn start_activity(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    intent: &JObject<'_>,
) -> Result<(), JniError> {
    env.call_method(
        activity,
        jni_str!("startActivity"),
        jni_sig!("(Landroid/content/Intent;)V"),
        &[JValue::Object(intent)],
    )?;
    Ok(())
}
