use std::fs::File;
use std::io;
use std::path::Path;

use sha1::{Digest, Sha1};
use std::sync::Arc;

use crate::download::{self, DownloadItem, DownloadStats};
use crate::error::{Error, Result};
use crate::manifest::{self, AssetIndexFile, MavenCoord, VersionEntry, VersionJson, VersionManifest};
use crate::paths::Paths;
use crate::rules::{allowed, Env};

pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// Android by default: no Mojang natives are extracted, the Android
    /// graphics stack ships its own.
    pub env: Env,
    pub concurrency: usize,
    pub extract_natives: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        InstallOptions {
            env: Env::android(),
            concurrency: 4,
            extract_natives: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InstallReport {
    pub files: usize,
    pub total_bytes: u64,
}

pub fn fetch_manifest() -> Result<VersionManifest> {
    download::get_json(manifest::VERSION_MANIFEST_URL)
}

pub fn find_version<'a>(manifest: &'a VersionManifest, id: &str) -> Option<&'a VersionEntry> {
    manifest.versions.iter().find(|v| v.id == id)
}

/// Downloads the version JSON (and its `inheritsFrom` parents) if needed.
pub fn ensure_version_json(paths: &Paths, manifest: &VersionManifest, id: &str) -> Result<()> {
    let entry = find_version(manifest, id).ok_or_else(|| {
        Error::other(format!("version {id} is not in the Mojang version manifest"))
    })?;
    let dest = paths.version_json(id);
    download::ensure_small_file(&entry.url, &dest, entry.sha1.as_deref(), None)?;

    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&dest)?)?;
    if let Some(parent) = value.get("inheritsFrom").and_then(serde_json::Value::as_str) {
        ensure_version_json(paths, manifest, parent)?;
    }
    Ok(())
}

/// Full install: version JSON, client jar, libraries, asset index and objects.
pub fn install(
    paths: &Paths,
    id: &str,
    opts: &InstallOptions,
) -> Result<InstallReport> {
    paths.ensure()?;
    let manifest = fetch_manifest()?;
    ensure_version_json(paths, &manifest, id)?;
    let version = manifest::resolve_version(paths, id)?;

    let mut items: Vec<DownloadItem> = Vec::new();

    // Client jar. Modded versions inherit the parent's jar, so don't duplicate
    // ~20 MB per modloader instance.
    if let Some(client) = version
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .filter(|_| !parent_jar_exists(paths, &version))
    {
        items.push(DownloadItem {
            url: client.url.clone(),
            dest: paths.version_jar(id),
            sha1: client.sha1.clone(),
            size: client.size,
        });
    }

    // Libraries + (optionally) desktop natives.
    for lib in &version.libraries {
        if !allowed(&lib.rules, &opts.env) {
            continue;
        }
        let coord = MavenCoord::parse(&lib.name);
        if let Some(artifact) = lib.artifact() {
            if artifact.url.is_empty() {
                continue;
            }
            let relative = artifact
                .path
                .clone()
                .map(std::path::PathBuf::from)
                .or_else(|| coord.as_ref().map(|c| c.relative_path()))
                .ok_or_else(|| Error::other(format!("bad library name: {}", lib.name)))?;
            items.push(DownloadItem {
                url: artifact.url.clone(),
                dest: paths.libraries_dir().join(relative),
                sha1: artifact.sha1.clone(),
                size: artifact.size,
            });
        } else if let Some(url) = lib.artifact_url() {
            if let Some(coord) = coord {
                items.push(DownloadItem {
                    url,
                    dest: paths.libraries_dir().join(coord.relative_path()),
                    sha1: None,
                    size: None,
                });
            }
        }

        if opts.extract_natives {
            if let Some(native) = lib.native_artifact(&opts.env) {
                if !native.url.is_empty() {
                    let relative = native
                        .path
                        .clone()
                        .map(std::path::PathBuf::from)
                        .or_else(|| {
                            let classifier = lib.native_classifier(&opts.env)?;
                            manifest::maven_path_with_classifier(&lib.name, &classifier)
                        });
                    if let Some(relative) = relative {
                        items.push(DownloadItem {
                            url: native.url.clone(),
                            dest: paths.libraries_dir().join(relative),
                            sha1: native.sha1.clone(),
                            size: native.size,
                        });
                    }
                }
            }
        }
    }

    // Asset index + every object it references.
    let index_id = version.asset_index_id();
    if let Some(index) = &version.asset_index {
        if !index.url.is_empty() {
            download::ensure_small_file(
                &index.url,
                &paths.assets_index(&index_id),
                index.sha1.as_deref(),
                index.size,
            )?;
        }
    }
    let index_path = paths.assets_index(&index_id);
    if index_path.is_file() {
        let index_file: AssetIndexFile =
            serde_json::from_str(&std::fs::read_to_string(&index_path)?)?;
        for object in index_file.objects.values() {
            let prefix = object.hash.get(0..2).unwrap_or("00");
            items.push(DownloadItem {
                url: format!("{RESOURCES_URL}/{prefix}/{}", object.hash),
                dest: paths.asset_object(&object.hash),
                sha1: Some(object.hash.clone()),
                size: Some(object.size),
            });
        }
    }

    let stats = Arc::new(DownloadStats::default());
    stats.total_files.store(items.len(), std::sync::atomic::Ordering::Relaxed);
    stats.total_bytes.store(
        items.iter().filter_map(|i| i.size).sum(),
        std::sync::atomic::Ordering::Relaxed,
    );
    let files = items.len();
    let total_bytes = items.iter().filter_map(|i| i.size).sum();

    download::download_all(items, opts.concurrency, &stats)?;

    if opts.extract_natives {
        extract_natives(paths, &version, &opts.env)?;
    }

    // Old (pre-1.7) versions expect a virtual asset tree. Copy/hard-link it.
    if index_id == "legacy" || index_id == "pre-1.6" {
        if let Err(err) = build_virtual_assets(paths, &index_path, &index_id) {
            eprintln!("warning: legacy assets could not be prepared: {err}");
        }
    }

    Ok(InstallReport { files, total_bytes })
}

fn parent_jar_exists(paths: &Paths, version: &VersionJson) -> bool {
    version
        .inherits_from
        .as_ref()
        .map(|parent| paths.version_jar(parent).is_file())
        .unwrap_or(false)
}

/// Walks the inheritance chain until it finds an actual client jar.
pub fn game_jar(paths: &Paths, version: &VersionJson) -> Result<std::path::PathBuf> {
    let mut id = version.id.clone();
    loop {
        let jar = paths.version_jar(&id);
        if jar.is_file() {
            return Ok(jar);
        }
        let current = manifest::load_version_json(paths, &id)?;
        match current.inherits_from {
            Some(parent) => id = parent,
            None => return Err(Error::MissingFile(jar)),
        }
    }
}

fn extract_natives(paths: &Paths, version: &VersionJson, env: &Env) -> Result<()> {
    let dest = paths.natives_dir(&version.id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::create_dir_all(&dest)?;

    for lib in &version.libraries {
        if !allowed(&lib.rules, env) {
            continue;
        }
        let Some(artifact) = lib.native_artifact(env) else {
            continue;
        };
        let relative = artifact
            .path
            .clone()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let classifier = lib.native_classifier(env)?;
                manifest::maven_path_with_classifier(&lib.name, &classifier)
            });
        let Some(relative) = relative else { continue };
        let jar = paths.libraries_dir().join(relative);
        if !jar.is_file() {
            return Err(Error::MissingFile(jar));
        }
        let excludes = lib
            .extract
            .as_ref()
            .map(|e| e.exclude.clone())
            .unwrap_or_else(|| vec!["META-INF/".into()]);
        extract_zip(&jar, &dest, &excludes)?;
    }
    Ok(())
}

fn extract_zip(jar: &Path, dest: &Path, exclude: &[String]) -> Result<()> {
    let file = File::open(jar)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let name = name.to_path_buf();
        let normalized = name.to_string_lossy().replace('\\', "/");
        if normalized.starts_with("META-INF/") {
            continue;
        }
        if exclude.iter().any(|prefix| normalized.starts_with(prefix)) {
            continue;
        }
        let out = dest.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut sink = File::create(&out)?;
        io::copy(&mut entry, &mut sink)?;
    }
    Ok(())
}

/// Some old versions read `assets/virtual/<index>/<name>` instead of hashed
/// objects. Hard-link when possible, copy otherwise.
fn build_virtual_assets(paths: &Paths, index_path: &Path, index_id: &str) -> Result<()> {
    let index: AssetIndexFile = serde_json::from_str(&std::fs::read_to_string(index_path)?)?;
    let virtual_dir = paths.assets_dir().join("virtual").join(index_id);
    for (name, object) in &index.objects {
        let src = paths.asset_object(&object.hash);
        if !src.is_file() {
            continue;
        }
        let dst = virtual_dir.join(name);
        if dst.is_file() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::hard_link(&src, &dst).is_err() {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Convenience for tests / tooling: hash a buffer.
pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}
