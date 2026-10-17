// Java calls this exported JNI callback; unsafe-code allowance is limited to its FFI boundary.
#![allow(unsafe_code)]
//! Reads Android's system animator scale and updates screen motion when the activity resumes.

use std::sync::{LazyLock, Mutex};

use android_activity::AndroidApp;
use jni::objects::JObject;
use jni::{EnvUnowned, jni_sig, jni_str};

use crate::AppWindow;

static WINDOW: LazyLock<Mutex<Option<slint::Weak<AppWindow>>>> = LazyLock::new(|| Mutex::new(None));

pub fn bind_window(window: slint::Weak<AppWindow>) {
    if let Ok(mut slot) = WINDOW.lock() {
        *slot = Some(window);
    }
}

pub fn read_animations_enabled(app: &AndroidApp) -> bool {
    read_animator_scale(app)
        .map(crate::motion::animations_enabled_for_scale)
        .unwrap_or(true)
}

fn read_animator_scale(app: &AndroidApp) -> Result<f32, jni::errors::Error> {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        env.call_method(
            activity,
            jni_str!("readAnimatorDurationScale"),
            jni_sig!("()F"),
            &[],
        )?
        .f()
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeActivity_nativeOnAnimatorDurationScaleChanged(
    _env: EnvUnowned<'_>,
    _activity: JObject<'_>,
    scale: f32,
) {
    let window = WINDOW.lock().ok().and_then(|slot| slot.clone());
    if let Some(window) = window {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = window.upgrade() {
                window.set_animations_enabled(crate::motion::animations_enabled_for_scale(scale));
            }
        });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeActivity_nativeOnApplicationActiveChanged(
    _env: EnvUnowned<'_>,
    _activity: JObject<'_>,
    active: bool,
) {
    let window = WINDOW.lock().ok().and_then(|slot| slot.clone());
    if let Some(window) = window {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = window.upgrade() {
                window.set_application_active(active);
            }
        });
    }
}
