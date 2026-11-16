use std::path::{Path, PathBuf};

use crate::error::Result;

/// Root of a Minecraft installation ("game directory").
///
/// Android: `<app files dir>/.minecraft`
/// Desktop: `~/.rustcraft`
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Paths { root: root.into() }
    }

    pub fn android(files_dir: impl AsRef<Path>) -> Self {
        Paths::new(files_dir.as_ref().join(".minecraft"))
    }

    pub fn desktop() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Paths::new(home.join(".rustcraft"))
    }

    pub fn ensure(&self) -> Result<()> {
        for dir in [
            self.root.clone(),
            self.versions_dir(),
            self.libraries_dir(),
            self.assets_dir(),
            self.assets_indexes_dir(),
            self.assets_objects_dir(),
            self.instances_dir(),
            self.java_dir(),
            self.logs_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions_dir().join(id)
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn assets_indexes_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    pub fn assets_index(&self, id: &str) -> PathBuf {
        self.assets_indexes_dir().join(format!("{id}.json"))
    }

    pub fn assets_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    pub fn asset_object(&self, hash: &str) -> PathBuf {
        let prefix = hash.get(0..2).unwrap_or("00");
        self.assets_objects_dir().join(prefix).join(hash)
    }

    pub fn natives_dir(&self, version_id: &str) -> PathBuf {
        self.root.join("natives").join(version_id)
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn java_dir(&self) -> PathBuf {
        self.root.join("java")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn auth_file(&self) -> PathBuf {
        self.root.join("auth.json")
    }
}
