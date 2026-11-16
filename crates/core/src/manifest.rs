use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::paths::Paths;
use crate::rules::{allowed, Env, Rule};

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const LAUNCHER_META_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub main_class: String,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub asset_index: Option<AssetIndex>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default, rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub release_time: Option<String>,
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub compliance_level: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
    #[serde(default)]
    pub server: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default = "default_java_major")]
    pub major_version: u32,
}

fn default_java_major() -> u32 {
    8
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
    pub id: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexFile {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        rules: Vec<Rule>,
        value: ArgumentValue,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MavenCoord<'a> {
    pub group: &'a str,
    pub artifact: &'a str,
    pub version: &'a str,
    pub classifier: Option<&'a str>,
}

/// Path of a specific classified artifact without borrowing the classifier
/// for the lifetime of the coordinate.
pub fn maven_path_with_classifier(name: &str, classifier: &str) -> Option<PathBuf> {
    let coord = MavenCoord::parse(name)?;
    let file = format!("{}-{}-{}.jar", coord.artifact, coord.version, classifier);
    Some(
        PathBuf::from(coord.group.replace('.', "/"))
            .join(coord.artifact)
            .join(coord.version)
            .join(file),
    )
}

impl<'a> MavenCoord<'a> {
    pub fn parse(name: &'a str) -> Option<Self> {
        let mut parts = name.split(':');
        let group = parts.next()?;
        let artifact = parts.next()?;
        let version = parts.next()?;
        let classifier = parts.next();
        Some(MavenCoord {
            group,
            artifact,
            version,
            classifier: classifier.filter(|c| !c.is_empty()),
        })
    }

    pub fn relative_path(&self) -> PathBuf {
        let file = match self.classifier {
            Some(classifier) => format!(
                "{}-{}-{}.jar",
                self.artifact, self.version, classifier
            ),
            None => format!("{}-{}.jar", self.artifact, self.version),
        };
        PathBuf::from(self.group.replace('.', "/"))
            .join(self.artifact)
            .join(self.version)
            .join(file)
    }
}

impl Library {
    pub fn is_allowed(&self, env: &Env) -> bool {
        allowed(&self.rules, env)
    }

    pub fn classpath_relative(&self) -> Option<PathBuf> {
        if let Some(downloads) = &self.downloads {
            // A `downloads` block without an artifact means native-only entry;
            // its classifier jar must not end up on the classpath.
            let artifact = downloads.artifact.as_ref()?;
            if let Some(path) = &artifact.path {
                return Some(PathBuf::from(path));
            }
            return MavenCoord::parse(&self.name).map(|coord| coord.relative_path());
        }
        MavenCoord::parse(&self.name).map(|coord| coord.relative_path())
    }

    pub fn artifact(&self) -> Option<&Artifact> {
        self.downloads.as_ref().and_then(|d| d.artifact.as_ref())
    }

    /// Classifier name for the current OS, e.g. `natives-linux`.
    pub fn native_classifier(&self, env: &Env) -> Option<String> {
        let natives = self.natives.as_ref()?;
        let classifier = natives.get(&env.os_name)?;
        let bits = if env.os_arch.contains("64") || env.os_arch == "aarch64" {
            "64"
        } else {
            "32"
        };
        Some(classifier.replace("${arch}", bits))
    }

    pub fn native_artifact(&self, env: &Env) -> Option<&Artifact> {
        let classifier = self.native_classifier(env)?;
        self.downloads.as_ref()?.classifiers.get(&classifier)
    }

    pub fn artifact_url(&self) -> Option<String> {
        if let Some(artifact) = self.artifact() {
            if !artifact.url.is_empty() {
                return Some(artifact.url.clone());
            }
        }
        let base = self.url.as_ref()?;
        let coord = MavenCoord::parse(&self.name)?;
        let base = base.trim_end_matches('/');
        Some(format!("{base}/{}", coord.relative_path().to_string_lossy()))
    }
}

impl VersionJson {
    pub fn jvm_arguments(&self, env: &Env) -> Vec<String> {
        self.arguments
            .as_ref()
            .map(|args| flatten_args(&args.jvm, env))
            .unwrap_or_default()
    }

    pub fn game_arguments(&self, env: &Env) -> Vec<String> {
        if let Some(args) = &self.arguments {
            return flatten_args(&args.game, env);
        }
        // Pre-1.13 versions use a single whitespace separated string.
        self.minecraft_arguments
            .as_deref()
            .map(|s| s.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    }

    pub fn required_java_major(&self) -> u32 {
        self.java_version
            .as_ref()
            .map(|j| j.major_version)
            .unwrap_or(8)
    }

    pub fn asset_index_id(&self) -> String {
        self.asset_index
            .as_ref()
            .map(|a| a.id.clone())
            .or_else(|| self.assets.clone())
            .unwrap_or_else(|| "legacy".to_string())
    }
}

pub fn flatten_args(args: &[Argument], env: &Env) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(s.clone()),
            Argument::Conditional { rules, value } => {
                if allowed(rules, env) {
                    match value {
                        ArgumentValue::One(s) => out.push(s.clone()),
                        ArgumentValue::Many(items) => out.extend(items.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

pub fn load_version_json(paths: &Paths, id: &str) -> Result<VersionJson> {
    let path = paths.version_json(id);
    if !path.is_file() {
        return Err(Error::MissingFile(path));
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// Resolves `inheritsFrom` chains (Fabric/Quilt/Forge) by merging JSON values:
/// libraries and argument lists are concatenated parent-first, scalars are
/// overridden by the child.
pub fn resolve_version(paths: &Paths, id: &str) -> Result<VersionJson> {
    let mut version: VersionJson = serde_json::from_value(resolve_value(paths, id)?)?;
    // The child id is authoritative; some modded JSONs omit it.
    if version.id.is_empty() {
        version.id = id.to_string();
    }
    Ok(version)
}

fn resolve_value(paths: &Paths, id: &str) -> Result<Value> {
    let path = paths.version_json(id);
    if !path.is_file() {
        return Err(Error::MissingFile(path));
    }
    let child: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;

    let mut merged = if let Some(parent_id) = child.get("inheritsFrom").and_then(Value::as_str) {
        resolve_value(paths, parent_id)?
    } else {
        Value::Object(Default::default())
    };
    merge_values(&mut merged, child);
    Ok(merged)
}

fn merge_values(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(base_value) => merge_values(base_value, overlay_value),
                    None => {
                        base_map.insert(key, overlay_value);
                    }
                }
            }
        }
        (Value::Array(base_items), Value::Array(overlay_items)) => {
            base_items.extend(overlay_items);
        }
        (base_value, overlay_value) => *base_value = overlay_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        let coord = MavenCoord::parse("org.lwjgl:lwjgl:3.3.3").unwrap();
        assert_eq!(
            coord.relative_path().to_string_lossy(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar"
        );

        let native = MavenCoord::parse("org.lwjgl:lwjgl:3.3.3:natives-linux").unwrap();
        assert_eq!(
            native.relative_path().to_string_lossy(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
        );
    }

    #[test]
    fn merges_library_lists() {
        let parent = serde_json::json!({
            "id": "1.20.1",
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{"name": "a:b:1"}],
            "arguments": {"game": ["--gameDir", "${game_directory}"], "jvm": ["-cp"]}
        });
        let child = serde_json::json!({
            "id": "fabric-loader-0.15",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{"name": "c:d:2"}],
            "arguments": {"game": ["--fabric"], "jvm": ["-DFabricMcEmu=net.minecraft.client.main.Main"]}
        });
        let mut merged = parent;
        merge_values(&mut merged, child);
        let version: VersionJson = serde_json::from_value(merged).unwrap();
        assert_eq!(version.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(version.libraries.len(), 2);
        assert_eq!(version.arguments.as_ref().unwrap().game.len(), 3);
        assert_eq!(version.required_java_major(), 8);
    }
}
