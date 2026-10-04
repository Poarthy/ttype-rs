use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("network: {0}")]
    Network(String),
    #[error("checksum missing or mismatched")]
    Checksum,
    #[error("archive extraction is unavailable")]
    Archive,
    #[error("update I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateState {
    pub last_check_unix: u64,
    pub latest: String,
    pub installed: String,
    pub last_install_unix: u64,
}

pub fn should_check(state: &UpdateState, now: u64) -> bool {
    now.saturating_sub(state.last_check_unix) >= Duration::from_secs(24 * 60 * 60).as_secs()
}

pub fn load_state(path: &Path) -> UpdateState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}
pub fn save_state(path: &Path, state: &UpdateState) -> Result<(), UpdateError> {
    let content = serde_json::to_vec_pretty(state)
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| UpdateError::Network("state has no parent directory".to_owned()))?;
    std::fs::create_dir_all(parent)?;
    super_atomic_write(path, &content)
}
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

pub fn newer_version(latest: &str, local: &str) -> bool {
    if latest.is_empty() {
        return false;
    }
    if local == "dev" {
        return true;
    }
    let parse = |version: &str| {
        version
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let latest = parse(latest);
    let local = parse(local);
    for index in 0..latest.len().max(local.len()) {
        let next = latest.get(index).copied().unwrap_or(0);
        let current = local.get(index).copied().unwrap_or(0);
        if next != current {
            return next > current;
        }
    }
    false
}

pub fn verify_checksum(bytes: &[u8], asset_name: &str, checksums: &str) -> Result<(), UpdateError> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    if checksums.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some(digest.as_str()) && fields.next() == Some(asset_name)
    }) {
        Ok(())
    } else {
        Err(UpdateError::Checksum)
    }
}

/// Writes, syncs, and renames a sibling file so a failed download cannot
/// corrupt the executable currently in use.
pub fn atomic_replace(executable: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let temporary = executable.with_extension("ttype-update-tmp");
    fs::write(&temporary, bytes)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&temporary)?;
    file.sync_all()?;
    fs::rename(&temporary, executable)?;
    Ok(())
}

fn super_atomic_write(path: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}

pub fn download(url: &str) -> Result<Vec<u8>, UpdateError> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let mut reader = response.into_body().into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}
