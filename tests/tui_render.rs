use std::time::Duration;

use ttype_core::clock::FakeClock;
use ttype_core::domain::{RunResult, TestConfig, TestKind, TextMode};
use ttype_core::session::Session;
use ttype_core::tui::{
    blind_reveal_end, hud_pace, render_chart, render_heatmap, render_result_chart,
    render_result_text,
};

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
    assert!(render_chart(&[1.0, 2.0], &[1.0, 3.0], &[0, 1], 40).contains('×'));
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

#[test]
fn result_chart_matches_the_go_braille_golden() {
    let actual = render_result_chart(
        &[0.0, 30.0, 60.0, 60.0],
        &[20.0, 40.0, 70.0, 80.0],
        &[0, 2, 0, 0],
        40,
        6,
        true,
    );
    let expected = concat!(
        "wpm over time · peak 60\n",
        "80┤                     ⣀⣀⣤⣤⣤⣤⣶⣶⣶⣶⣶⣶⣾⣿⣿⣿\n",
        "  │            ×⣀⣠⣤⣴⣶⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿\n",
        "40┤⣀⣀⣠⣤⣤⣤⣶⣶⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿\n",
        " 0┤⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿\n",
        "  └0s─────────────────────────────────3s"
    );
    assert_eq!(actual, expected);
}

#[test]
fn result_chart_uses_the_go_ascii_fallback_thresholds() {
    assert_eq!(
        render_result_chart(&[20.0, 40.0, 60.0], &[], &[], 30, 10, true),
        "wpm over time · peak 60\n:=#"
    );
    assert!(render_result_chart(&[20.0], &[], &[], 60, 10, true).is_empty());
    assert!(render_result_chart(&[20.0, 40.0], &[], &[], 60, 1, true).is_empty());
}

#[test]
fn hud_uses_the_go_one_second_display_floor_without_extrapolation() {
    let clock = FakeClock::new();
    let mut session = match Session::new(
        TestConfig {
            kind: TestKind::Timed,
            duration: Duration::from_secs(60),
            ..TestConfig::default()
        },
        "abc",
        clock.clone(),
    ) {
        Ok(session) => session,
        Err(error) => panic!("session: {error}"),
    };
    session.input_char('a');
    clock.advance(Duration::from_millis(100));
    assert_eq!(hud_pace(&session), (12.0, 12.0));
}

#[test]
fn result_clock_rounds_like_go_time_round() {
    let result = RunResult {
        id: String::new(),
        timestamp: String::new(),
        config: TestConfig::default(),
        wpm: 0.0,
        raw_wpm: 0.0,
        accuracy: 100.0,
        consistency: 0.0,
        correct: 0,
        incorrect: 0,
        keystrokes_correct: 0,
        keystrokes_incorrect: 0,
        skipped: 0,
        total_chars: 0,
        duration: Duration::from_millis(1_500),
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
    assert!(render_result_text(&result, 80).contains("time 0:02"));
}
