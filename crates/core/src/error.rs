use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("bad JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("launch failed: {0}")]
    Launch(String),

    #[error("missing file: {0}")]
    MissingFile(PathBuf),

    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}
