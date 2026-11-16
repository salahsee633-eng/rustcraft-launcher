//! M3.1 — JVM bridge for Android.
//!
//! This module does one job: prove that a real OpenJDK runtime can be
//! unpacked from the APK and started inside our NativeActivity process.
//! It uses `JLI_Launch` (the same entry point PojavLauncher uses) rather than
//! spawning a `java` binary, because Android forbids executing files from the
//! app's writable data directory.
//!
//! The JRE and the tiny bridge test jar are produced by CI (see
//! `.github/workflows/android-preview.yml`); nothing here downloads them at
//! runtime, so the first test works offline.

use std::ffi::{c_char, c_int, c_void, CString};
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use android_activity::AndroidApp;

/// Increment when the CI pack changes so old extractions are replaced.
const JRE_PACK_VERSION: &str = "jre8-41";

static APP: OnceLock<AndroidApp> = OnceLock::new();

pub fn init(app: AndroidApp) {
    let _ = APP.set(app);
}

pub fn self_test() -> String {
    match run_self_test() {
        Ok(info) => format!("JVM OK — {info}"),
        Err(err) => format!("JVM bridge failed: {err}"),
    }
}

fn run_self_test() -> Result<String, String> {
    let app = APP.get().ok_or("Android context is missing")?;
    let files = app
        .internal_data_path()
        .ok_or("internal data path is unavailable")?;
    fs::create_dir_all(&files).map_err(|e| format!("cannot create data dir: {e}"))?;

    log_info("preparing JRE");
    let jre = prepare_jre(app, &files)?;
    let bridge = extract_asset(app, "bridge.jar", &files.join("bridge.jar"))?;

    let result_path = files.join("jvm-result.txt");
    let _ = fs::remove_file(&result_path);

    log_info(&format!("starting JVM at {}", jre.display()));
    std::thread::Builder::new()
        .name("jvm-launch".into())
        .stack_size(8 * 1024 * 1024)
        .spawn({
            let jre = jre.clone();
            let bridge = bridge.clone();
            let result_path = result_path.clone();
            let files = files.clone();
            move || {
                if let Err(err) = launch_jvm(&jre, &bridge, &result_path, &files) {
                    log_error(&err);
                }
            }
        })
        .map_err(|e| format!("cannot spawn JVM thread: {e}"))?
        .join()
        .map_err(|_| "JVM thread panicked".to_string())?;

    let text = fs::read_to_string(&result_path)
        .map_err(|e| format!("JVM produced no result file: {e}"))?;
    Ok(text.trim().replace('\n', " | "))
}

/// Unpacks `assets/jre8.zip` once into the app's private files directory.
fn prepare_jre(app: &AndroidApp, files: &Path) -> Result<PathBuf, String> {
    let home = files.join("jre8");
    let marker = home.join(".rustcraft-ready");
    if fs::read_to_string(&marker).map(|s| s.trim() == JRE_PACK_VERSION).unwrap_or(false) {
        return Ok(home);
    }

    log_info("unpacking JRE (first run only)");
    let zip_path = files.join("jre8.zip.tmp");
    extract_asset(app, "jre8.zip", &zip_path)?;

    let tmp = files.join("jre8.tmp");
    if tmp.exists() {
        fs::remove_dir_all(&tmp).map_err(|e| format!("cannot clean old JRE: {e}"))?;
    }
    fs::create_dir_all(&tmp).map_err(|e| format!("cannot create JRE dir: {e}"))?;

    let file = File::open(&zip_path).map_err(|e| format!("cannot open JRE zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("bad JRE zip: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("bad JRE entry {index}: {e}"))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out = tmp.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
            }
            let mut target =
                File::create(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
            io::copy(&mut entry, &mut target)
                .map_err(|e| format!("write {}: {e}", out.display()))?;
        }
    }

    fs::write(tmp.join(".rustcraft-ready"), JRE_PACK_VERSION)
        .map_err(|e| format!("write JRE marker: {e}"))?;
    if home.exists() {
        fs::remove_dir_all(&home).map_err(|e| format!("replace old JRE: {e}"))?;
    }
    fs::rename(&tmp, &home).map_err(|e| format!("finish JRE install: {e}"))?;
    let _ = fs::remove_file(&zip_path);
    Ok(home)
}

fn extract_asset(app: &AndroidApp, name: &str, out: &Path) -> Result<PathBuf, String> {
    let c_name = CString::new(name).map_err(|_| format!("bad asset name {name}"))?;
    let mut asset = app
        .asset_manager()
        .open(&c_name)
        .ok_or_else(|| format!("asset {name} is missing from the APK"))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let mut file = File::create(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    io::copy(&mut asset, &mut file).map_err(|e| format!("copy asset {name}: {e}"))?;
    Ok(out.to_path_buf())
}

type JliLaunch = unsafe extern "C" fn(
    argc: c_int,
    argv: *mut *mut c_char,
    jargc: c_int,
    jargv: *const *const c_char,
    appclassc: c_int,
    appclassv: *const *const c_char,
    fullversion: *const c_char,
    dotversion: *const c_char,
    pname: *const c_char,
    lname: *const c_char,
    javaargs: u8,
    cpwildcard: u8,
    javaw: u8,
    ergo: c_int,
) -> c_int;

fn launch_jvm(home: &Path, bridge: &Path, result: &Path, game_dir: &Path) -> Result<(), String> {
    let lib_dir = home.join("lib").join("aarch64");
    let jli_dir = lib_dir.join("jli");
    let server_dir = lib_dir.join("server");

    if !jli_dir.join("libjli.so").is_file() {
        return Err(format!("libjli.so not found under {}", jli_dir.display()));
    }

    std::env::set_var("JAVA_HOME", home);
    std::env::set_var("HOME", game_dir);
    std::env::set_var("TMPDIR", game_dir);
    let ld = format!(
        "{}:{}:{}:/system/lib64:/vendor/lib64",
        jli_dir.display(),
        lib_dir.display(),
        server_dir.display()
    );
    std::env::set_var("LD_LIBRARY_PATH", &ld);
    let _ = std::env::set_current_dir(game_dir);

    // Android's linker caches its search path early, so load every library we
    // need by absolute path with RTLD_GLOBAL before asking JLI to start the VM.
    let mut libs = Vec::new();
    collect_libs(&lib_dir, &mut libs);
    libs.sort_by_key(|path| preload_priority(path));
    for lib in libs {
        if lib.file_name().and_then(|n| n.to_str()) == Some("libjsig.so") {
            continue;
        }
        if let Err(err) = dlopen_global(&lib) {
            log_info(&format!("skip {}: {err}", lib.display()));
        }
    }

    let jli = jli_dir.join("libjli.so");
    let handle = dlopen_global(&jli)?;
    let symbol = unsafe { libc::dlsym(handle, b"JLI_Launch\0".as_ptr().cast()) };
    if symbol.is_null() {
        return Err("libjli.so has no JLI_Launch symbol".into());
    }
    let launch: JliLaunch = unsafe { std::mem::transmute(symbol) };

    let classpath = bridge.to_string_lossy().to_string();
    let java_args = [
        "java".to_string(),
        "-Xmx512M".to_string(),
        "-Xms128M".to_string(),
        "-XX:+UseSerialGC".to_string(),
        format!("-Djava.home={}", home.display()),
        format!("-Djava.library.path={}", lib_dir.display()),
        "-cp".to_string(),
        classpath,
        "rustcraft.BridgeTest".to_string(),
        result.to_string_lossy().to_string(),
    ];
    let cargs: Vec<CString> = java_args
        .iter()
        .map(|arg| CString::new(arg.as_bytes()).unwrap_or_default())
        .collect();
    let mut argv: Vec<*mut c_char> = cargs.iter().map(|arg| arg.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());

    let full = CString::new("1.8.0-internal").unwrap();
    let dot = CString::new("1.8").unwrap();
    let prog = CString::new("java").unwrap();

    let code = unsafe {
        launch(
            java_args.len() as c_int,
            argv.as_mut_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            full.as_ptr(),
            dot.as_ptr(),
            prog.as_ptr(),
            prog.as_ptr(),
            0, // javaargs (JNI_FALSE)
            1, // classpath wildcards
            0, // javaw
            0, // ergonomics
        )
    };

    if code == 0 {
        Ok(())
    } else {
        Err(format!("JLI_Launch returned {code}"))
    }
}

fn collect_libs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_libs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("so") {
            out.push(path);
        }
    }
}

fn preload_priority(path: &Path) -> (u8, String) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let rank = match name.as_str() {
        "libjava.so" => 0,
        "libverify.so" => 1,
        "libnet.so" => 2,
        "libnio.so" => 3,
        "libzip.so" => 4,
        "libjvm.so" => 5,
        _ => 6,
    };
    (rank, name)
}

fn dlopen_global(path: &Path) -> Result<*mut c_void, String> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| format!("path has a NUL byte: {}", path.display()))?;
    let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
    if handle.is_null() {
        let err = unsafe {
            let ptr = libc::dlerror();
            if ptr.is_null() {
                "unknown dlopen error".to_string()
            } else {
                std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        Err(err)
    } else {
        Ok(handle)
    }
}

fn log_info(message: &str) {
    log_android(4, message); // ANDROID_LOG_INFO
}

fn log_error(message: &str) {
    log_android(6, message); // ANDROID_LOG_ERROR
}

fn log_android(priority: c_int, message: &str) {
    let tag = CString::new("RustCraft").unwrap();
    let text = CString::new(message.replace('\0', " ")).unwrap();
    unsafe {
        __android_log_write(priority, tag.as_ptr(), text.as_ptr());
    }
}

extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}
