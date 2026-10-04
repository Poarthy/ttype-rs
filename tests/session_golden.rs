use std::time::Duration;

use ttype_core::clock::FakeClock;
use ttype_core::domain::{TestConfig, TestKind, TextMode};
use ttype_core::session::{Session, SessionState};

fn timed(target: &str, mode: TextMode) -> (Session, FakeClock) {
    let clock = FakeClock::new();
    let config = TestConfig {
        kind: TestKind::Timed,
        duration: Duration::from_secs(60),
        text_mode: mode,
        ..TestConfig::default()
    };
    let session = match Session::new(config, target, clock.clone()) {
        Ok(value) => value,
        Err(error) => panic!("session should start: {error}"),
    };
    (session, clock)
}

#[test]
fn first_accepted_key_starts_the_clock() {
    let (mut session, clock) = timed("abc", TextMode::Words);
    assert_eq!(session.state(), SessionState::Ready);
    assert_eq!(session.remaining(), Duration::from_secs(60));
    session.input_char('a');
    clock.advance(Duration::from_secs(2));
    assert_eq!(session.state(), SessionState::Active);
    assert_eq!(session.remaining(), Duration::from_secs(58));
}

#[test]
fn word_space_skips_remainder_but_code_space_is_literal() {
    let (mut words, _) = timed("abc def", TextMode::Words);
    words.input_char('a');
    words.input_char(' ');
    assert_eq!(words.input_chars(), &['a', '\0', '\0', '\0']);
    assert_eq!(words.skipped(), 2);

    let (mut code, _) = timed("select id", TextMode::Sql);
    code.input_char('s');
    code.input_char(' ');
    assert_eq!(code.input_text(), "s ");
    assert_eq!(code.cursor(), 2);
}

#[test]
fn code_mode_counts_tabs_and_newlines_as_literal_practice_characters() {
    let (mut code, _) = timed("a\t{\n}", TextMode::Rust);
    for character in ['a', '\t', '{', '\n', '}'] {
        code.input_char(character);
    }
    assert_eq!(code.input_text(), "a\t{\n}");
    assert_eq!(code.counts().correct, 5);
}

#[test]
fn extras_are_capped_but_every_keystroke_hurts_accuracy() {
    let (mut session, _) = timed("cat dog", TextMode::Words);
    for character in "cat".chars() {
        session.input_char(character);
    }
    for _ in 0..50 {
        session.input_char('x');
    }
    assert_eq!(session.extras_at(3).len(), 20);
    assert_eq!(session.keystrokes(), (3, 50));
    assert_eq!(session.counts().extra, 20);
}

#[test]
fn backspace_and_delete_word_rewind_buffer_counts_but_not_accuracy() {
    let (mut session, _) = timed("the cat sat", TextMode::Words);
    for character in "the cXt".chars() {
        session.input_char(character);
    }
    let keystrokes = session.keystrokes();
    assert!(session.backspace());
    assert!(session.delete_word());
    assert!(session.backspace());
    assert_eq!(session.keystrokes(), keystrokes);
    assert_eq!(session.input_text(), "the");
    assert_eq!(session.counts().correct, 3);
    assert_eq!(session.counts().incorrect, 0);
}

#[test]
fn space_on_an_untouched_word_is_ignored_and_does_not_start_a_run() {
    let (mut session, clock) = timed("abc def", TextMode::Words);
    session.input_char(' ');
    assert_eq!(session.state(), SessionState::Ready);
    clock.advance(Duration::from_secs(61));
    assert!(!session.tick());
    assert_eq!(session.state(), SessionState::Ready);
}

#[test]
fn code_modes_need_three_case_inversions_but_word_modes_need_two() {
    let (mut words, _) = timed("hello", TextMode::Words);
    words.input_char('H');
    words.input_char('E');
    assert!(words.caps_lock_suspected());

    let (mut code, _) = timed("abc", TextMode::Sql);
    code.input_char('A');
    code.input_char('B');
    assert!(!code.caps_lock_suspected());
    code.input_char('C');
    assert!(code.caps_lock_suspected());
}

#[test]
fn per_second_history_uses_full_seconds_and_long_enough_final_partials() {
    let (mut session, clock) = timed("abcdefghij", TextMode::Words);
    session.input_char('a');
    session.input_char('b');
    clock.advance(Duration::from_millis(2500));
    session.input_char('c');
    session.input_char('d');
    clock.advance(Duration::from_millis(500));
    session.finish_now();
    assert_eq!(session.raw_wpm_history(), vec![24.0, 0.0, 24.0]);
    assert_eq!(session.error_history(), vec![0, 0, 0]);
}

#[test]
fn session_uses_the_one_second_rating_floor_and_partial_second_rule() {
    let config = TestConfig {
        kind: TestKind::Words,
        word_count: 1,
        text_mode: TextMode::Words,
        ..TestConfig::default()
    };
    let clock = FakeClock::new();
    let mut session = match Session::new(config, "maximum", clock.clone()) {
        Ok(value) => value,
        Err(error) => panic!("session should start: {error}"),
    };
    session.input_char('m');
    clock.advance(Duration::from_millis(1));
    for character in "aximum".chars() {
        session.input_char(character);
    }
    let result = match session.result() {
        Ok(value) => value,
        Err(error) => panic!("completed session should have a result: {error}"),
    };
    assert_eq!(result.wpm, 84.0);
    assert_eq!(result.raw_wpm, 84.0);
    assert_eq!(result.duration, Duration::from_millis(1));
    assert!(result.raw_wpm_history.is_empty());
}

#[test]
fn min_wpm_only_fails_after_five_second_grace() {
    let clock = FakeClock::new();
    let config = TestConfig {
        kind: TestKind::Timed,
        duration: Duration::from_secs(60),
        text_mode: TextMode::Words,
        min_wpm: 60,
        ..TestConfig::default()
    };
    let mut session = match Session::new(config, "a very long target", clock.clone()) {
        Ok(value) => value,
        Err(error) => panic!("session should start: {error}"),
    };
    session.input_char('a');
    clock.advance(Duration::from_millis(4999));
    assert!(!session.tick());
    clock.advance(Duration::from_millis(1));
    assert!(session.tick());
    let result = match session.result() {
        Ok(value) => value,
        Err(error) => panic!("failed session should have a result: {error}"),
    };
    assert!(result.failed);
    assert_eq!(result.failure_reason, "WPM below minimum (60)");
}

#[test]
fn replay_events_reproduce_the_same_result() {
    let (mut original, clock) = timed("the cat sat", TextMode::Words);
    for character in "the cst ".chars() {
        original.input_char(character);
        clock.advance(Duration::from_millis(120));
    }
    assert!(original.backspace());
    assert!(original.delete_word());
    clock.advance(Duration::from_millis(300));
    for character in "cat sat".chars() {
        original.input_char(character);
        clock.advance(Duration::from_millis(90));
    }
    clock.advance(Duration::from_secs(60));
    assert!(original.tick());
    let expected = match original.result() {
        Ok(value) => value,
        Err(error) => panic!("result should be available: {error}"),
    };

    let replay_clock = FakeClock::new();
    let config = original.config().clone();
    let mut replay = match Session::new(config, "the cat sat", replay_clock.clone()) {
        Ok(value) => value,
        Err(error) => panic!("replay should start: {error}"),
    };
    for event in original.events() {
        replay_clock.set(event.offset);
        replay.apply_event(event);
    }
    replay_clock.set(expected.duration);
    assert!(replay.tick());
    let actual = match replay.result() {
        Ok(value) => value,
        Err(error) => panic!("replay should finish: {error}"),
    };
    assert_eq!(actual.wpm, expected.wpm);
    assert_eq!(actual.raw_wpm, expected.raw_wpm);
    assert_eq!(actual.accuracy, expected.accuracy);
    assert_eq!(actual.consistency, expected.consistency);
}
