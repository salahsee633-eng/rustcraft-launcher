use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};

use crate::error::{Error, Result};

/// Shared blocking HTTP client. One connection pool for the whole app.
pub fn http_client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(crate::USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        // Assets are small; 120s is plenty and keeps a stuck socket from
        // holding a worker forever on mobile networks.
        .timeout(Duration::from_secs(120))
        .build()?)
}

pub fn get_text(url: &str) -> Result<String> {
    let client = http_client()?;
    let res = client.get(url).send()?;
    let status = res.status();
    let text = res.text()?;
    if !status.is_success() {
        return Err(Error::other(format!("HTTP {status} for {url}")));
    }
    Ok(text)
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let client = http_client()?;
    let res = client.get(url).send()?;
    let status = res.status();
    if !status.is_success() {
        return Err(Error::other(format!("HTTP {status} for {url}")));
    }
    Ok(res.bytes()?.to_vec())
}

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    Ok(serde_json::from_str(&get_text(url)?)?)
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 32 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// True when the file exists and (if a hash is known) matches it.
pub fn file_is_valid(path: &Path, expected_sha1: Option<&str>) -> bool {
    if !path.is_file() {
        return false;
    }
    match expected_sha1 {
        Some(expected) => sha1_file(path)
            .map(|actual| actual.eq_ignore_ascii_case(expected))
            .unwrap_or(false),
        None => true,
    }
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// Live counters the UI can poll while downloads happen on worker threads.
#[derive(Debug, Default)]
pub struct DownloadStats {
    pub done_bytes: AtomicU64,
    pub total_bytes: AtomicU64,
    pub done_files: AtomicUsize,
    pub total_files: AtomicUsize,
}

impl DownloadStats {
    pub fn progress(&self) -> f32 {
        let total = self.total_bytes.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (self.done_bytes.load(Ordering::Relaxed) as f32 / total as f32).clamp(0.0, 1.0)
    }
}

/// Downloads a list of files with a small worker pool and per-file resume
/// marker (`.part`). Finished files are verified against their SHA-1.
pub fn download_all(
    items: Vec<DownloadItem>,
    concurrency: usize,
    stats: &Arc<DownloadStats>,
) -> Result<()> {
    let queue = Arc::new(Mutex::new(VecDeque::from(items)));
    let mut handles = Vec::new();

    for _ in 0..concurrency.max(1) {
        let queue = Arc::clone(&queue);
        let stats = Arc::clone(stats);
        handles.push(std::thread::spawn(move || -> Result<()> {
            loop {
                let next = queue.lock().unwrap().pop_front();
                let Some(item) = next else { return Ok(()) };

                // Mobile networks and the asset CDN drop connections
                // occasionally; retry a few times before failing the install.
                let mut attempt = 0;
                loop {
                    match download_one(&item, &stats) {
                        Ok(()) => break,
                        Err(err) if attempt < 3 && !matches!(err, Error::HashMismatch { .. }) => {
                            attempt += 1;
                            std::thread::sleep(Duration::from_millis(300 * attempt));
                        }
                        Err(err) => return Err(err),
                    }
                }
                stats.done_files.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    let mut first_error = None;
    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
            Err(_) => {
                if first_error.is_none() {
                    first_error = Some(Error::other("download worker panicked"));
                }
            }
        }
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn download_one(item: &DownloadItem, stats: &DownloadStats) -> Result<()> {
    if file_is_valid(&item.dest, item.sha1.as_deref()) {
        let size = item.size.unwrap_or_else(|| {
            fs::metadata(&item.dest).map(|m| m.len()).unwrap_or(0)
        });
        stats.done_bytes.fetch_add(size, Ordering::Relaxed);
        return Ok(());
    }

    if let Some(parent) = item.dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let part = part_path(&item.dest);
    let client = http_client()?;
    let mut res = client.get(&item.url).send()?;
    let status = res.status();
    if !status.is_success() {
        return Err(Error::other(format!(
            "HTTP {status} while downloading {}",
            item.url
        )));
    }

    let mut file = File::create(&part)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = res.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        stats.done_bytes.fetch_add(n as u64, Ordering::Relaxed);
    }
    file.flush()?;
    drop(file);

    if let Some(expected) = &item.sha1 {
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = fs::remove_file(&part);
            return Err(Error::HashMismatch {
                path: item.dest.clone(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    fs::rename(&part, &item.dest)?;
    Ok(())
}

fn part_path(dest: &Path) -> PathBuf {
    let mut os = dest.as_os_str().to_owned();
    os.push(".part");
    PathBuf::from(os)
}

/// Ensures a remote JSON file is cached on disk (re-downloads on hash change).
pub fn ensure_small_file(url: &str, dest: &Path, sha1: Option<&str>, size: Option<u64>) -> Result<()> {
    if file_is_valid(dest, sha1) {
        return Ok(());
    }
    let bytes = get_bytes(url)?;
    if let Some(expected) = sha1 {
        let actual = hex::encode(Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(Error::HashMismatch {
                path: dest.to_path_buf(),
                expected: expected.to_string(),
                actual,
            });
        }
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, bytes)?;
    let _ = size;
    Ok(())
}
