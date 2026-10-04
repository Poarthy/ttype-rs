use std::time::Duration;

use proptest::prelude::*;
use ttype_core::clock::FakeClock;
use ttype_core::domain::{CharCounts, TestConfig, TestKind, TextMode};
use ttype_core::session::Session;
use ttype_core::stats::consistency;

fn recount(input: &[char], target: &[char]) -> CharCounts {
    input
        .iter()
        .enumerate()
        .fold(CharCounts::default(), |mut counts, (index, character)| {
            if *character == '\0' {
                return counts;
            }
            if index >= target.len() {
                counts.extra += 1;
            } else if *character == target[index] {
                counts.correct += 1;
            } else {
                counts.incorrect += 1;
            }
            counts
        })
}

proptest! {
    #[test]
    fn incremental_counts_match_recount(ops in prop::collection::vec(0u8..10, 1..500)) {
        let clock = FakeClock::new();
        let config = TestConfig {
            kind: TestKind::Timed,
            duration: Duration::from_secs(300),
            text_mode: TextMode::Words,
            ..TestConfig::default()
        };
        let target = "the quick brown fox jumps over the lazy dog";
        let mut session = match Session::new(config, target, clock) {
            Ok(value) => value,
            Err(error) => panic!("session should start: {error}"),
        };
        let keyboard: Vec<char> = "thequickbrownfxjmpsvlazydg ".chars().collect();
        for operation in ops {
            match operation {
                0..=5 => session.input_char(keyboard[usize::from(operation) % keyboard.len()]),
                6..=7 => { session.backspace(); }
                _ => { session.delete_word(); }
            }
            let mut expected = recount(session.input_chars(), &target.chars().collect::<Vec<_>>());
            for position in 0..=target.chars().count() {
                expected.extra += session.extras_at(position).len() as i32;
            }
            prop_assert_eq!(session.counts(), expected);
        }
    }

    #[test]
    fn consistency_is_bounded(samples in prop::collection::vec(0.0f64..1000.0, 0..100)) {
        let score = consistency(&samples);
        prop_assert!((0.0..=100.0).contains(&score));
    }
}
