//! Startup diagnostics for the Android build.
//!
//! A NativeActivity that dies before the first frame gives the user no clue
//! why. This module installs a panic hook that logs to logcat, appends to a
//! file in external app storage, and shows the message as an Android Toast.
//! It also logs the startup steps so we can tell how far the process got.

use std::ffi::{c_char, c_int, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use android_activity::AndroidApp;
use jni::objects::{JObject, JString, JValue};
use jni::JavaVM;

static APP: OnceLock<AndroidApp> = OnceLock::new();

pub fn install(app: AndroidApp) {
    let _ = APP.set(app.clone());
    std::panic::set_hook(Box::new(move |info| {
        let message = format!("RustCraft panic: {info}");
        logcat(6, &message);
        if let Some(app) = APP.get() {
            append_log(app, &message);
            let _ = toast(app, &message);
        }
    }));
}

pub fn log(app: &AndroidApp, step: &str) {
    logcat(4, step);
    append_log(app, step);
}

pub fn toast_error(app: &AndroidApp, what: &str, error: &str) {
    let message = format!("{what}: {error}");
    logcat(6, &message);
    append_log(app, &message);
    let _ = toast(app, &message);
}

fn append_log(app: &AndroidApp, message: &str) {
    let Some(dir) = app.external_data_path().or_else(|| app.internal_data_path()) else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path: PathBuf = dir.join("rustcraft-startup.log");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{timestamp} {message}");
    }
}

fn logcat(priority: c_int, message: &str) {
    let tag = CString::new("RustCraft").unwrap();
    let text = CString::new(message.replace('\0', " ")).unwrap();
    unsafe {
        __android_log_write(priority, tag.as_ptr(), text.as_ptr());
    }
}

fn toast(app: &AndroidApp, message: &str) -> Result<(), String> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
        .map_err(|e| format!("JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let class = env
        .find_class("android/widget/Toast")
        .map_err(|e| format!("Toast class: {e}"))?;
    let text = env.new_string(message).map_err(|e| format!("text: {e}"))?;
    let toast = env
        .call_static_method(
            class,
            "makeText",
            "(Landroid/content/Context;Ljava/lang/CharSequence;I)Landroid/widget/Toast;",
            &[
                JValue::Object(&activity),
                JValue::Object(&text),
                JValue::Int(1), // LENGTH_LONG
            ],
        )
        .map_err(|e| format!("makeText: {e}"))?
        .l()
        .map_err(|e| format!("toast object: {e}"))?;
    env.call_method(toast, "show", "()V", &[])
        .map_err(|e| format!("show: {e}"))?;
    Ok(())
}

extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}
