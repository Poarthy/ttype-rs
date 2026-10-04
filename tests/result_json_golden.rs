use std::time::Duration;

use serde_json::Value;
use ttype_core::domain::{RunResult, TestConfig};

#[test]
fn stdout_result_uses_go_scalars_for_ids_timestamps_and_durations() {
    let result = RunResult {
        id: "0123456789abcdef".to_owned(),
        timestamp: "2026-09-18T10:00:30Z".to_owned(),
        config: TestConfig {
            duration: Duration::from_secs(60),
            ..TestConfig::default()
        },
        wpm: 60.0,
        raw_wpm: 60.0,
        accuracy: 100.0,
        consistency: 100.0,
        correct: 300,
        incorrect: 0,
        keystrokes_correct: 300,
        keystrokes_incorrect: 0,
        skipped: 0,
        total_chars: 300,
        duration: Duration::from_secs(30),
        seed: 42,
        wpm_history: vec![60.0],
        raw_wpm_history: vec![60.0],
        error_history: vec![0],
        char_errors: Default::default(),
        failed: false,
        failure_reason: String::new(),
        words: Vec::new(),
        started_at: "2026-09-18T10:00:00Z".to_owned(),
    };
    let json: Value = serde_json::to_value(result).unwrap_or_else(|error| panic!("JSON: {error}"));
    assert_eq!(json["id"], "0123456789abcdef");
    assert_eq!(json["timestamp"], "2026-09-18T10:00:30Z");
    assert_eq!(json["config"]["duration"], 60);
    assert_eq!(json["duration"], 30_000_000_000_u64);
    assert!(json.get("started_at").is_none());
}
