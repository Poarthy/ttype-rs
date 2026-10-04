use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Archive;
use thiserror::Error;

pub const RELEASES_URL: &str = "https://github.com/alirezaudev/ttype/releases";
const CHECK_INTERVAL: u64 = 24 * 60 * 60;
const INSTALL_RETRY: u64 = 60 * 60;
const MAX_DOWNLOAD_BYTES: usize = 64 << 20;
const TEMP_PREFIX: &str = ".ttype-update-";
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const STALL_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DOWNLOAD_TIME: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("network: {0}")]
    Network(String),
    #[error("checksum missing or mismatched")]
    Checksum,
    #[error("archive has no ttype binary")]
    Archive,
    #[error("ttype was built from source; pull and rebuild it to update")]
    SourceBuild,
    #[error("ttype was installed by a package manager; update it there")]
    PackageManaged,
    #[error("ttype cannot update itself here; run {0}")]
    ManualInstall(String),
    #[error("new binary did not run: {0}")]
    Verification(String),
    #[error("update I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateState {
    pub last_check_unix: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub latest: String,
    pub last_install_unix: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub installed: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallKind {
    ReleaseScript,
    PackageManager,
    Source,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutomaticUpdateMode {
    Auto,
    Notify,
    Off,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AutoUpdateNotice {
    StartingInstall(String),
    Available(String),
    Installed(String),
}

/// `TTYPE_UPDATE` takes precedence over the saved setting. Unknown values use
/// the next source, and an unknown or empty saved setting means auto.
pub fn automatic_update_mode(setting: &str, environment: Option<&str>) -> AutomaticUpdateMode {
    fn parse(value: &str) -> Option<AutomaticUpdateMode> {
        match value {
            "auto" => Some(AutomaticUpdateMode::Auto),
            "notify" => Some(AutomaticUpdateMode::Notify),
            "off" => Some(AutomaticUpdateMode::Off),
            _ => None,
        }
    }
    environment
        .and_then(parse)
        .or_else(|| parse(setting))
        .unwrap_or(AutomaticUpdateMode::Auto)
}

/// Starts the same daily release check used by the Go TUI without adding an
/// async runtime. The Ratatui loop remains single-threaded and polls the
/// receiver for a completed status notice.
pub fn spawn_auto_update(
    state_path: PathBuf,
    local: String,
    mode: AutomaticUpdateMode,
) -> Option<Receiver<AutoUpdateNotice>> {
    if mode == AutomaticUpdateMode::Off {
        return None;
    }
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok(state) = check_daily(&state_path, &local, RELEASES_URL) else {
            return;
        };
        if state.latest.is_empty() {
            return;
        }
        let latest = state.latest;
        let Ok(plan) = InstallPlan::current() else {
            let _ = sender.send(AutoUpdateNotice::Available(latest));
            return;
        };
        if mode == AutomaticUpdateMode::Auto && plan.require_owned(&latest, &local).is_ok() {
            let _ = sender.send(AutoUpdateNotice::StartingInstall(latest.clone()));
            if install(
                &plan.executable,
                &local,
                &latest,
                RELEASES_URL,
                &plan,
                false,
                &state_path,
            )
            .is_ok()
            {
                let _ = sender.send(AutoUpdateNotice::Installed(latest));
                return;
            }
        }
        let _ = sender.send(AutoUpdateNotice::Available(latest));
    });
    Some(receiver)
}

#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub kind: InstallKind,
    pub executable: PathBuf,
    pub platform: String,
}

impl InstallPlan {
    pub fn current() -> Result<Self, UpdateError> {
        let executable = std::env::current_exe()?;
        let executable = fs::canonicalize(&executable).unwrap_or(executable);
        let path = executable
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        let kind = if path.contains("/cellar/")
            || path.contains("/scoop/apps/")
            || under_system_dir(&path)
        {
            InstallKind::PackageManager
        } else if option_env!("TTYPE_RELEASE") == Some("1") {
            InstallKind::ReleaseScript
        } else {
            InstallKind::Source
        };
        Ok(Self {
            kind,
            executable,
            platform: platform_suffix(),
        })
    }

    pub fn require_owned(&self, latest: &str, local: &str) -> Result<(), UpdateError> {
        match self.kind {
            InstallKind::PackageManager => Err(UpdateError::PackageManaged),
            InstallKind::Source => Err(UpdateError::SourceBuild),
            InstallKind::ReleaseScript if cfg!(windows) || !same_major(latest, local) => {
                Err(UpdateError::ManualInstall("ttype update".to_owned()))
            }
            InstallKind::ReleaseScript if !directory_writable(self.executable.parent()) => {
                Err(UpdateError::ManualInstall("sudo ttype update".to_owned()))
            }
            InstallKind::ReleaseScript => Ok(()),
        }
    }

    pub fn require_owned_explicit(&self) -> Result<(), UpdateError> {
        match self.kind {
            InstallKind::PackageManager => Err(UpdateError::PackageManaged),
            InstallKind::Source => Err(UpdateError::SourceBuild),
            InstallKind::ReleaseScript => Ok(()),
        }
    }
}

pub fn should_check(state: &UpdateState, now: u64) -> bool {
    now < state.last_check_unix || now.saturating_sub(state.last_check_unix) >= CHECK_INTERVAL
}

/// A missing/corrupt state only causes an earlier, best-effort check.
pub fn load_state(path: &Path) -> UpdateState {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_state(path: &Path, state: &UpdateState) -> Result<(), UpdateError> {
    let contents = serde_json::to_vec_pretty(state)
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| UpdateError::Network("state has no parent directory".to_owned()))?;
    fs::create_dir_all(parent)?;
    atomic_write(path, &contents)
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

/// Records the attempt before networking, matching Go's offline daily backoff.
pub fn check_daily(path: &Path, local: &str, base: &str) -> Result<UpdateState, UpdateError> {
    let now = now_unix();
    let mut state = load_state(path);
    if !should_check(&state, now) {
        return Ok(state);
    }
    state.last_check_unix = now;
    save_state(path, &state)?;
    if let Ok(latest) = latest_release(base) {
        state.latest = latest;
        save_state(path, &state)?;
    }
    // `UpdateState` doubles as the command-facing version information.
    // Keep a cached older release on disk only when it can still update this
    // binary, just as Go's VersionInfo suppresses stale update notices.
    if !newer_version(&state.latest, local) {
        state.latest.clear();
    }
    Ok(state)
}

/// Reads the redirect target from `/latest` without following it.
pub fn latest_release(base: &str) -> Result<String, UpdateError> {
    let config = ureq::config::Config::builder()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_global(Some(CHECK_TIMEOUT))
        .timeout_recv_response(Some(CHECK_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let response = agent
        .head(format!("{}/latest", base.trim_end_matches('/')))
        .header("User-Agent", "ttype")
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let status = response.status().as_u16();
    if !(300..400).contains(&status) {
        return Err(UpdateError::Network(format!(
            "check releases: HTTP {status}"
        )));
    }
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| UpdateError::Network("no release published yet".to_owned()))?;
    let (_, tag) = location
        .rsplit_once("/releases/tag/")
        .ok_or_else(|| UpdateError::Network("no release published yet".to_owned()))?;
    let version = tag.trim_start_matches('v');
    if version_parts(version).is_none() {
        return Err(UpdateError::Network("no release published yet".to_owned()));
    }
    Ok(version.to_owned())
}

pub fn newer_version(latest: &str, local: &str) -> bool {
    let Some(latest) = version_parts(latest) else {
        return false;
    };
    let Some(local) = version_parts(local) else {
        return !latest.is_empty();
    };
    for index in 0..latest.len().min(local.len()) {
        let remote = latest.get(index).copied().unwrap_or(0);
        let current = local.get(index).copied().unwrap_or(0);
        if remote != current {
            return remote > current;
        }
    }
    latest.len() > local.len()
}

fn version_parts(version: &str) -> Option<Vec<u64>> {
    let version = version.trim().trim_start_matches('v');
    if version.is_empty() {
        return None;
    }
    version
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u64>, _>>()
        .ok()
}

fn same_major(a: &str, b: &str) -> bool {
    match (version_parts(a), version_parts(b)) {
        (Some(a), Some(b)) => a.first() == b.first(),
        _ => false,
    }
}

pub fn asset_name(version: &str) -> String {
    format!("ttype_{version}_{}.tar.gz", platform_suffix())
}
fn platform_suffix() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    format!("{os}_{arch}")
}
fn binary_name() -> &'static str {
    if cfg!(windows) { "ttype.exe" } else { "ttype" }
}
fn under_system_dir(path: &str) -> bool {
    [
        "/usr/bin/",
        "/usr/sbin/",
        "/bin/",
        "/sbin/",
        "/nix/store/",
        "/snap/",
    ]
    .iter()
    .any(|root| path.starts_with(root))
}
fn directory_writable(directory: Option<&Path>) -> bool {
    let Some(directory) = directory else {
        return false;
    };
    let probe = directory.join(format!("{TEMP_PREFIX}probe-{}", now_unix()));
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

pub fn install(
    path: &Path,
    local: &str,
    latest: &str,
    base: &str,
    plan: &InstallPlan,
    asked: bool,
    state_path: &Path,
) -> Result<(), UpdateError> {
    if !newer_version(latest, local) {
        return Ok(());
    }
    if asked {
        plan.require_owned_explicit()?;
    } else {
        plan.require_owned(latest, local)?;
    }
    let now = now_unix();
    let state = load_state(state_path);
    if state.installed == latest {
        return Ok(());
    }
    if !asked
        && now >= state.last_install_unix
        && now.saturating_sub(state.last_install_unix) < INSTALL_RETRY
    {
        return Err(UpdateError::Network(
            "tried to install less than an hour ago".to_owned(),
        ));
    }
    let result = install_release(path, latest, base, &plan.platform);
    let mut state = load_state(state_path);
    state.last_install_unix = now;
    if result.is_ok() {
        state.installed = latest.to_owned();
    }
    save_state(state_path, &state)?;
    result
}

fn install_release(
    executable: &Path,
    version: &str,
    base: &str,
    platform: &str,
) -> Result<(), UpdateError> {
    let name = format!("ttype_{version}_{platform}.tar.gz");
    let release = format!("{}/download/v{version}", base.trim_end_matches('/'));
    let archive = download_limited(&format!("{release}/{name}"))?;
    let checksums = download_limited(&format!("{release}/checksums.txt"))?;
    verify_checksum(&archive, &name, &String::from_utf8_lossy(&checksums))?;
    let binary = extract_binary(&archive)?;
    replace_verified(executable, &binary, version)
}

pub fn download(url: &str) -> Result<Vec<u8>, UpdateError> {
    download_limited(url)
}
fn download_limited(url: &str) -> Result<Vec<u8>, UpdateError> {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(MAX_DOWNLOAD_TIME))
        .timeout_recv_body(Some(STALL_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let response = agent
        .get(url)
        .header("User-Agent", "ttype")
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    if response.status().as_u16() != 200 {
        return Err(UpdateError::Network(format!(
            "download: HTTP {}",
            response.status()
        )));
    }
    let mut reader = response
        .into_body()
        .into_reader()
        .take((MAX_DOWNLOAD_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err(UpdateError::Network("download exceeds 64 MiB".to_owned()));
    }
    Ok(bytes)
}

pub fn verify_checksum(bytes: &[u8], asset_name: &str, checksums: &str) -> Result<(), UpdateError> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    if checksums.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some(digest.as_str())
            && fields
                .next()
                .is_some_and(|name| name.trim_start_matches('*') == asset_name)
    }) {
        Ok(())
    } else {
        Err(UpdateError::Checksum)
    }
}

pub fn extract_binary(archive: &[u8]) -> Result<Vec<u8>, UpdateError> {
    let decoder = GzDecoder::new(Cursor::new(archive));
    let mut tar = Archive::new(decoder);
    for entry in tar
        .entries()
        .map_err(|error| UpdateError::Network(format!("open archive: {error}")))?
    {
        let entry =
            entry.map_err(|error| UpdateError::Network(format!("read archive: {error}")))?;
        let is_binary = entry.header().entry_type().is_file()
            && entry
                .path()
                .ok()
                .and_then(|path| path.file_name().map(|name| name == binary_name()))
                .unwrap_or(false);
        if !is_binary {
            continue;
        }
        let mut contents = Vec::new();
        entry
            .take((MAX_DOWNLOAD_BYTES + 1) as u64)
            .read_to_end(&mut contents)?;
        if contents.len() > MAX_DOWNLOAD_BYTES {
            return Err(UpdateError::Archive);
        }
        return Ok(contents);
    }
    Err(UpdateError::Archive)
}

/// Writes, verifies, syncs, then renames a sibling file. The old executable
/// remains untouched whenever downloading or verification fails.
pub fn atomic_replace(executable: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let parent = executable
        .parent()
        .ok_or_else(|| UpdateError::Network("executable has no parent".to_owned()))?;
    let temporary = parent.join(format!("{TEMP_PREFIX}{}", now_unix()));
    write_sync(&temporary, bytes)?;
    fs::rename(&temporary, executable)?;
    Ok(())
}

fn replace_verified(executable: &Path, bytes: &[u8], version: &str) -> Result<(), UpdateError> {
    remove_stale_temps(executable.parent());
    let parent = executable
        .parent()
        .ok_or_else(|| UpdateError::Network("executable has no parent".to_owned()))?;
    let temporary = parent.join(format!("{TEMP_PREFIX}{}", now_unix()));
    write_sync(&temporary, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))?;
    }
    let verification = runs_as_version(&temporary, version);
    if let Err(error) = verification {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    #[cfg(windows)]
    {
        let old = executable.with_extension("old");
        let _ = fs::remove_file(&old);
        fs::rename(executable, &old)?;
        if let Err(error) = fs::rename(&temporary, executable) {
            let _ = fs::rename(&old, executable);
            return Err(UpdateError::Io(error));
        }
    }
    #[cfg(not(windows))]
    fs::rename(&temporary, executable)?;
    Ok(())
}

fn write_sync(path: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn runs_as_version(path: &Path, version: &str) -> Result<(), UpdateError> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|error| UpdateError::Verification(error.to_string()))?;
    if !output.status.success() || !String::from_utf8_lossy(&output.stdout).contains(version) {
        return Err(UpdateError::Verification(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(())
}

fn remove_stale_temps(parent: Option<&Path>) {
    let Some(parent) = parent else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let old = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > Duration::from_secs(3600));
        if old
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(TEMP_PREFIX))
        {
            let _ = fs::remove_file(path);
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}
