//! Wallpaper-derived Material dark roles and system motion preference.
use android_activity::AndroidApp;
use jni::{JValue, jni_sig, jni_str};
use slint::ComponentHandle;

use crate::{AppWindow, Theme};

pub fn configure(app: &AndroidApp, window: &AppWindow) {
    let result = super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, app.activity_as_ptr())?;
        let mut colors = [0_u32; 10];
        for (role, color) in colors.iter_mut().enumerate() {
            *color = env.call_method(
                &activity,
                jni_str!("getNotaeColor"),
                jni_sig!("(I)I"),
                &[JValue::Int(role as i32)],
            )?.i()? as u32;
        }
        let reduced = env.call_method(
            &activity,
            jni_str!("isNotaeMotionReduced"),
            jni_sig!("()Z"),
            &[],
        )?.z()?;
        Ok((colors, reduced))
    });
    match result {
        Ok((colors, reduced)) => {
            let theme = window.global::<Theme>();
            if colors[0] != 0 {
                let colors = colors.map(slint::Color::from_argb_encoded);
                theme.set_background(colors[0]);
                theme.set_surface(colors[1]);
                theme.set_container(colors[2]);
                theme.set_high(colors[3]);
                theme.set_foreground(colors[4]);
                theme.set_muted(colors[5]);
                theme.set_primary(colors[6]);
                theme.set_on_primary(colors[7]);
                theme.set_primary_container(colors[8]);
                theme.set_outline(colors[9]);
            }
            window.set_reduced_motion(reduced);
        }
        Err(error) => eprintln!("notae: cannot read Android appearance preferences: {error}"),
    }
}
