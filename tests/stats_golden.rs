use std::collections::BTreeMap;
use std::time::Duration;

use ttype_core::domain::CharCounts;
use ttype_core::stats::{accuracy, consistency, raw_wpm, top_char_errors, wpm};

#[test]
fn go_wpm_vectors_are_preserved() {
    assert_eq!(wpm(300, Duration::from_secs(60)), 60.0);
    assert_eq!(wpm(300, Duration::from_secs(30)), 120.0);
    assert_eq!(wpm(100, Duration::ZERO), 0.0);
    assert_eq!(wpm(37, Duration::from_secs(10)), 44.4);
    assert_eq!(
        raw_wpm(
            CharCounts {
                correct: 250,
                incorrect: 50,
                extra: 0,
            },
            Duration::from_secs(60),
        ),
        60.0
    );
}

#[test]
fn go_accuracy_and_consistency_vectors_are_preserved() {
    assert_eq!(accuracy(0, 0), 100.0);
    assert_eq!(accuracy(97, 3), 97.0);
    assert_eq!(consistency(&[]), 100.0);
    assert_eq!(consistency(&[0.0, 0.0, 0.0]), 100.0);
    assert_eq!(consistency(&[60.0, 60.0, 60.0, 60.0]), 100.0);
    assert_eq!(consistency(&[75.0, 45.0]), 75.0);
}

#[test]
fn heatmap_ranks_by_descending_count_then_character() {
    let counts = BTreeMap::from([
        ("e".to_owned(), 2),
        ("a".to_owned(), 5),
        ("z".to_owned(), 2),
        ("q".to_owned(), 1),
    ]);
    let ranked = top_char_errors(&counts, 3);
    assert_eq!(ranked[0].character, "a");
    assert_eq!(ranked[0].count, 5);
    assert_eq!(ranked[1].character, "e");
    assert_eq!(ranked[2].character, "z");
}
