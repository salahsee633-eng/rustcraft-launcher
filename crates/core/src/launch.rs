use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::error::{Error, Result};
use crate::install::game_jar;
use crate::manifest;
use crate::paths::Paths;
use crate::rules::{allowed, Env};
use crate::session::Session;
use crate::{LAUNCHER_NAME, LAUNCHER_VERSION};

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub max_memory_mb: u32,
    pub min_memory_mb: u32,
    pub width: u32,
    pub height: u32,
    pub demo: bool,
    pub java: Option<PathBuf>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    /// Skip libraries whose Maven group starts with one of these prefixes.
    /// Android builds use this to drop desktop LWJGL in favour of the mobile
    /// LWJGL fork (see docs/ARCHITECTURE.md).
    pub classpath_exclude: Vec<String>,
    /// Jars that must come before everything else on the classpath.
    pub extra_classpath: Vec<PathBuf>,
    pub env: Env,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        LaunchOptions {
            max_memory_mb: 1024,
            min_memory_mb: 256,
            width: 1280,
            height: 720,
            demo: false,
            java: None,
            extra_jvm_args: Vec::new(),
            extra_game_args: Vec::new(),
            classpath_exclude: Vec::new(),
            extra_classpath: Vec::new(),
            env: Env::android(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub cwd: PathBuf,
}

impl LaunchPlan {
    pub fn spawn(&self) -> Result<Child> {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args).current_dir(&self.cwd);
        for (key, value) in &self.envs {
            cmd.env(key, value);
        }
        Ok(cmd.spawn()?)
    }

    /// Spawn while teeing stdout/stderr into a log file (the in-game log).
    pub fn spawn_with_log(&self, log_path: &std::path::Path) -> Result<Child> {
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let log = File::create(log_path)?;
        let err_log = log.try_clone()?;

        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err_log));
        for (key, value) in &self.envs {
            cmd.env(key, value);
        }
        Ok(cmd.spawn()?)
    }
}

/// Builds the complete `java` command line for a version.
pub fn build_plan(
    paths: &Paths,
    version_id: &str,
    session: &Session,
    opts: &LaunchOptions,
) -> Result<LaunchPlan> {
    let version = manifest::resolve_version(paths, version_id)?;
    if version.main_class.is_empty() {
        return Err(Error::Launch(format!(
            "version {version_id} has no main class"
        )));
    }

    let mut env = opts.env.clone();
    env.features.insert("is_demo_user".into(), opts.demo);
    env.features.insert("has_custom_resolution".into(), true);
    env.features.insert("has_quick_plays_support".into(), false);

    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut missing: Vec<PathBuf> = Vec::new();

    for extra in &opts.extra_classpath {
        if seen.insert(extra.clone()) {
            classpath.push(extra.clone());
        }
    }

    for lib in &version.libraries {
        if !allowed(&lib.rules, &env) {
            continue;
        }
        if opts
            .classpath_exclude
            .iter()
            .any(|prefix| lib.name.starts_with(prefix))
        {
            continue;
        }
        let Some(relative) = lib.classpath_relative() else {
            continue;
        };
        let path = paths.libraries_dir().join(relative);
        if !path.is_file() {
            missing.push(path);
            continue;
        }
        if seen.insert(path.clone()) {
            classpath.push(path);
        }
    }

    if !missing.is_empty() {
        let sample = missing
            .iter()
            .take(4)
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::Launch(format!(
            "{} libraries are missing (did install finish?): {sample}",
            missing.len()
        )));
    }

    let jar = game_jar(paths, &version)?;
    if seen.insert(jar.clone()) {
        classpath.push(jar);
    }

    let natives_dir = paths.natives_dir(&version.id);
    std::fs::create_dir_all(&natives_dir)?;

    let index_id = version.asset_index_id();
    let separator = if env.os_name == "windows" { ";" } else { ":" };
    let classpath_string = classpath
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join(separator);

    let mut vars = HashMap::new();
    insert(&mut vars, "natives_directory", &natives_dir.to_string_lossy());
    insert(&mut vars, "launcher_name", LAUNCHER_NAME);
    insert(&mut vars, "launcher_version", LAUNCHER_VERSION);
    insert(&mut vars, "classpath", &classpath_string);
    insert(&mut vars, "classpath_separator", separator);
    insert(
        &mut vars,
        "library_directory",
        &paths.libraries_dir().to_string_lossy(),
    );
    insert(&mut vars, "version_name", &version.id);
    insert(
        &mut vars,
        "game_directory",
        &paths.root.to_string_lossy(),
    );
    insert(&mut vars, "assets_root", &paths.assets_dir().to_string_lossy());
    insert(&mut vars, "assets_index_name", &index_id);
    insert(&mut vars, "auth_player_name", &session.username);
    insert(&mut vars, "auth_uuid", &session.uuid);
    insert(&mut vars, "auth_access_token", &session.access_token);
    insert(
        &mut vars,
        "auth_xuid",
        session.xuid.as_deref().unwrap_or_default(),
    );
    insert(
        &mut vars,
        "clientid",
        session.client_id.as_deref().unwrap_or("0"),
    );
    insert(&mut vars, "user_type", &session.user_type);
    insert(
        &mut vars,
        "version_type",
        version.kind.as_deref().unwrap_or("release"),
    );
    insert(&mut vars, "user_properties", "{}");
    insert(&mut vars, "resolution_width", &opts.width.to_string());
    insert(&mut vars, "resolution_height", &opts.height.to_string());
    insert(
        &mut vars,
        "game_assets",
        &paths
            .assets_dir()
            .join("virtual")
            .join(&index_id)
            .to_string_lossy(),
    );

    let jvm_from_json = version.jvm_arguments(&env);

    let mut args: Vec<String> = vec![
        format!("-Xmx{}M", opts.max_memory_mb),
        format!("-Xms{}M", opts.min_memory_mb),
        // Serial GC keeps pause times and footprint down on 2-4 GB phones.
        "-XX:+UseSerialGC".to_string(),
        format!(
            "-Dminecraft.launcher.brand={LAUNCHER_NAME}"
        ),
        format!(
            "-Dminecraft.launcher.version={LAUNCHER_VERSION}"
        ),
    ];

    for arg in &jvm_from_json {
        args.push(fill(arg, &vars));
    }
    if !jvm_from_json
        .iter()
        .any(|a| a.contains("java.library.path"))
    {
        args.push(format!("-Djava.library.path={}", natives_dir.display()));
    }
    if !jvm_from_json
        .iter()
        .any(|a| a == "-cp" || a == "-classpath")
    {
        args.push("-cp".to_string());
        args.push(classpath_string);
    }
    args.extend(opts.extra_jvm_args.iter().cloned());
    args.push(version.main_class.clone());

    let mut game_args: Vec<String> = version
        .game_arguments(&env)
        .iter()
        .map(|arg| fill(arg, &vars))
        .collect();
    game_args.extend(opts.extra_game_args.iter().cloned());
    args.extend(game_args);

    Ok(LaunchPlan {
        program: opts
            .java
            .clone()
            .unwrap_or_else(|| PathBuf::from("java")),
        args,
        envs: Vec::new(),
        cwd: paths.root.clone(),
    })
}

fn insert(vars: &mut HashMap<String, String>, key: &str, value: &str) {
    vars.insert(key.to_string(), value.to_string());
}

/// Replaces `${placeholders}`; unknown placeholders are kept verbatim so
/// debugging a modded version JSON stays possible.
pub fn fill(template: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(value) => out.push_str(value),
                    None => {
                        out.push_str(&rest[start..start + 2 + end + 1]);
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_known_keys() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), "hello".to_string());
        assert_eq!(fill("${a} world", &vars), "hello world");
        assert_eq!(fill("${missing}", &vars), "${missing}");
        assert_eq!(fill("${a}${a}", &vars), "hellohello");
    }
}
