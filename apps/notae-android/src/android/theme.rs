//! System motion preference. Colors come from the fixed One UI palette in `components.slint`.
use android_activity::AndroidApp;
use jni::{jni_sig, jni_str};

use crate::AppWindow;

pub fn configure(app: &AndroidApp, window: &AppWindow) {
    let result = super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, app.activity_as_ptr())?;
        env.call_method(&activity, jni_str!("isNotaeMotionReduced"), jni_sig!("()Z"), &[])?
            .z()
    });
    match result {
        Ok(reduced) => window.set_reduced_motion(reduced),
        Err(error) => eprintln!("notae: cannot read Android motion preference: {error}"),
    }
}
