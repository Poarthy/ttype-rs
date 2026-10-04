use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::clock::Clock;
use crate::domain::{
    CharCounts, GenerateOptions, RunResult, TestConfig, TestKind, TextMode, WordResult,
};
use crate::replay::{ReplayEvent, ReplayEventKind};
use crate::stats;
use crate::text::{StaticText, TextError, TextSource};

const SKIP_CHAR: char = '\0';
const PARTIAL_SECOND_FLOOR: Duration = Duration::from_millis(500);
const MIN_RATED_TIME: Duration = Duration::from_secs(1);
const MIN_WPM_GRACE: Duration = Duration::from_secs(5);
const MAX_EXTRAS: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Ready,
    Active,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeystrokeStatus {
    Pending,
    Correct,
    Incorrect,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SecondBucket {
    typed: i32,
    errors: i32,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("timed session requires a positive duration")]
    MissingDuration,
    #[error("generate target: {0}")]
    Generate(#[from] TextError),
    #[error("session not finished")]
    NotFinished,
}

pub struct Session {
    config: TestConfig,
    source: Box<dyn TextSource>,
    target: String,
    target_chars: Vec<char>,
    input: Vec<char>,
    counts: CharCounts,
    state: SessionState,
    keystrokes_correct: i32,
    keystrokes_incorrect: i32,
    clock: Box<dyn Clock>,
    started_at: Option<Duration>,
    ended_at: Option<Duration>,
    caps_inversions: i32,
    seed: i64,
    wpm_history: Vec<f64>,
    seconds: Vec<SecondBucket>,
    skipped: i32,
    char_errors: BTreeMap<String, i32>,
    failed: bool,
    failure_reason: String,
    events: Vec<ReplayEvent>,
    extras: BTreeMap<usize, Vec<char>>,
    extras_revision: usize,
    word_at: Vec<usize>,
    missed: Vec<bool>,
}

impl Session {
    pub fn new<C>(
        config: TestConfig,
        target: impl Into<String>,
        clock: C,
    ) -> Result<Self, SessionError>
    where
        C: Clock + 'static,
    {
        Self::from_source(config, Box::new(StaticText(target.into())), clock)
    }

    pub fn from_source<C>(
        config: TestConfig,
        source: Box<dyn TextSource>,
        clock: C,
    ) -> Result<Self, SessionError>
    where
        C: Clock + 'static,
    {
        if matches!(config.kind, TestKind::Timed) && config.duration.is_zero() {
            return Err(SessionError::MissingDuration);
        }
        let seed = resolve_seed(config.seed);
        let mut session = Self {
            config,
            source,
            target: String::new(),
            target_chars: Vec::new(),
            input: Vec::new(),
            counts: CharCounts::default(),
            state: SessionState::Ready,
            keystrokes_correct: 0,
            keystrokes_incorrect: 0,
            clock: Box::new(clock),
            started_at: None,
            ended_at: None,
            caps_inversions: 0,
            seed,
            wpm_history: Vec::new(),
            seconds: Vec::new(),
            skipped: 0,
            char_errors: BTreeMap::new(),
            failed: false,
            failure_reason: String::new(),
            events: Vec::new(),
            extras: BTreeMap::new(),
            extras_revision: 0,
            word_at: Vec::new(),
            missed: Vec::new(),
        };
        session.load_target()?;
        Ok(session)
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn target_chars(&self) -> &[char] {
        &self.target_chars
    }

    pub fn input_chars(&self) -> &[char] {
        &self.input
    }

    pub fn input_text(&self) -> String {
        self.input.iter().collect()
    }

    pub fn cursor(&self) -> usize {
        self.input.len()
    }

    pub fn config(&self) -> &TestConfig {
        &self.config
    }

    pub fn counts(&self) -> CharCounts {
        self.counts
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn skipped(&self) -> i32 {
        self.skipped
    }

    pub fn extras_at(&self, position: usize) -> &[char] {
        self.extras.get(&position).map_or(&[], Vec::as_slice)
    }

    pub fn extras_revision(&self) -> usize {
        self.extras_revision
    }

    pub fn char_errors(&self) -> &BTreeMap<String, i32> {
        &self.char_errors
    }

    pub fn keystrokes(&self) -> (i32, i32) {
        (self.keystrokes_correct, self.keystrokes_incorrect)
    }

    pub fn events(&self) -> &[ReplayEvent] {
        &self.events
    }

    pub fn wpm_history(&self) -> &[f64] {
        &self.wpm_history
    }

    pub fn raw_wpm_history(&self) -> Vec<f64> {
        self.per_second_samples().0
    }

    pub fn error_history(&self) -> Vec<i32> {
        self.per_second_samples().1
    }

    pub fn remaining(&self) -> Duration {
        if matches!(self.config.kind, TestKind::Words) {
            return Duration::ZERO;
        }
        self.config.duration.saturating_sub(self.elapsed())
    }

    pub fn live_wpm(&self) -> f64 {
        stats::wpm(self.counts.correct, self.elapsed())
    }

    pub fn live_raw_wpm(&self) -> f64 {
        stats::raw_wpm(self.counts, self.elapsed())
    }

    pub fn live_accuracy(&self) -> f64 {
        stats::accuracy(self.keystrokes_correct, self.keystrokes_incorrect)
    }

    pub fn status_at(&self, index: usize) -> KeystrokeStatus {
        match (self.input.get(index), self.target_chars.get(index)) {
            (None, _) => KeystrokeStatus::Pending,
            (Some(_), None) => KeystrokeStatus::Incorrect,
            (Some(actual), Some(expected)) if actual == expected => KeystrokeStatus::Correct,
            (Some(_), Some(_)) => KeystrokeStatus::Incorrect,
        }
    }

    pub fn words_progress(&self) -> (usize, usize) {
        if !self.config.is_words_mode() {
            return (0, 0);
        }
        let completed =
            completed_words(&self.input, &self.target_chars).min(self.config.word_count);
        (completed, self.config.word_count)
    }

    pub fn caps_lock_suspected(&self) -> bool {
        let threshold = if self.config.text_mode.commits_words_on_space() {
            2
        } else {
            3
        };
        self.caps_inversions >= threshold
    }

    pub fn restart(&mut self) -> Result<(), SessionError> {
        self.input.clear();
        self.counts = CharCounts::default();
        self.keystrokes_correct = 0;
        self.keystrokes_incorrect = 0;
        self.state = SessionState::Ready;
        self.started_at = None;
        self.ended_at = None;
        self.caps_inversions = 0;
        self.wpm_history.clear();
        self.seconds.clear();
        self.skipped = 0;
        self.char_errors.clear();
        self.failed = false;
        self.failure_reason.clear();
        self.events.clear();
        self.extras.clear();
        self.extras_revision = self.extras_revision.saturating_add(1);
        self.seed = resolve_seed(self.config.seed);
        self.load_target()
    }

    pub fn input_char(&mut self, character: char) {
        let literal_code_control =
            !self.config.text_mode.commits_words_on_space() && matches!(character, '\n' | '\t');
        if matches!(self.state, SessionState::Finished)
            || (character.is_control() && !literal_code_control)
        {
            return;
        }

        let position = self.input.len();
        let commits_words = self.config.text_mode.commits_words_on_space();
        let skipping = commits_words
            && character == ' '
            && position < self.target_chars.len()
            && self.target_chars[position] != ' '
            && position != word_start_at(&self.target_chars, position);

        if commits_words
            && character != ' '
            && position < self.target_chars.len()
            && self.target_chars[position] == ' '
        {
            self.record_event(ReplayEventKind::Rune, Some(character));
            self.keystrokes_incorrect += 1;
            self.bucket_keystroke(false);
            self.mark_missed(position);
            if self.extras_at(position).len() < MAX_EXTRAS {
                self.extras.entry(position).or_default().push(character);
                self.extras_revision = self.extras_revision.saturating_add(1);
                self.counts.extra += 1;
            }
            self.record_wpm_snapshot();
            return;
        }

        if commits_words
            && character == ' '
            && position < self.target_chars.len()
            && self.target_chars[position] != ' '
            && position == word_start_at(&self.target_chars, position)
        {
            return;
        }

        if matches!(self.state, SessionState::Ready) {
            self.state = SessionState::Active;
            self.started_at = Some(self.clock.now());
        }
        self.record_event(ReplayEventKind::Rune, Some(character));

        if skipping {
            self.skip_current_word(position);
        } else {
            if let Some(expected) = self.target_chars.get(position).copied() {
                self.update_caps_streak(character, expected);
            }
            match self.target_chars.get(position).copied() {
                None => {
                    self.keystrokes_incorrect += 1;
                    self.counts.extra += 1;
                    self.bucket_keystroke(false);
                    self.mark_missed(position);
                }
                Some(expected) if character == expected => {
                    self.keystrokes_correct += 1;
                    self.counts.correct += 1;
                    self.bucket_keystroke(true);
                }
                Some(expected) => {
                    self.keystrokes_incorrect += 1;
                    self.counts.incorrect += 1;
                    *self.char_errors.entry(expected.to_string()).or_default() += 1;
                    self.bucket_keystroke(false);
                    self.mark_missed(position);
                }
            }
            self.input.push(character);
        }

        self.record_wpm_snapshot();
        self.check_min_wpm();
        if matches!(self.state, SessionState::Finished) {
            return;
        }
        let text_done = matches!(self.config.kind, TestKind::Words)
            || matches!(self.config.text_mode, TextMode::Custom);
        if text_done && self.input.len() >= self.target_chars.len() {
            self.finish();
        }
    }

    pub fn backspace(&mut self) -> bool {
        if !self.backspace_internal() {
            return false;
        }
        self.record_event(ReplayEventKind::Backspace, None);
        true
    }

    pub fn delete_word(&mut self) -> bool {
        if !self.delete_word_internal() {
            return false;
        }
        self.record_event(ReplayEventKind::DeleteWord, None);
        true
    }

    pub fn apply_event(&mut self, event: &ReplayEvent) {
        match event.kind {
            ReplayEventKind::Rune => {
                if let Some(character) = event.character {
                    self.input_char(character);
                }
            }
            ReplayEventKind::Backspace => {
                self.backspace();
            }
            ReplayEventKind::DeleteWord => {
                self.delete_word();
            }
        }
    }

    pub fn tick(&mut self) -> bool {
        if !matches!(self.state, SessionState::Active) {
            return false;
        }
        self.record_wpm_snapshot();
        self.check_min_wpm();
        if matches!(self.state, SessionState::Finished) {
            return true;
        }
        if matches!(self.config.kind, TestKind::Words) {
            return false;
        }
        if self.elapsed() >= self.config.duration {
            self.finish();
            return true;
        }
        false
    }

    pub fn finish_now(&mut self) {
        if matches!(self.state, SessionState::Finished) {
            return;
        }
        if matches!(self.state, SessionState::Ready) {
            self.state = SessionState::Active;
            self.started_at = Some(self.clock.now());
        }
        self.finish();
    }

    pub fn result(&self) -> Result<RunResult, SessionError> {
        if !matches!(self.state, SessionState::Finished) {
            return Err(SessionError::NotFinished);
        }
        let rated_elapsed = self.elapsed().max(MIN_RATED_TIME);
        let (raw_wpm_history, error_history) = self.per_second_samples();
        Ok(RunResult {
            config: self.config.clone(),
            wpm: stats::wpm(self.counts.correct, rated_elapsed),
            raw_wpm: stats::raw_wpm(self.counts, rated_elapsed),
            accuracy: stats::accuracy(self.keystrokes_correct, self.keystrokes_incorrect),
            consistency: stats::consistency(&raw_wpm_history),
            correct: self.counts.correct,
            incorrect: self.counts.incorrect,
            keystrokes_correct: self.keystrokes_correct,
            keystrokes_incorrect: self.keystrokes_incorrect,
            skipped: self.skipped,
            total_chars: self.counts.total_typed(),
            duration: self.elapsed(),
            seed: self.seed,
            wpm_history: self.wpm_history.clone(),
            raw_wpm_history,
            error_history,
            char_errors: self.char_errors.clone(),
            failed: self.failed,
            failure_reason: self.failure_reason.clone(),
            words: self.words(),
        })
    }

    pub fn elapsed(&self) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::ZERO;
        };
        match self.state {
            SessionState::Ready => Duration::ZERO,
            SessionState::Active => self.clock.now().saturating_sub(started_at),
            SessionState::Finished => self
                .ended_at
                .unwrap_or(started_at)
                .saturating_sub(started_at),
        }
    }

    pub fn words(&self) -> Vec<WordResult> {
        let mut words = Vec::new();
        let mut start = 0;
        while start < self.target_chars.len() && start < self.input.len() {
            if self.target_chars[start] == ' ' {
                start += 1;
                continue;
            }
            let end = word_end_at(&self.target_chars, start);
            let word_index = self.word_at[start];
            let mut word = WordResult {
                line: 0,
                expected: self.target_chars[start..end].iter().collect(),
                typed: String::new(),
                missed: self.missed[word_index],
                corrected: false,
            };
            if word.missed {
                word.typed = self.input[start..end.min(self.input.len())]
                    .iter()
                    .filter(|character| **character != SKIP_CHAR)
                    .chain(self.extras_at(end).iter())
                    .collect();
                word.corrected = word.typed == word.expected;
            }
            words.push(word);
            start = end;
        }
        words
    }

    fn load_target(&mut self) -> Result<(), SessionError> {
        let target = self.source.generate(&GenerateOptions {
            mode: self.config.text_mode,
            word_limit: if self.config.is_words_mode() {
                self.config.word_count
            } else {
                0
            },
            language: self.config.language.clone(),
            punctuation: self.config.punctuation,
            numbers: self.config.numbers,
            blind: self.config.blind,
            zen: self.config.zen,
            min_wpm: self.config.min_wpm,
            seed: self.seed,
        })?;
        self.target_chars = target.chars().collect();
        self.target = target;
        self.index_words();
        Ok(())
    }

    fn index_words(&mut self) {
        self.word_at.clear();
        let mut word = 0;
        for (index, character) in self.target_chars.iter().enumerate() {
            if *character != ' ' && index > 0 && self.target_chars[index - 1] == ' ' {
                word += 1;
            }
            self.word_at.push(word);
        }
        self.missed = vec![false; word.saturating_add(1)];
    }

    fn record_event(&mut self, kind: ReplayEventKind, character: Option<char>) {
        self.events.push(ReplayEvent {
            offset: self.elapsed(),
            kind,
            character,
        });
    }

    fn mark_missed(&mut self, position: usize) {
        let Some(last_index) = self.word_at.len().checked_sub(1) else {
            return;
        };
        let word_index = self.word_at[position.min(last_index)];
        if let Some(missed) = self.missed.get_mut(word_index) {
            *missed = true;
        }
    }

    fn update_caps_streak(&mut self, typed: char, expected: char) {
        if !has_case(typed) || !has_case(expected) {
            return;
        }
        if typed.is_uppercase() != expected.is_uppercase() {
            self.caps_inversions += 1;
        } else {
            self.caps_inversions = 0;
        }
    }

    fn check_min_wpm(&mut self) {
        if self.config.min_wpm <= 0 || !matches!(self.state, SessionState::Active) {
            return;
        }
        if self.elapsed() < MIN_WPM_GRACE {
            return;
        }
        if (self.live_wpm() as i32) < self.config.min_wpm {
            self.failed = true;
            self.failure_reason = format!("WPM below minimum ({})", self.config.min_wpm);
            self.finish();
        }
    }

    fn backspace_internal(&mut self) -> bool {
        if matches!(self.state, SessionState::Finished) || self.input.is_empty() {
            return false;
        }
        let position = self.input.len();
        if !self.extras_at(position).is_empty() {
            self.drop_extras(position, self.extras_at(position).len() - 1);
            return true;
        }
        if self.input[position - 1] == SKIP_CHAR {
            let mut start = position;
            while start > 0 && self.input[start - 1] == SKIP_CHAR {
                start -= 1;
            }
            self.truncate_input(start);
            return true;
        }
        self.truncate_input(position - 1);
        true
    }

    fn delete_word_internal(&mut self) -> bool {
        if matches!(self.state, SessionState::Finished) || self.input.is_empty() {
            return false;
        }
        let position = self.input.len();
        self.drop_extras(position, 0);
        let start = word_start(position, &self.input);
        if start != position {
            self.truncate_input(start);
            return true;
        }
        if self.input[position - 1] != ' ' {
            return false;
        }
        self.truncate_input(word_start(position - 1, &self.input));
        true
    }

    fn drop_extras(&mut self, position: usize, keep: usize) {
        let length = self.extras_at(position).len();
        if length <= keep {
            return;
        }
        // Extras are capped at MAX_EXTRAS, so this conversion cannot truncate.
        self.counts.extra -= (length - keep) as i32;
        if keep == 0 {
            self.extras.remove(&position);
        } else if let Some(extra) = self.extras.get_mut(&position) {
            extra.truncate(keep);
        }
        self.extras_revision = self.extras_revision.saturating_add(1);
    }

    fn truncate_input(&mut self, length: usize) {
        let old_length = self.input.len();
        for position in (length + 1)..=old_length {
            self.drop_extras(position, 0);
        }
        for (index, character) in self.input[length..].iter().enumerate() {
            let position = length + index;
            if *character == SKIP_CHAR {
                if self
                    .target_chars
                    .get(position)
                    .is_some_and(|target| *target != ' ')
                {
                    self.skipped -= 1;
                }
            } else if position >= self.target_chars.len() {
                self.counts.extra -= 1;
            } else if *character == self.target_chars[position] {
                self.counts.correct -= 1;
            } else {
                self.counts.incorrect -= 1;
            }
        }
        self.input.truncate(length);
    }

    fn record_wpm_snapshot(&mut self) {
        if !matches!(self.state, SessionState::Active) {
            return;
        }
        let second = self.elapsed().as_secs() as usize;
        self.grow_series(second);
        self.wpm_history[second] =
            stats::wpm(self.counts.correct, self.elapsed().max(MIN_RATED_TIME));
    }

    fn bucket_keystroke(&mut self, correct: bool) {
        let second = self.elapsed().as_secs() as usize;
        self.grow_series(second);
        self.seconds[second].typed += 1;
        if !correct {
            self.seconds[second].errors += 1;
        }
    }

    fn grow_series(&mut self, second: usize) {
        while self.wpm_history.len() <= second {
            self.wpm_history.push(0.0);
        }
        while self.seconds.len() <= second {
            self.seconds.push(SecondBucket::default());
        }
    }

    fn per_second_samples(&self) -> (Vec<f64>, Vec<i32>) {
        if self.seconds.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let elapsed = self.elapsed();
        let full_seconds = elapsed.as_secs() as usize;
        let mut raw = Vec::new();
        let mut errors = Vec::new();
        for (index, bucket) in self.seconds.iter().enumerate() {
            let interval = if index >= full_seconds {
                let partial = elapsed.saturating_sub(Duration::from_secs(full_seconds as u64));
                if partial < PARTIAL_SECOND_FLOOR {
                    break;
                }
                partial
            } else {
                Duration::from_secs(1)
            };
            raw.push(stats::wpm(bucket.typed, interval));
            errors.push(bucket.errors);
        }
        (raw, errors)
    }

    fn skip_current_word(&mut self, position: usize) {
        self.keystrokes_incorrect += 1;
        if let Some(expected) = self.target_chars.get(position) {
            *self.char_errors.entry(expected.to_string()).or_default() += 1;
        }
        self.bucket_keystroke(false);
        self.mark_missed(position);
        let end = word_end_at(&self.target_chars, position);
        for _ in position..end {
            self.input.push(SKIP_CHAR);
            self.skipped += 1;
        }
        if self
            .target_chars
            .get(end)
            .is_some_and(|character| *character == ' ')
        {
            self.input.push(SKIP_CHAR);
        }
    }

    fn finish(&mut self) {
        self.record_wpm_snapshot();
        self.state = SessionState::Finished;
        self.ended_at = Some(self.clock.now());
    }
}

fn resolve_seed(configured: i64) -> i64 {
    if configured != 0 {
        return configured;
    }
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_nanos() & (i64::MAX as u128)) as i64,
        Err(_) => 1,
    }
}

fn completed_words(input: &[char], target: &[char]) -> usize {
    if target.is_empty() {
        return 0;
    }
    if input.len() >= target.len() {
        return count_words(target);
    }
    input
        .iter()
        .enumerate()
        .filter(|(index, _)| target[*index] == ' ')
        .count()
}

fn count_words(text: &[char]) -> usize {
    if text.is_empty() {
        return 0;
    }
    1 + text.iter().filter(|character| **character == ' ').count()
}

fn has_case(character: char) -> bool {
    character.is_uppercase() || character.is_lowercase()
}

fn word_start(position: usize, input: &[char]) -> usize {
    input[..position]
        .iter()
        .rposition(|character| *character == ' ')
        .map_or(0, |index| index + 1)
}

fn word_start_at(target: &[char], position: usize) -> usize {
    let mut start = position.min(target.len());
    while start > 0 && target[start - 1] != ' ' {
        start -= 1;
    }
    start
}

fn word_end_at(target: &[char], start: usize) -> usize {
    target[start..]
        .iter()
        .position(|character| *character == ' ')
        .map_or(target.len(), |offset| start + offset)
}
