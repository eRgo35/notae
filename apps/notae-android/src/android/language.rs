// Java calls this exported JNI callback; unsafe-code allowance is limited to its FFI boundary.
#![allow(unsafe_code)]
//! Connects the Settings language choice to Android's per-app language setting.
//!
//! On Android 13 and later the choice lives in Android itself, so Notae Settings and the
//! system's "App languages" screen always agree. Older versions have no such setting, so
//! the choice is kept in Notae's preferences instead.

use std::cell::RefCell;

use android_activity::AndroidApp;
use jni::errors::Error as JniError;
use jni::objects::{JObject, JString};
use jni::{EnvUnowned, JValue, jni_sig, jni_str};

use crate::language::{LanguageChoice, bundle_for};

thread_local! {
    /// Runs on the Slint thread when Android reports a language change.
    static ON_CHANGE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Lists English and Polish under Android's per-app languages (Android 14 and later).
pub fn declare_languages(app: &AndroidApp) {
    if let Err(error) = call_void(app, |env, activity| {
        env.call_method(
            activity,
            jni_str!("declareNotaeLanguages"),
            jni_sig!("()V"),
            &[],
        )
        .map(drop)
    }) {
        eprintln!("notae: cannot declare app languages: {error}");
    }
}

/// The choice Android stores, or `None` where Android has no per-app language setting.
pub fn read_choice(app: &AndroidApp) -> Option<LanguageChoice> {
    call_string(app, jni_str!("readNotaeLanguageChoice"))
        .inspect_err(|error| eprintln!("notae: cannot read the app language: {error}"))
        .ok()
        .flatten()
        .map(|tag| LanguageChoice::from_tag(&tag))
}

/// Stores `choice` in Android; `false` where Android has no per-app language setting.
pub fn write_choice(app: &AndroidApp, choice: LanguageChoice) -> bool {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        let tag = env.new_string(choice.tag())?;
        env.call_method(
            activity,
            jni_str!("writeNotaeLanguageChoice"),
            jni_sig!("(Ljava/lang/String;)Z"),
            &[JValue::Object(&tag)],
        )?
        .z()
    })
    .inspect_err(|error| eprintln!("notae: cannot store the app language: {error}"))
    .unwrap_or(false)
}

/// Shows the translation for `choice`, given the device language. Callers re-sync the
/// window afterwards so text built in Rust follows.
pub fn select(app: &AndroidApp, choice: LanguageChoice) {
    let system_tag = call_string(app, jni_str!("readSystemLanguageTag"))
        .ok()
        .flatten()
        .unwrap_or_default();
    let bundle = bundle_for(choice, &system_tag);
    eprintln!("notae: language choice={choice:?} system={system_tag:?} bundle={bundle:?}");
    crate::language::set_active_bundle(bundle);
    if let Err(error) = slint::select_bundled_translation(bundle) {
        eprintln!("notae: cannot select translation {bundle:?}: {error}");
    }
}

/// Registers what to do when Android reports a language change. Call on the Slint thread.
pub fn on_change(handler: impl Fn() + 'static) {
    ON_CHANGE.with(|slot| *slot.borrow_mut() = Some(Box::new(handler)));
}

fn call_void(
    app: &AndroidApp,
    call: impl FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> Result<(), JniError>,
) -> Result<(), JniError> {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        call(env, &activity)
    })
}

/// Calls a no-argument activity method returning a `String`; `None` for a Java `null`.
fn call_string(
    app: &AndroidApp,
    method: &'static jni::strings::JNIStr,
) -> Result<Option<String>, JniError> {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        let value = env
            .call_method(activity, method, jni_sig!("()Ljava/lang/String;"), &[])?
            .l()?;
        if value.is_null() {
            return Ok(None);
        }
        let value = env.cast_local::<JString>(value)?;
        Ok(Some(value.try_to_string(env)?))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeActivity_nativeOnLanguageConfigurationChanged(
    _env: EnvUnowned<'_>,
    _activity: JObject<'_>,
) {
    let _ = slint::invoke_from_event_loop(|| {
        ON_CHANGE.with(|slot| {
            if let Some(handler) = slot.borrow().as_ref() {
                handler();
            }
        });
    });
}
