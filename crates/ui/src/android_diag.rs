//! Startup diagnostics for the Android build.
//!
//! A NativeActivity that dies before the first frame gives the user no clue
//! why. This module logs to logcat, appends to a file in app storage, mirrors
//! the log into public Downloads through the Java `Diag` helper, and shows
//! errors as Android Toasts.

use std::ffi::{c_char, c_int, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use android_activity::AndroidApp;
use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};

static APP: OnceLock<AndroidApp> = OnceLock::new();
static DIAG_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Called from `LauncherActivity.onCreate` on the Java main thread, where
/// `FindClass` can see application classes. Later calls come from the native
/// thread, so we keep a global reference.
#[no_mangle]
pub extern "system" fn Java_org_rustcraft_LauncherActivity_nativeRegisterDiagClass(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    diag_class: jni::sys::jclass,
) {
    let env = match unsafe { JNIEnv::from_raw(env) } {
        Ok(env) => env,
        Err(_) => return,
    };
    let object = unsafe { JObject::from_raw(diag_class as jni::sys::jobject) };
    if let Ok(global) = env.new_global_ref(object) {
        let _ = DIAG_CLASS.set(global);
    }
}

pub fn install(app: AndroidApp) {
    let _ = APP.set(app.clone());
    std::panic::set_hook(Box::new(move |info| {
        let message = format!("RustCraft panic: {info}");
        logcat(6, &message);
        if let Some(app) = APP.get() {
            append_log(app, &message);
            publish_log(app, &message);
            let _ = toast(app, &message);
        }
    }));
}

pub fn log(app: &AndroidApp, step: &str) {
    let message = format!("[{}] {step}", timestamp());
    logcat(4, &message);
    append_log(app, &message);
    publish_log(app, &message);
}

pub fn toast_error(app: &AndroidApp, what: &str, error: &str) {
    let message = format!("{what}: {error}");
    logcat(6, &message);
    append_log(app, &message);
    publish_log(app, &message);
    let _ = toast(app, &message);
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn append_log(app: &AndroidApp, message: &str) {
    let Some(dir) = app.external_data_path().or_else(|| app.internal_data_path()) else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path: PathBuf = dir.join("rustcraft-startup.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn publish_log(app: &AndroidApp, message: &str) {
    let vm = match unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) } {
        Ok(vm) => vm,
        Err(_) => return,
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(_) => return,
    };
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let text = match env.new_string(message) {
        Ok(text) => text,
        Err(_) => return,
    };

    let cached_class;
    let class = if let Some(global) = DIAG_CLASS.get() {
        cached_class = unsafe { JClass::from_raw(global.as_obj().as_raw()) };
        &cached_class
    } else {
        match env.find_class("org/rustcraft/Diag") {
            Ok(class) => {
                cached_class = class;
                &cached_class
            }
            Err(_) => return,
        }
    };

    let _ = env
        .call_static_method(
            class,
            "log",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(&activity), JValue::Object(&text)],
        )
        .map(|_| ());
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
