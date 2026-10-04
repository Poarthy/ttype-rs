use std::collections::BTreeMap;
use std::fs;

use ttype_core::domain::{RunResult, TestConfig, TextMode, WordResult};
use ttype_core::result_file::{ResultSource, write};

#[test]
fn writes_the_go_result_file_shape_without_private_custom_text() {
    let root = std::env::temp_dir().join(format!("ttype-rs-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap_or_else(|error| panic!("dir: {error}"));
    let path = root.join("run.json");
    let result = RunResult {
        id: "0123456789abcdef".to_owned(),
        timestamp: "2026-09-18T10:00:05Z".to_owned(),
        config: TestConfig {
            text_mode: TextMode::Custom,
            ..TestConfig::default()
        },
        duration: std::time::Duration::from_millis(30_006),
        wpm: 50.0,
        correct: 10,
        incorrect: 2,
        total_chars: 15,
        char_errors: BTreeMap::from([("t".to_owned(), 2)]),
        words: vec![WordResult {
            line: 1,
            expected: "letter".to_owned(),
            typed: "lettter".to_owned(),
            missed: true,
            corrected: false,
        }],
        raw_wpm: 0.0,
        accuracy: 0.0,
        consistency: 0.0,
        keystrokes_correct: 0,
        keystrokes_incorrect: 0,
        skipped: 0,
        seed: 0,
        wpm_history: Vec::new(),
        raw_wpm_history: Vec::new(),
        error_history: Vec::new(),
        failed: false,
        failure_reason: String::new(),
        started_at: "2026-09-18T09:59:59Z".to_owned(),
    };
    write(
        &path,
        Some(&result),
        true,
        ResultSource {
            file: Some("typing.txt"),
            text: Some("I  think"),
        },
    )
    .unwrap_or_else(|error| panic!("write: {error}"));
    let raw = fs::read_to_string(path).unwrap_or_else(|error| panic!("read: {error}"));
    assert!(raw.ends_with('\n'));
    assert!(raw.contains("\"status\": \"completed\""));
    assert!(raw.contains("\"started_at\": \"2026-09-18T09:59:59Z\""));
    assert!(raw.contains("\"duration_s\": 30.01"));
    assert!(raw.contains("\"extra\": 3"));
    assert!(raw.contains("\"file\": \"typing.txt\""));
    assert!(raw.contains("\"typed\": \"lettter\""));
    assert!(!raw.contains("I  think"));
}

#[test]
fn records_quit_when_no_run_started() {
    let root = std::env::temp_dir().join(format!("ttype-rs-result-quit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap_or_else(|error| panic!("dir: {error}"));
    let path = root.join("quit.json");
    write(&path, None, false, ResultSource::default())
        .unwrap_or_else(|error| panic!("write: {error}"));
    let raw = fs::read_to_string(path).unwrap_or_else(|error| panic!("read: {error}"));
    assert!(raw.contains("\"status\": \"quit\""));
}
