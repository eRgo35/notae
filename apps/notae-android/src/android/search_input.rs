// Java calls these exported JNI callbacks; unsafe-code allowance is limited to their FFI boundary.
#![allow(unsafe_code)]
//! Android keyboard and system speech-recognition integration.

use std::sync::{Mutex, OnceLock};

use android_activity::AndroidApp;
use jni::objects::{JObject, JObjectArray, JString};
use jni::{EnvUnowned, jni_sig, jni_str};

use crate::AppWindow;
use crate::voice_search::{recognized_query, voice_result_is_current};

pub const VOICE_REQUEST_CODE: i32 = 0x4e4f;
const RESULT_OK: i32 = -1;
static WINDOW: OnceLock<Mutex<Option<slint::Weak<AppWindow>>>> = OnceLock::new();

fn window_slot() -> &'static Mutex<Option<slint::Weak<AppWindow>>> {
    WINDOW.get_or_init(|| Mutex::new(None))
}

pub fn bind_window(window: slint::Weak<AppWindow>) {
    if let Ok(mut slot) = window_slot().lock() {
        *slot = Some(window);
    }
}

pub fn read_hardware_keyboard(app: &AndroidApp) -> bool {
    call_activity_bool(app, ActivityMethod::HardwareKeyboard).unwrap_or(false)
}

pub fn voice_recognizer_available(app: &AndroidApp) -> bool {
    call_activity_bool(app, ActivityMethod::VoiceAvailable).unwrap_or(false)
}

pub fn launch_voice_recognition(app: &AndroidApp) -> Result<bool, jni::errors::Error> {
    call_activity_bool(app, ActivityMethod::StartVoice)
}

#[derive(Clone, Copy)]
enum ActivityMethod {
    HardwareKeyboard,
    VoiceAvailable,
    StartVoice,
}

fn call_activity_bool(
    app: &AndroidApp,
    method: ActivityMethod,
) -> Result<bool, jni::errors::Error> {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        let result = match method {
            ActivityMethod::HardwareKeyboard => env.call_method(
                activity,
                jni_str!("hasHardwareKeyboard"),
                jni_sig!("()Z"),
                &[],
            )?,
            ActivityMethod::VoiceAvailable => env.call_method(
                activity,
                jni_str!("hasVoiceRecognizer"),
                jni_sig!("()Z"),
                &[],
            )?,
            ActivityMethod::StartVoice => env.call_method(
                activity,
                jni_str!("startVoiceRecognition"),
                jni_sig!("()Z"),
                &[],
            )?,
        };
        result.z()
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeActivity_nativeOnKeyboardConfigurationChanged(
    _env: EnvUnowned<'_>,
    _activity: JObject<'_>,
    present: bool,
) {
    let window = window_slot().lock().ok().and_then(|slot| slot.clone());
    if let Some(window) = window {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = window.upgrade() {
                window.set_hardware_keyboard_present(present);
            }
        });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeActivity_nativeOnVoiceResult<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _activity: JObject<'local>,
    request_code: jni::sys::jint,
    result_code: jni::sys::jint,
    results: JObjectArray<'local, JString<'local>>,
) {
    let phrases = unowned_env
        .with_env(|env| -> Result<Vec<String>, jni::errors::Error> {
            if results.is_null() {
                return Ok(Vec::new());
            }
            let count = results.len(env)?;
            let mut phrases = Vec::with_capacity(count);
            for index in 0..count {
                let value = results.get_element(env, index)?;
                if value.is_null() {
                    continue;
                }
                let phrase = JString::from(value);
                phrases.push(phrase.mutf8_chars(env)?.to_str().into_owned());
            }
            Ok(phrases)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();

    let window = window_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .filter(|window| window.upgrade().is_some());
    if !voice_result_is_current(request_code, VOICE_REQUEST_CODE, window.is_some()) {
        return;
    }
    let Some(query) = recognized_query(result_code == RESULT_OK, &phrases).map(str::to_owned)
    else {
        return;
    };
    if let Some(window) = window {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = window.upgrade() {
                window.set_search_text(query.clone().into());
                window.invoke_search_changed(query.into());
            }
        });
    }
}
