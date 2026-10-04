use std::collections::BTreeMap;
use std::time::Duration;

use crate::domain::CharCounts;

const CHARS_PER_WORD: f64 = 5.0;

pub fn wpm(correct_characters: i32, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        return 0.0;
    }
    round2((f64::from(correct_characters) / CHARS_PER_WORD) * (60.0 / elapsed.as_secs_f64()))
}

pub fn raw_wpm(counts: CharCounts, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        return 0.0;
    }
    round2((f64::from(counts.total_typed()) / CHARS_PER_WORD) * (60.0 / elapsed.as_secs_f64()))
}

pub fn accuracy(correct: i32, incorrect: i32) -> f64 {
    let total = correct + incorrect;
    if total == 0 {
        return 100.0;
    }
    round2(f64::from(correct) / f64::from(total) * 100.0)
}

pub fn consistency(raw_samples: &[f64]) -> f64 {
    if raw_samples.is_empty() {
        return 100.0;
    }
    let sum: f64 = raw_samples.iter().sum();
    let mean = sum / raw_samples.len() as f64;
    if mean <= 0.0 {
        return 100.0;
    }
    let variance = raw_samples
        .iter()
        .map(|sample| {
            let difference = sample - mean;
            difference * difference
        })
        .sum::<f64>()
        / raw_samples.len() as f64;
    let coefficient_of_variation = variance.sqrt() / mean;
    round2(consistency_curve(coefficient_of_variation).clamp(0.0, 100.0))
}

fn consistency_curve(coefficient_of_variation: f64) -> f64 {
    let square = coefficient_of_variation * coefficient_of_variation;
    let scaled = coefficient_of_variation * (1.0 + square * (1.0 / 3.0 + square / 5.0));
    100.0 * (1.0 - scaled.tanh())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharError {
    pub character: String,
    pub count: i32,
}

pub fn top_char_errors(counts: &BTreeMap<String, i32>, limit: usize) -> Vec<CharError> {
    if counts.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut errors: Vec<_> = counts
        .iter()
        .map(|(character, count)| CharError {
            character: character.clone(),
            count: *count,
        })
        .collect();
    errors.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.character.cmp(&right.character))
    });
    errors.truncate(limit);
    errors
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
