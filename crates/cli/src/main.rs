//! Desktop harness for the RustCraft core.
//!
//! This is intentionally the *first* thing you get working: install, log in and
//! launch on a Linux/Windows/macOS machine, then move the same core into the
//! Android app. Debugging renderers on a phone is painful; debugging them here
//! is not.

use std::path::PathBuf;

use rustcraft_core::auth;
use rustcraft_core::install::{self, InstallOptions};
use rustcraft_core::launch::{self, LaunchOptions};
use rustcraft_core::manifest;
use rustcraft_core::paths::Paths;
use rustcraft_core::rules::Env;
use rustcraft_core::session::{self, Session};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> rustcraft_core::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        print_help();
        return Ok(());
    };

    let root = value_of(&args, "--root").map(Paths::new);
    let paths = root.unwrap_or_else(Paths::desktop);

    match command.as_str() {
        "list" => list_versions(),
        "install" => {
            let id = args.get(1).ok_or_else(|| rustcraft_core::Error::other("usage: install <version>"))?;
            let opts = InstallOptions {
                env: Env::desktop(),
                concurrency: 4,
                extract_natives: true,
            };
            println!("installing {id} into {}", paths.root.display());
            let report = install::install(&paths, id, &opts)?;
            println!(
                "done: {} files, {:.1} MB",
                report.files,
                report.total_bytes as f64 / 1_048_576.0
            );
            Ok(())
        }
        "login" => {
            if let Some(name) = value_of(&args, "--offline") {
                let s = Session::offline(&name);
                session::save(&paths, &s)?;
                println!("saved offline session '{name}'");
                return Ok(());
            }

            let client_id = value_of(&args, "--client-id")
                .unwrap_or_else(|| auth::PLACEHOLDER_CLIENT_ID.to_string());
            println!("requesting a Microsoft device code...");
            let code = auth::start_device_login(&client_id)?;
            println!();
            println!("  {}", code.message);
            println!("  open: {}", code.verification_uri);
            println!("  code: {}", code.user_code);
            println!();
            println!("waiting...");

            let tokens = auth::wait_for_device_login(&client_id, &code, |_, _| {})?;
            let mut s = auth::login_with_ms_tokens(&tokens)?;
            s.client_id = Some(client_id);
            session::save(&paths, &s)?;
            println!("signed in as {} ({})", s.username, s.uuid);
            Ok(())
        }
        "plan" => {
            let id = args.get(1).ok_or_else(|| rustcraft_core::Error::other("usage: plan <version>"))?;
            let s = load_or_offline(&paths)?;
            let plan = launch::build_plan(&paths, id, &s, &desktop_options(&args)?)?;
            println!("{}", plan.program.display());
            for arg in &plan.args {
                println!("  {arg}");
            }
            Ok(())
        }
        "launch" => {
            let id = args.get(1).ok_or_else(|| rustcraft_core::Error::other("usage: launch <version>"))?;
            let s = load_or_offline(&paths)?;
            let plan = launch::build_plan(&paths, id, &s, &desktop_options(&args)?)?;
            let log = paths.logs_dir().join("latest.log");
            println!("starting {} (log: {})", s.username, log.display());
            let mut child = plan.spawn_with_log(&log)?;
            let status = child.wait()?;
            println!("game exited with {status}");
            Ok(())
        }
        "java" => {
            println!("{}", find_java().display());
            Ok(())
        }
        "device" => {
            let profile = rustcraft_core::profile::DeviceProfile::detect();
            println!("{}", profile.describe());
            println!("heap: {:?}", profile.jvm_heap_args());
            println!("renderer: {}", profile.renderer.label());
            Ok(())
        }
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(rustcraft_core::Error::other(format!(
            "unknown command '{other}' (try: help)"
        ))),
    }
}

fn list_versions() -> rustcraft_core::Result<()> {
    let manifest: manifest::VersionManifest = rustcraft_core::download::get_json(
        manifest::VERSION_MANIFEST_URL,
    )?;
    println!("latest release: {}  snapshot: {}", manifest.latest.release, manifest.latest.snapshot);
    for version in manifest.versions.iter().filter(|v| v.kind == "release").take(25) {
        println!("{:>14}  {}", version.id, version.kind);
    }
    Ok(())
}

fn load_or_offline(paths: &Paths) -> rustcraft_core::Result<Session> {
    match session::load(paths)? {
        Some(session) => Ok(session),
        None => {
            eprintln!("no saved account, using offline profile 'Player'");
            Ok(Session::offline("Player"))
        }
    }
}

fn desktop_options(args: &[String]) -> rustcraft_core::Result<LaunchOptions> {
    let ram = value_of(args, "--ram")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(2048);
    let width = value_of(args, "--width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1280);
    let height = value_of(args, "--height")
        .and_then(|v| v.parse().ok())
        .unwrap_or(720);

    let mut opts = LaunchOptions {
        max_memory_mb: ram,
        min_memory_mb: (ram / 4).max(256),
        width,
        height,
        java: Some(find_java()),
        env: Env::desktop(),
        ..LaunchOptions::default()
    };
    opts.extra_jvm_args.push("-Djava.awt.headless=false".into());
    Ok(opts)
}

fn find_java() -> PathBuf {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let candidate = PathBuf::from(home).join("bin").join("java");
        if candidate.is_file() {
            return candidate;
        }
    }
    // Fall back to PATH; Command resolves it at spawn time.
    PathBuf::from(if cfg!(windows) { "java.exe" } else { "java" })
}

fn value_of(args: &[String], key: &str) -> Option<String> {
    let index = args.iter().position(|a| a == key)?;
    args.get(index + 1).cloned()
}

fn print_help() {
    println!(
        "RustCraft Launcher (desktop core harness)

USAGE:
  rustcraft list
  rustcraft install <version> [--root PATH]
  rustcraft login [--client-id ID] | login --offline <name>
  rustcraft plan <version> [--ram MB] [--width N] [--height N]
  rustcraft launch <version> [--ram MB] [--width N] [--height N]
  rustcraft device
  rustcraft java

The Android app uses the exact same rustcraft-core crate."
    );
}
