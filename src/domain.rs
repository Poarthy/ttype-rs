use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    #[default]
    Timed,
    Words,
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum,
)]
#[serde(rename_all = "lowercase")]
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
    Rust,
    Custom,
}

impl TextMode {
    pub const fn all() -> &'static [Self] {
        &[
            Self::Words,
            Self::Sentences,
            Self::Sql,
            Self::Go,
            Self::Backend,
            Self::Python,
            Self::Shell,
            Self::Regex,
            Self::Rust,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Words => "words",
            Self::Sentences => "sentences",
            Self::Sql => "sql",
            Self::Go => "go",
            Self::Backend => "backend",
            Self::Python => "python",
            Self::Shell => "shell",
            Self::Regex => "regex",
            Self::Rust => "rust",
            Self::Custom => "custom",
        }
    }

    pub const fn commits_words_on_space(self) -> bool {
        matches!(self, Self::Words | Self::Sentences | Self::Custom)
    }
}

impl fmt::Display for TextMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown text mode {0:?}")]
pub struct ParseTextModeError(pub String);

impl FromStr for TextMode {
    type Err = ParseTextModeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "words" => Ok(Self::Words),
            "sentences" => Ok(Self::Sentences),
            "sql" => Ok(Self::Sql),
            "go" => Ok(Self::Go),
            "backend" => Ok(Self::Backend),
            "python" => Ok(Self::Python),
            "shell" => Ok(Self::Shell),
            "regex" => Ok(Self::Regex),
            "rust" => Ok(Self::Rust),
            "custom" => Ok(Self::Custom),
            _ => Err(ParseTextModeError(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TestConfig {
    pub kind: TestKind,
    #[serde(with = "duration_seconds")]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WordResult {
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub line: usize,
    pub expected: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub typed: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub missed: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub corrected: bool,
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunResult {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub timestamp: String,
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
    #[serde(with = "duration_nanoseconds")]
    pub duration: Duration,
    pub seed: i64,
    pub wpm_history: Vec<f64>,
    pub raw_wpm_history: Vec<f64>,
    pub error_history: Vec<i32>,
    pub char_errors: BTreeMap<String, i32>,
    pub failed: bool,
    pub failure_reason: String,
    pub words: Vec<WordResult>,
    #[serde(skip)]
    pub started_at: String,
}

mod duration_seconds {
    use std::time::Duration;

    use serde::Serializer;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }
}

mod duration_nanoseconds {
    use std::time::Duration;

    use serde::Serializer;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u128(duration.as_nanos())
    }
}
