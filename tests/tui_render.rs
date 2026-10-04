use std::time::Duration;

use ttype_core::clock::FakeClock;
use ttype_core::domain::{RunResult, TestConfig, TestKind, TextMode};
use ttype_core::session::Session;
use ttype_core::tui::{blind_reveal_end, render_chart, render_heatmap, render_result_text};

#[test]
fn blind_mode_reveals_only_completed_words() {
    let target: Vec<_> = "one two".chars().collect();
    assert_eq!(blind_reveal_end(&target, 0), 0);
    assert_eq!(blind_reveal_end(&target, 2), 0);
    assert_eq!(blind_reveal_end(&target, 4), 4);
}

#[test]
fn result_includes_chart_heatmap_and_all_key_stats() {
    let result = RunResult {
        id: String::new(),
        timestamp: String::new(),
        config: TestConfig {
            kind: TestKind::Words,
            word_count: 3,
            text_mode: TextMode::Words,
            ..TestConfig::default()
        },
        wpm: 80.0,
        raw_wpm: 90.0,
        accuracy: 95.0,
        consistency: 88.0,
        correct: 20,
        incorrect: 2,
        keystrokes_correct: 20,
        keystrokes_incorrect: 2,
        skipped: 0,
        total_chars: 22,
        duration: Duration::from_secs(10),
        seed: 1,
        wpm_history: vec![40.0, 80.0],
        raw_wpm_history: vec![50.0, 90.0],
        error_history: vec![0, 1],
        char_errors: [("e".to_owned(), 2), (" ".to_owned(), 1)]
            .into_iter()
            .collect(),
        failed: false,
        failure_reason: String::new(),
        words: Vec::new(),
        started_at: String::new(),
    };
    let text = render_result_text(&result, 80);
    assert!(text.contains("Test Complete"));
    assert!(text.contains("wpm over time"));
    assert!(text.contains("missed"));
    assert!(render_chart(&[1.0, 2.0], &[1.0, 3.0], &[0, 1], 40).contains("err"));
    assert!(render_heatmap(&result.char_errors, 8).contains("space×1"));
}

#[test]
fn replay_events_are_suitable_for_original_speed_playback() {
    let clock = FakeClock::new();
    let mut session = Session::new(
        TestConfig {
            kind: TestKind::Words,
            word_count: 1,
            ..TestConfig::default()
        },
        "a",
        clock.clone(),
    )
    .unwrap_or_else(|error| panic!("session: {error}"));
    session.input_char('a');
    assert_eq!(session.events().len(), 1);
    assert_eq!(session.events()[0].offset, Duration::ZERO);
}
