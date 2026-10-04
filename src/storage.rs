use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{Paths, atomic_write};
use crate::domain::{RunResult, TextMode};
use crate::replay::{Replay, ReplayEvent, ReplayEventKind};

const HISTORY_LIMIT: usize = 1000;
const REPLAY_MAGIC: &[u8] = b"TTRP";

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
    #[error("corrupt replay")]
    CorruptReplay,
}

/// Go-compatible replay v1: TTRP, version, ULEB sizes, delta milliseconds.
pub fn encode_replay(replay: &Replay) -> Vec<u8> {
    let mut data = Vec::from(REPLAY_MAGIC);
    data.push(1);
    put_varint(&mut data, replay.target.len() as u64);
    data.extend_from_slice(replay.target.as_bytes());
    put_varint(&mut data, replay.events.len() as u64);
    let mut previous = 0_u128;
    for event in &replay.events {
        data.push(match event.kind {
            ReplayEventKind::Rune => 0,
            ReplayEventKind::Backspace => 1,
            ReplayEventKind::DeleteWord => 2,
        });
        let milliseconds = event.offset.as_millis();
        put_varint(&mut data, milliseconds.saturating_sub(previous) as u64);
        previous = milliseconds;
        if let Some(character) = event.character {
            put_varint(&mut data, character as u32 as u64);
        }
    }
    data
}

pub fn decode_replay(data: &[u8]) -> Result<Replay, StorageError> {
    if data.get(..4) != Some(REPLAY_MAGIC) || data.get(4) != Some(&1) {
        return Err(StorageError::CorruptReplay);
    }
    let mut index = 5;
    let length = read_varint(data, &mut index)? as usize;
    let target = std::str::from_utf8(
        data.get(index..index.saturating_add(length))
            .ok_or(StorageError::CorruptReplay)?,
    )
    .map_err(|_| StorageError::CorruptReplay)?
    .to_owned();
    index += length;
    let count = read_varint(data, &mut index)? as usize;
    let mut events = Vec::with_capacity(count);
    let mut milliseconds = 0_u64;
    for _ in 0..count {
        let tag = *data.get(index).ok_or(StorageError::CorruptReplay)?;
        index += 1;
        milliseconds = milliseconds.saturating_add(read_varint(data, &mut index)?);
        let (kind, character) = match tag {
            0 => (
                ReplayEventKind::Rune,
                char::from_u32(read_varint(data, &mut index)? as u32),
            ),
            1 => (ReplayEventKind::Backspace, None),
            2 => (ReplayEventKind::DeleteWord, None),
            _ => return Err(StorageError::CorruptReplay),
        };
        if tag == 0 && character.is_none() {
            return Err(StorageError::CorruptReplay);
        }
        events.push(ReplayEvent {
            offset: std::time::Duration::from_millis(milliseconds),
            kind,
            character,
        });
    }
    Ok(Replay { target, events })
}

fn put_varint(data: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        data.push((value as u8) | 128);
        value >>= 7;
    }
    data.push(value as u8);
}
fn read_varint(data: &[u8], index: &mut usize) -> Result<u64, StorageError> {
    let mut value = 0;
    for shift in (0..64).step_by(7) {
        let byte = *data.get(*index).ok_or(StorageError::CorruptReplay)?;
        *index += 1;
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    Err(StorageError::CorruptReplay)
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
