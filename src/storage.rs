use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{Paths, atomic_write};
use crate::domain::{RunResult, TestConfig, TestKind, TextMode, WordResult};
use crate::lock::lock_directory;
use crate::replay::{Replay, ReplayEvent, ReplayEventKind};

const HISTORY_LIMIT: usize = 1000;
const REPLAY_MAGIC: &[u8] = b"TTRP";
const REPLAY_LIMIT: usize = 50;
static NEXT_RESULT_ID: AtomicU64 = AtomicU64::new(0);

/// The flat, additive JSON format written by Go's `storedResult`. Keeping the
/// field names and optional fields makes an existing Go history usable here.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct StoredResult {
    pub id: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub test_kind: String,
    #[serde(default)]
    pub duration_sec: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub word_count: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text_mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub theme: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub punctuation: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub numbers: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub blind: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub zen: bool,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub width: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub min_wpm: i32,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub seed: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,
    #[serde(default)]
    pub wpm: f64,
    #[serde(default)]
    pub raw_wpm: f64,
    #[serde(default)]
    pub accuracy: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub consistency: f64,
    #[serde(default)]
    pub correct: i32,
    #[serde(default)]
    pub incorrect: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub keystrokes_correct: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub keystrokes_incorrect: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub skipped: i32,
    #[serde(default)]
    pub total_chars: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wpm_history: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_wpm_history: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_history: Vec<i32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub char_errors: BTreeMap<String, i32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub failed: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub failure_reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missed_words: Vec<WordResult>,
}

impl StoredResult {
    pub fn text_mode(&self) -> TextMode {
        self.text_mode.parse().unwrap_or(TextMode::Words)
    }

    pub fn kind(&self) -> TestKind {
        if self.test_kind == "words" {
            TestKind::Words
        } else {
            TestKind::Timed
        }
    }

    pub fn test_label(&self) -> String {
        if self.word_count > 0 {
            format!("{}w", self.word_count)
        } else {
            format!("{}s", self.duration_sec)
        }
    }

    pub fn test_config(&self) -> TestConfig {
        TestConfig {
            kind: self.kind(),
            duration: Duration::from_secs(self.duration_sec),
            word_count: self.word_count,
            text_mode: self.text_mode(),
            language: self.language.clone(),
            theme: if self.theme.is_empty() {
                "default".to_owned()
            } else {
                self.theme.clone()
            },
            width: self.width.max(0) as usize,
            punctuation: self.punctuation,
            numbers: self.numbers,
            blind: self.blind,
            zen: self.zen,
            min_wpm: self.min_wpm,
            seed: self.seed,
            tag: self.tag.clone(),
        }
    }

    fn from_result(id: String, timestamp: String, result: &RunResult) -> Self {
        let config = &result.config;
        let duration_sec = if config.is_words_mode() {
            result.duration.as_secs()
        } else {
            config.duration.as_secs()
        };
        Self {
            id,
            timestamp,
            test_kind: match config.kind {
                TestKind::Timed => "timed".to_owned(),
                TestKind::Words => "words".to_owned(),
            },
            duration_sec,
            word_count: config.word_count,
            text_mode: config.text_mode.as_str().to_owned(),
            language: config.language.clone(),
            theme: config.theme.clone(),
            punctuation: config.punctuation,
            numbers: config.numbers,
            blind: config.blind,
            zen: config.zen,
            width: config.width as i32,
            min_wpm: config.min_wpm,
            seed: result.seed,
            tag: config.tag.clone(),
            wpm: result.wpm,
            raw_wpm: result.raw_wpm,
            accuracy: result.accuracy,
            consistency: result.consistency,
            correct: result.correct,
            incorrect: result.incorrect,
            keystrokes_correct: result.keystrokes_correct,
            keystrokes_incorrect: result.keystrokes_incorrect,
            skipped: result.skipped,
            total_chars: result.total_chars,
            wpm_history: result.wpm_history.clone(),
            raw_wpm_history: result.raw_wpm_history.clone(),
            error_history: result.error_history.clone(),
            char_errors: result.char_errors.clone(),
            failed: result.failed,
            failure_reason: result.failure_reason.clone(),
            missed_words: result
                .words
                .iter()
                .filter(|word| word.missed)
                .cloned()
                .collect(),
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
fn is_zero(value: &usize) -> bool {
    *value == 0
}
fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}
fn is_zero_i64(value: &i64) -> bool {
    *value == 0
}
fn is_zero_f64(value: &f64) -> bool {
    *value == 0.0
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PersonalBests {
    pub best_wpm: f64,
    pub best_accuracy: f64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct StatsSummary {
    pub total_tests: usize,
    pub filter_mode: String,
    pub filter_tag: String,
    pub average_wpm: f64,
    pub average_accuracy: f64,
    pub best_wpm: f64,
    pub recent_average_wpm: f64,
    pub personal_best: PersonalBests,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("history I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("history JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("corrupt replay")]
    CorruptReplay,
    #[error("invalid replay id")]
    InvalidReplayId,
    #[error("replay has no events")]
    EmptyReplay,
}

pub fn history_path(paths: &Paths) -> PathBuf {
    paths.data.join("history.json")
}
pub fn bests_path(paths: &Paths) -> PathBuf {
    paths.data.join("bests.json")
}

/// Reads storage order (oldest first), matching Go's on-disk history.
pub fn load_history(paths: &Paths) -> Result<Vec<StoredResult>, StorageError> {
    match fs::read(history_path(paths)) {
        Ok(contents) => Ok(serde_json::from_slice(&contents)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(StorageError::Io(error)),
    }
}

/// Lists newest first, as the Go `ListResults` store method does.
pub fn list_results(paths: &Paths, limit: usize) -> Result<Vec<StoredResult>, StorageError> {
    let mut history = load_history(paths)?;
    if limit > 0 && history.len() > limit {
        history = history.split_off(history.len() - limit);
    }
    history.reverse();
    Ok(history)
}

pub fn save_result(paths: &Paths, result: &RunResult) -> Result<String, StorageError> {
    let _lock = lock_directory(&paths.data)?;
    let mut history = load_history(paths)?;
    let id = if result.id.is_empty() {
        new_result_id()
    } else {
        result.id.clone()
    };
    let timestamp = if result.timestamp.is_empty() {
        rfc3339_now()
    } else {
        result.timestamp.clone()
    };
    let stored = StoredResult::from_result(id.clone(), timestamp, result);
    history.push(stored.clone());
    if history.len() > HISTORY_LIMIT {
        history.drain(..history.len() - HISTORY_LIMIT);
    }
    atomic_write(&history_path(paths), &serde_json::to_vec_pretty(&history)?)?;
    update_bests(paths, &stored)?;
    Ok(id)
}

fn new_result_id() -> String {
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    let sequence = NEXT_RESULT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{:016x}", clock ^ sequence)
}

pub fn load_bests(paths: &Paths) -> Result<PersonalBests, StorageError> {
    match fs::read(bests_path(paths)) {
        Ok(contents) => Ok(serde_json::from_slice(&contents)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PersonalBests::default()),
        Err(error) => Err(StorageError::Io(error)),
    }
}

fn update_bests(paths: &Paths, result: &StoredResult) -> Result<(), StorageError> {
    if result.failed || result.text_mode() == TextMode::Custom {
        return Ok(());
    }
    let mut bests = load_bests(paths)?;
    let mut changed = false;
    if result.wpm > bests.best_wpm {
        bests.best_wpm = result.wpm;
        changed = true;
    }
    if result.accuracy > bests.best_accuracy {
        bests.best_accuracy = result.accuracy;
        changed = true;
    }
    if changed {
        bests.updated_at = rfc3339_now();
        atomic_write(&bests_path(paths), &serde_json::to_vec_pretty(&bests)?)?;
    }
    Ok(())
}

pub fn filter_results(
    mut results: Vec<StoredResult>,
    mode: Option<TextMode>,
    tag: Option<&str>,
    exclude_failed: bool,
) -> Vec<StoredResult> {
    results.retain(|result| {
        mode.is_none_or(|needle| result.text_mode() == needle)
            && tag.is_none_or(|needle| result.tag == needle)
            && (!exclude_failed || !result.failed)
    });
    results
}

pub fn summarize(
    paths: &Paths,
    mode: Option<TextMode>,
    tag: Option<&str>,
    exclude_failed: bool,
) -> Result<StatsSummary, StorageError> {
    let records = filter_results(load_history(paths)?, mode, tag, exclude_failed);
    let personal_best = load_bests(paths)?;
    let mut summary = StatsSummary {
        total_tests: records.len(),
        filter_mode: mode.map_or_else(String::new, |value| value.to_string()),
        filter_tag: tag.unwrap_or_default().to_owned(),
        personal_best,
        ..StatsSummary::default()
    };
    if records.is_empty() {
        return Ok(summary);
    }

    summary.average_wpm =
        round2(records.iter().map(|item| item.wpm).sum::<f64>() / records.len() as f64);
    summary.average_accuracy =
        round2(records.iter().map(|item| item.accuracy).sum::<f64>() / records.len() as f64);
    let only_custom = mode == Some(TextMode::Custom);
    summary.best_wpm = records
        .iter()
        .filter(|item| only_custom || item.text_mode() != TextMode::Custom)
        .map(|item| item.wpm)
        .fold(0.0, f64::max);
    let recent_start = records.len().saturating_sub(10);
    summary.recent_average_wpm = round2(
        records[recent_start..]
            .iter()
            .map(|item| item.wpm)
            .sum::<f64>()
            / records[recent_start..].len() as f64,
    );
    Ok(summary)
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn export_csv(
    paths: &Paths,
    mode: Option<TextMode>,
    tag: Option<&str>,
    exclude_failed: bool,
) -> Result<String, StorageError> {
    let rows = filter_results(load_history(paths)?, mode, tag, exclude_failed);
    let mut output = String::from(
        "timestamp,mode,language,test,wpm,raw_wpm,accuracy,consistency,errors,failed,tag\n",
    );
    for row in rows {
        let test_label = row.test_label();
        let fields = [
            row.timestamp,
            row.text_mode,
            row.language,
            test_label,
            format!("{:.2}", row.wpm),
            format!("{:.2}", row.raw_wpm),
            format!("{:.2}", row.accuracy),
            format!("{:.2}", row.consistency),
            row.keystrokes_incorrect.to_string(),
            row.failed.to_string(),
            row.tag,
        ];
        output.push_str(
            &fields
                .iter()
                .map(|field| csv_field(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    Ok(output)
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
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
    if count > data.len().saturating_mul(2) {
        return Err(StorageError::CorruptReplay);
    }
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
            offset: Duration::from_millis(milliseconds),
            kind,
            character,
        });
    }
    Ok(Replay { target, events })
}

pub fn save_replay(paths: &Paths, id: &str, replay: &Replay) -> Result<(), StorageError> {
    validate_replay_id(id)?;
    if replay.events.is_empty() {
        return Err(StorageError::EmptyReplay);
    }
    let directory = paths.data.join("replays");
    fs::create_dir_all(&directory)?;
    atomic_write(&directory.join(format!("{id}.bin")), &encode_replay(replay))?;
    let mut entries: Vec<_> = fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "bin")
        })
        .collect();
    entries.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    let excess = entries.len().saturating_sub(REPLAY_LIMIT);
    for entry in entries.into_iter().take(excess) {
        fs::remove_file(entry.path())?;
    }
    Ok(())
}

pub fn load_replay(paths: &Paths, id: &str) -> Result<Replay, StorageError> {
    validate_replay_id(id)?;
    decode_replay(&fs::read(
        paths.data.join("replays").join(format!("{id}.bin")),
    )?)
}

fn validate_replay_id(id: &str) -> Result<(), StorageError> {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return Err(StorageError::InvalidReplayId);
    }
    Ok(())
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
        remove_optional(&paths.data.join(name))?;
    }
    Ok(())
}
fn remove_languages(paths: &Paths) -> Result<(), StorageError> {
    remove_optional(&paths.data.join("languages"))
}
fn remove_optional(path: &std::path::Path) -> Result<(), StorageError> {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StorageError::Io(error)),
    }
}

fn rfc3339_now() -> String {
    crate::time::rfc3339_now()
}
