use std::fs;
use std::sync::Arc;
use std::thread;

use ttype_core::config::Paths;
use ttype_core::domain::{RunResult, TestConfig, TestKind, TextMode, WordResult};
use ttype_core::storage::{export_csv, list_results, save_result};

fn paths() -> Paths {
    let root = std::env::temp_dir().join(format!("ttype-rs-history-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    Paths {
        config: root.join("config"),
        data: root.join("data"),
    }
}

#[test]
fn reads_the_go_history_schema_and_lists_newest_first() {
    let paths = paths();
    fs::create_dir_all(&paths.data).unwrap_or_else(|error| panic!("data directory: {error}"));
    fs::write(
        paths.data.join("history.json"),
        r#"[
          {"id":"old","timestamp":"2026-01-01T00:00:00Z","test_kind":"timed","duration_sec":60,"text_mode":"words","wpm":50},
          {"id":"new","timestamp":"2026-01-02T00:00:00Z","test_kind":"words","duration_sec":12,"word_count":3,"text_mode":"go","wpm":75,"missed_words":[{"line":2,"expected":"thing","typed":"thnig","missed":true}]}
        ]"#,
    )
    .unwrap_or_else(|error| panic!("fixture: {error}"));

    let rows = list_results(&paths, 0).unwrap_or_else(|error| panic!("history: {error}"));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "new");
    assert_eq!(rows[0].text_mode(), TextMode::Go);
    assert_eq!(rows[0].word_count, 3);
    assert_eq!(rows[0].missed_words[0].line, 2);
}

#[test]
fn saves_go_compatible_flat_results_and_retains_only_missed_words() {
    let paths = paths();
    let result = RunResult {
        id: "0123456789abcdef".to_owned(),
        timestamp: "2026-09-18T10:00:05Z".to_owned(),
        config: TestConfig {
            kind: TestKind::Words,
            word_count: 3,
            text_mode: TextMode::Rust,
            theme: "dracula".to_owned(),
            language: "spanish".to_owned(),
            tag: "practice".to_owned(),
            ..TestConfig::default()
        },
        wpm: 92.5,
        raw_wpm: 98.25,
        accuracy: 96.5,
        consistency: 88.0,
        correct: 270,
        incorrect: 10,
        keystrokes_correct: 280,
        keystrokes_incorrect: 12,
        skipped: 1,
        total_chars: 282,
        duration: std::time::Duration::from_secs(12),
        seed: 4242,
        wpm_history: vec![12.0, 40.0, 92.5],
        raw_wpm_history: vec![24.0, 48.0, 96.0],
        error_history: vec![0, 2, 1],
        char_errors: [("e".to_owned(), 2)].into_iter().collect(),
        failed: false,
        failure_reason: String::new(),
        words: vec![
            WordResult {
                line: 1,
                expected: "fine".to_owned(),
                typed: String::new(),
                missed: false,
                corrected: false,
            },
            WordResult {
                line: 2,
                expected: "letter".to_owned(),
                typed: "lettter".to_owned(),
                missed: true,
                corrected: true,
            },
        ],
        started_at: String::new(),
    };

    let id = save_result(&paths, &result).unwrap_or_else(|error| panic!("save: {error}"));
    assert!(!id.is_empty());
    let raw = fs::read_to_string(paths.data.join("history.json"))
        .unwrap_or_else(|error| panic!("read: {error}"));
    assert!(raw.contains("\"text_mode\": \"rust\""));
    assert!(raw.contains("\"missed_words\""));
    assert!(!raw.contains("\"expected\": \"fine\""));

    let csv =
        export_csv(&paths, None, None, false).unwrap_or_else(|error| panic!("export: {error}"));
    assert!(csv.starts_with("timestamp,mode,language,test,wpm,raw_wpm"));
    assert!(csv.contains(",rust,spanish,3w,92.50,98.25,96.50,88.00,12,false,practice"));
}

#[test]
fn concurrent_saves_keep_every_completed_result() {
    let paths = Arc::new(paths());
    let mut workers = Vec::new();
    for index in 0..40 {
        let paths = Arc::clone(&paths);
        workers.push(thread::spawn(move || {
            let result = RunResult {
                id: format!("concurrent-{index}"),
                timestamp: "2026-04-01T12:00:00Z".to_owned(),
                config: TestConfig::default(),
                wpm: 60.0,
                raw_wpm: 60.0,
                accuracy: 100.0,
                consistency: 0.0,
                correct: 0,
                incorrect: 0,
                keystrokes_correct: 0,
                keystrokes_incorrect: 0,
                skipped: 0,
                total_chars: 0,
                duration: std::time::Duration::ZERO,
                seed: 0,
                wpm_history: Vec::new(),
                raw_wpm_history: Vec::new(),
                error_history: Vec::new(),
                char_errors: Default::default(),
                failed: false,
                failure_reason: String::new(),
                words: Vec::new(),
                started_at: String::new(),
            };
            save_result(&paths, &result)
        }));
    }
    for worker in workers {
        match worker.join() {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => panic!("save result: {error}"),
            Err(_) => panic!("concurrent save panicked"),
        }
    }
    let rows = match list_results(&paths, 0) {
        Ok(rows) => rows,
        Err(error) => panic!("list results: {error}"),
    };
    assert_eq!(rows.len(), 40);
}
