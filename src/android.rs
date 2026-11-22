//! Platform glue for the Android build: soft-keyboard type, haptics and
//! opening links. Everything is a no-op on the desktop so the same UI code can
//! be exercised locally.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Haptic {
    Tap,
    LongPress,
    Confirm,
    Reject,
}

#[cfg(target_os = "android")]
mod imp {
    use std::sync::OnceLock;

    use android_activity::input::{ImeOptions, InputType, TextInputAction};
    use android_activity::AndroidApp;
    use jni::objects::{JObject, JValue};
    use jni::{jni_sig, jni_str, Env, JavaVM};

    use super::Haptic;

    static APP: OnceLock<AndroidApp> = OnceLock::new();

    pub fn init(app: &AndroidApp) {
        let _ = APP.set(app.clone());
    }

    pub fn app() -> Option<&'static AndroidApp> {
        APP.get()
    }

    pub fn set_numeric_ime() {
        let Some(app) = app() else { return };
        app.set_ime_editor_info(
            InputType::TYPE_CLASS_NUMBER | InputType::TYPE_NUMBER_FLAG_DECIMAL,
            TextInputAction::Done,
            ImeOptions::empty(),
        );
        app.show_soft_input(true);
    }

    pub fn set_text_ime() {
        let Some(app) = app() else { return };
        app.set_ime_editor_info(
            InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_CAP_SENTENCES,
            TextInputAction::Done,
            ImeOptions::empty(),
        );
        app.show_soft_input(true);
    }

    pub fn hide_keyboard() {
        if let Some(app) = app() {
            app.hide_soft_input(false);
        }
    }

    fn with_env(f: impl FnOnce(&mut Env, &JObject)) {
        let Some(app) = app() else { return };
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
        let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            f(env, &activity);
            Ok(())
        });
    }

    pub fn haptic(kind: Haptic) {
        let constant = match kind {
            Haptic::Tap => 1,       // HapticFeedbackConstants.VIRTUAL_KEY
            Haptic::LongPress => 0, // LONG_PRESS
            Haptic::Confirm => 16,  // CONFIRM
            Haptic::Reject => 17,   // REJECT
        };
        with_env(|env, activity| {
            let window = env
                .call_method(
                    activity,
                    jni_str!("getWindow"),
                    jni_sig!("()Landroid/view/Window;"),
                    &[],
                )
                .ok()
                .and_then(|value| value.l().ok());
            let Some(window) = window else { return };
            let decor = env
                .call_method(
                    &window,
                    jni_str!("getDecorView"),
                    jni_sig!("()Landroid/view/View;"),
                    &[],
                )
                .ok()
                .and_then(|value| value.l().ok());
            let Some(decor) = decor else { return };
            let _ = env.call_method(
                &decor,
                jni_str!("performHapticFeedback"),
                jni_sig!("(I)Z"),
                &[JValue::Int(constant)],
            );
        });
    }

    pub fn open_url(url: &str) {
        with_env(|env, activity| {
            let Ok(intent) =
                env.new_object(jni_str!("android/content/Intent"), jni_sig!("()V"), &[])
            else {
                return;
            };
            let Ok(action) = env.new_string("android.intent.action.VIEW") else {
                return;
            };
            let _ = env.call_method(
                &intent,
                jni_str!("setAction"),
                jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
                &[JValue::Object(action.as_ref())],
            );
            let Ok(text) = env.new_string(url) else {
                return;
            };
            let Ok(uri) = env
                .call_static_method(
                    jni_str!("android/net/Uri"),
                    jni_str!("parse"),
                    jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                    &[JValue::Object(text.as_ref())],
                )
                .and_then(|value| value.l())
            else {
                return;
            };
            let _ = env.call_method(
                &intent,
                jni_str!("setData"),
                jni_sig!("(Landroid/net/Uri;)Landroid/content/Intent;"),
                &[JValue::Object(&uri)],
            );
            // FLAG_ACTIVITY_NEW_TASK
            let _ = env.call_method(
                &intent,
                jni_str!("addFlags"),
                jni_sig!("(I)Landroid/content/Intent;"),
                &[JValue::Int(0x1000_0000)],
            );
            let _ = env.call_method(
                activity,
                jni_str!("startActivity"),
                jni_sig!("(Landroid/content/Intent;)V"),
                &[JValue::Object(&intent)],
            );
        });
    }
}

#[cfg(target_os = "android")]
pub use imp::*;

#[cfg(not(target_os = "android"))]
pub fn init() {}

#[cfg(not(target_os = "android"))]
pub fn set_numeric_ime() {}

#[cfg(not(target_os = "android"))]
pub fn set_text_ime() {}

#[cfg(not(target_os = "android"))]
pub fn hide_keyboard() {}

#[cfg(not(target_os = "android"))]
pub fn haptic(_kind: Haptic) {}

#[cfg(not(target_os = "android"))]
pub fn open_url(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
