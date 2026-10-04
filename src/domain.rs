use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CharCounts {
    pub correct: i32,
    pub incorrect: i32,
    pub extra: i32,
}

impl CharCounts {
    pub fn total_typed(self) -> i32 {
        self.correct + self.incorrect + self.extra
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TestKind {
    #[default]
    Timed,
    Words,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum TextMode {
    #[default]
    Words,
    Sentences,
    Sql,
    Go,
    Backend,
    Python,
    Shell,
    Regex,
    Custom,
}

impl TextMode {
    pub const fn commits_words_on_space(self) -> bool {
        matches!(self, Self::Words | Self::Sentences | Self::Custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestConfig {
    pub kind: TestKind,
    pub duration: Duration,
    pub word_count: usize,
    pub text_mode: TextMode,
    pub language: String,
    pub theme: String,
    pub width: usize,
    pub punctuation: bool,
    pub numbers: bool,
    pub blind: bool,
    pub zen: bool,
    pub min_wpm: i32,
    pub seed: i64,
    pub tag: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            kind: TestKind::Timed,
            duration: Duration::from_secs(60),
            word_count: 0,
            text_mode: TextMode::Words,
            language: String::new(),
            theme: "default".to_owned(),
            width: 0,
            punctuation: false,
            numbers: false,
            blind: false,
            zen: false,
            min_wpm: 0,
            seed: 0,
            tag: String::new(),
        }
    }
}

impl TestConfig {
    pub const fn is_words_mode(&self) -> bool {
        matches!(self.kind, TestKind::Words) && self.word_count > 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenerateOptions {
    pub mode: TextMode,
    pub word_limit: usize,
    pub language: String,
    pub punctuation: bool,
    pub numbers: bool,
    pub blind: bool,
    pub zen: bool,
    pub min_wpm: i32,
    pub seed: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WordResult {
    pub line: usize,
    pub expected: String,
    pub typed: String,
    pub missed: bool,
    pub corrected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunResult {
    pub config: TestConfig,
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub correct: i32,
    pub incorrect: i32,
    pub keystrokes_correct: i32,
    pub keystrokes_incorrect: i32,
    pub skipped: i32,
    pub total_chars: i32,
    pub duration: Duration,
    pub seed: i64,
    pub wpm_history: Vec<f64>,
    pub raw_wpm_history: Vec<f64>,
    pub error_history: Vec<i32>,
    pub char_errors: BTreeMap<String, i32>,
    pub failed: bool,
    pub failure_reason: String,
    pub words: Vec<WordResult>,
}
