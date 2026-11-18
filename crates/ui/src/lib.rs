//! Slint UI shared by the desktop harness and the Android build.
//!
//! The UI deliberately stays "dumb": it owns no game state, it only renders
//! properties and forwards callbacks to `rustcraft-core` (or to the Android
//! JVM bridge later).

slint::include_modules!();

#[cfg(feature = "android")]
mod android_bridge;
#[cfg(feature = "android")]
mod android_diag;

use slint::{Model, ModelRc, VecModel};

pub fn build_ui() -> Result<AppWindow, slint::PlatformError> {
    let ui = AppWindow::new()?;

    let versions = initial_versions();
    ui.set_versions(ModelRc::new(VecModel::from(versions)));
    ui.set_selected(0);
    ui.set_account_name("NOT SIGNED IN".into());
    #[cfg(feature = "android")]
    ui.set_status_text("M3.1: press PLAY to test the JVM bridge.".into());
    #[cfg(not(feature = "android"))]
    ui.set_status_text("Ready. Install a version, then press PLAY.".into());

    let weak = ui.as_weak();
    ui.on_select(move |index| {
        if let Some(ui) = weak.upgrade() {
            ui.set_selected(index);
        }
    });

    let weak = ui.as_weak();
    ui.on_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            let versions = initial_versions();
            ui.set_versions(ModelRc::new(VecModel::from(versions)));
            ui.set_status_text("Version list refreshed.".into());
        }
    });

    let weak = ui.as_weak();
    ui.on_login(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status_text(
                "Microsoft sign-in lands in milestone M4; use `rustcraft login` for now.".into(),
            );
        }
    });

    let weak = ui.as_weak();
    ui.on_settings(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status_text(
                "Settings screen (RAM, renderer, JVM args) is milestone M5.".into(),
            );
        }
    });

    let weak = ui.as_weak();
    ui.on_nav(move |target| {
        if let Some(ui) = weak.upgrade() {
            let message = match target.as_str() {
                "home" => "Home",
                "instances" => "Instances (M5)",
                _ => "Not implemented yet",
            };
            ui.set_status_text(message.into());
        }
    });

    let weak = ui.as_weak();
    ui.on_play(move || {
        let Some(ui) = weak.upgrade() else { return };
        let index = ui.get_selected().max(0) as usize;
        let Some(item) = ui.get_versions().row_data(index) else {
            ui.set_status_text("No version selected.".into());
            return;
        };
        let version_id = item.id.to_string();

        ui.set_busy(true);
        #[cfg(feature = "android")]
        ui.set_status_text("Testing JVM bridge (first run unpacks the runtime)...".into());
        #[cfg(not(feature = "android"))]
        ui.set_status_text(format!("Launching {version_id}...").into());

        let weak = weak.clone();
        std::thread::spawn(move || {
            let message = try_launch(&version_id);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_busy(false);
                    ui.set_status_text(message.as_str().into());
                }
            });
        });
    });

    Ok(ui)
}

fn initial_versions() -> Vec<VersionItem> {
    #[cfg(feature = "desktop")]
    {
        let paths = rustcraft_core::paths::Paths::desktop();
        let mut found = Vec::new();
        if let Ok(entries) = std::fs::read_dir(paths.versions_dir()) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() {
                    continue;
                }
                let id = entry.file_name().to_string_lossy().to_string();
                if paths.version_json(&id).is_file() {
                    found.push(VersionItem {
                        id: id.as_str().into(),
                        kind: "installed".into(),
                        installed: true,
                    });
                }
            }
        }
        if !found.is_empty() {
            found.sort_by(|a, b| b.id.as_str().cmp(a.id.as_str()));
            return found;
        }
    }

    vec![
        VersionItem {
            id: "1.21.1".into(),
            kind: "release".into(),
            installed: false,
        },
        VersionItem {
            id: "1.20.1".into(),
            kind: "release".into(),
            installed: false,
        },
        VersionItem {
            id: "1.8.9".into(),
            kind: "legacy".into(),
            installed: false,
        },
    ]
}

#[cfg(feature = "desktop")]
fn try_launch(version_id: &str) -> String {
    use rustcraft_core::launch::{self, LaunchOptions};
    use rustcraft_core::paths::Paths;
    use rustcraft_core::rules::Env;
    use rustcraft_core::session::{self, Session};

    let paths = Paths::desktop();
    let session = match session::load(&paths) {
        Ok(Some(session)) => session,
        Ok(None) => Session::offline("Player"),
        Err(err) => return format!("Could not read account: {err}"),
    };

    let opts = LaunchOptions {
        max_memory_mb: 2048,
        min_memory_mb: 512,
        java: Some(find_java()),
        env: Env::desktop(),
        ..LaunchOptions::default()
    };

    let plan = match launch::build_plan(&paths, version_id, &session, &opts) {
        Ok(plan) => plan,
        Err(err) => return format!("Cannot launch {version_id}: {err}"),
    };

    let log = paths.logs_dir().join("latest.log");
    match plan.spawn_with_log(&log) {
        Ok(_) => format!("Started {version_id}. Log: {}", log.display()),
        Err(err) => format!("Failed to start java: {err}"),
    }
}

#[cfg(all(feature = "android", not(feature = "desktop")))]
fn try_launch(_version_id: &str) -> String {
    android_bridge::self_test()
}

#[cfg(not(any(feature = "desktop", feature = "android")))]
fn try_launch(_version_id: &str) -> String {
    "This build has no launcher backend enabled.".to_string()
}

#[cfg(feature = "desktop")]
fn find_java() -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let candidate = PathBuf::from(home).join("bin").join("java");
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(if cfg!(windows) { "java.exe" } else { "java" })
}

#[cfg(target_os = "android")]
mod android {
    use super::build_ui;
    use slint::ComponentHandle;

    /// Entry point used by `android-activity` (NativeActivity).
    #[no_mangle]
    pub fn android_main(app: android_activity::AndroidApp) {
        android_diag::install(app.clone());
        android_diag::log(&app, "android_main: start");

        android_bridge::init(app.clone());
        if let Err(err) = slint::android::init(app.clone()) {
            android_diag::toast_error(&app, "Slint init failed", &err.to_string());
            return;
        }
        android_diag::log(&app, "android_main: platform ready");

        let ui = match build_ui() {
            Ok(ui) => ui,
            Err(err) => {
                android_diag::toast_error(&app, "UI build failed", &err.to_string());
                return;
            }
        };
        android_diag::log(&app, "android_main: UI ready");

        if let Err(err) = ui.run() {
            android_diag::toast_error(&app, "UI loop failed", &err.to_string());
        }
    }
}
