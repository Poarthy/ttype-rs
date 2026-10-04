use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{Paths, atomic_write};
use crate::domain::{RunResult, TextMode};

const HISTORY_LIMIT: usize = 1000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StoredResult {
    pub timestamp: String,
    pub mode: TextMode,
    pub tag: String,
    pub result: RunResultWire,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunResultWire {
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub correct: i32,
    pub incorrect: i32,
    pub total_chars: i32,
    pub failed: bool,
    pub duration_ms: u128,
}

impl From<&RunResult> for RunResultWire {
    fn from(result: &RunResult) -> Self {
        Self {
            wpm: result.wpm,
            raw_wpm: result.raw_wpm,
            accuracy: result.accuracy,
            consistency: result.consistency,
            correct: result.correct,
            incorrect: result.incorrect,
            total_chars: result.total_chars,
            failed: result.failed,
            duration_ms: result.duration.as_millis(),
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("history I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("history JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn history_path(paths: &Paths) -> PathBuf {
    paths.data.join("history.json")
}

pub fn load_history(paths: &Paths) -> Result<Vec<StoredResult>, StorageError> {
    match fs::read(history_path(paths)) {
        Ok(contents) => Ok(serde_json::from_slice(&contents)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(StorageError::Io(error)),
    }
}

pub fn save_result(paths: &Paths, result: &RunResult) -> Result<(), StorageError> {
    fs::create_dir_all(&paths.data)?;
    let mut history = load_history(paths)?;
    history.push(StoredResult {
        timestamp: format!("{:?}", std::time::SystemTime::now()),
        mode: result.config.text_mode,
        tag: result.config.tag.clone(),
        result: RunResultWire::from(result),
    });
    if history.len() > HISTORY_LIMIT {
        history.drain(..history.len() - HISTORY_LIMIT);
    }
    let body = serde_json::to_vec_pretty(&history)?;
    atomic_write(&history_path(paths), &body)?;
    Ok(())
}

pub fn clear(paths: &Paths, target: crate::cli::ClearTarget) -> Result<(), StorageError> {
    match target {
        crate::cli::ClearTarget::History => remove_history(paths)?,
        crate::cli::ClearTarget::Languages => remove_languages(paths)?,
        crate::cli::ClearTarget::All => {
            remove_history(paths)?;
            remove_languages(paths)?;
        }
    }
    Ok(())
}

fn remove_history(paths: &Paths) -> Result<(), StorageError> {
    for name in ["history.json", "bests.json", "replays"] {
        let path = paths.data.join(name);
        if path.is_dir() {
            match fs::remove_dir_all(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(StorageError::Io(error)),
            }
        } else {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(StorageError::Io(error)),
            }
        }
    }
    Ok(())
}
fn remove_languages(paths: &Paths) -> Result<(), StorageError> {
    let path = paths.data.join("languages");
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StorageError::Io(error)),
    }
}
