use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;
use crate::paths::Paths;

/// Everything the game needs to know about the player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub username: String,
    /// Undashed UUID, as passed to `${auth_uuid}`.
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    #[serde(default)]
    pub xuid: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
}

impl Session {
    pub fn offline(username: &str) -> Self {
        Session {
            username: username.to_string(),
            uuid: Uuid::new_v4().simple().to_string(),
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
            xuid: None,
            client_id: None,
        }
    }
}

pub fn save(paths: &Paths, session: &Session) -> Result<()> {
    paths.ensure()?;
    let json = serde_json::to_vec_pretty(session)?;
    write_private(&paths.auth_file(), &json)
}

pub fn load(paths: &Paths) -> Result<Option<Session>> {
    let path = paths.auth_file();
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&text)?))
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
