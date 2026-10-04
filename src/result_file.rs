use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{RunResult, TextMode, WordResult};

const RESULT_FILE_VERSION: u8 = 1;

#[derive(Clone, Debug, Default)]
pub struct ResultSource<'a> {
    pub file: Option<&'a str>,
    pub text: Option<&'a str>,
}

#[derive(Debug, Error)]
pub enum ResultFileError {
    #[error("result JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("write result file: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug, Serialize)]
struct ResultFile<'a> {
    version: u8,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    duration_s: f64,
    wpm: f64,
    raw: f64,
    accuracy: f64,
    consistency: f64,
    chars: ResultChars,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    char_errors: BTreeMap<String, i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<TextMode>,
    #[serde(skip_serializing_if = "String::is_empty")]
    language: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<StoredSource>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    words: Vec<WordResult>,
}

#[derive(Clone, Debug, Serialize)]
struct ResultChars {
    correct: i32,
    incorrect: i32,
    extra: i32,
    skipped: i32,
}

#[derive(Clone, Debug, Serialize)]
struct StoredSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    sha256: String,
}

pub fn write(
    path: &Path,
    result: Option<&RunResult>,
    finished: bool,
    source: ResultSource<'_>,
) -> Result<(), ResultFileError> {
    let body = serde_json::to_vec_pretty(&new_result_file(result, finished, source))?;
    let mut with_newline = body;
    with_newline.push(b'\n');
    fs::write(path, with_newline)?;
    Ok(())
}

fn new_result_file<'a>(
    result: Option<&'a RunResult>,
    finished: bool,
    source: ResultSource<'_>,
) -> ResultFile<'a> {
    let Some(result) = result else {
        return empty_result("quit");
    };
    let status = if result.failed {
        "failed_min_wpm"
    } else if finished {
        "completed"
    } else {
        "quit"
    };
    let extra = (result.total_chars - result.correct - result.incorrect).max(0);
    let source = source.text.map(|text| StoredSource {
        file: source.file.map(str::to_owned),
        sha256: source_hash(text),
    });
    ResultFile {
        version: RESULT_FILE_VERSION,
        status,
        started_at: (!result.started_at.is_empty()).then(|| result.started_at.clone()),
        duration_s: round_hundredths(result.duration.as_secs_f64()),
        wpm: result.wpm,
        raw: result.raw_wpm,
        accuracy: result.accuracy,
        consistency: result.consistency,
        chars: ResultChars {
            correct: result.correct,
            incorrect: result.incorrect,
            extra,
            skipped: result.skipped,
        },
        char_errors: result.char_errors.clone(),
        mode: Some(result.config.text_mode),
        language: result.config.language.clone(),
        tag: result.config.tag.clone(),
        source,
        words: result.words.clone(),
    }
}

fn empty_result<'a>(status: &'a str) -> ResultFile<'a> {
    ResultFile {
        version: RESULT_FILE_VERSION,
        status,
        started_at: None,
        duration_s: 0.0,
        wpm: 0.0,
        raw: 0.0,
        accuracy: 0.0,
        consistency: 0.0,
        chars: ResultChars {
            correct: 0,
            incorrect: 0,
            extra: 0,
            skipped: 0,
        },
        char_errors: BTreeMap::new(),
        mode: None,
        language: String::new(),
        tag: String::new(),
        source: None,
        words: Vec::new(),
    }
}

fn round_hundredths(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn source_hash(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::source_hash;

    #[test]
    fn source_hash_ignores_whitespace() {
        assert_eq!(source_hash("I  think"), source_hash("I think"));
    }
}
