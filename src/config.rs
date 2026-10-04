use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cli::{ConfigArgs, ThemeName, UpdateMode};
use crate::domain::{TestConfig, TestKind, TextMode};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub default_duration: u64,
    pub default_word_count: usize,
    pub default_width: usize,
    pub theme: String,
    pub language: String,
    pub default_mode: TextMode,
    pub punctuation: bool,
    pub numbers: bool,
    pub blind: bool,
    pub zen: bool,
    pub default_min_wpm: i32,
    pub onboarded: bool,
    pub update: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_duration: 60,
            default_word_count: 0,
            default_width: 0,
            theme: "default".to_owned(),
            language: String::new(),
            default_mode: TextMode::Words,
            punctuation: false,
            numbers: false,
            blind: false,
            zen: false,
            default_min_wpm: 0,
            onboarded: false,
            update: "auto".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self, ConfigError> {
        let config_base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(dirs::config_dir)
            .ok_or(ConfigError::NoConfigDir)?;
        let data_base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(dirs::data_dir)
            .ok_or(ConfigError::NoDataDir)?;
        Ok(Self {
            config: config_base.join("ttype"),
            data: data_base.join("ttype"),
        })
    }
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine config directory")]
    NoConfigDir,
    #[error("could not determine data directory")]
    NoDataDir,
    #[error("configuration I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("configuration serialization: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("word count must be at least 1")]
    WordCount,
    #[error("custom needs text each time, so it can't be the default mode")]
    CustomMode,
}

pub fn load(paths: &Paths) -> Result<Settings, ConfigError> {
    let file = paths.config_file();
    match fs::read_to_string(file) {
        Ok(raw) => Ok(toml::from_str(&raw)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(ConfigError::Io(error)),
    }
}

pub fn save(paths: &Paths, settings: &Settings) -> Result<(), ConfigError> {
    fs::create_dir_all(&paths.config)?;
    let body = toml::to_string_pretty(settings)?;
    Ok(atomic_write(&paths.config_file(), body.as_bytes())?)
}

pub fn apply(settings: &mut Settings, args: &ConfigArgs) -> Result<(), ConfigError> {
    if let Some(value) = args.default_time {
        settings.default_duration = value;
        settings.default_word_count = 0;
    }
    if let Some(value) = args.default_words {
        if value == 0 {
            return Err(ConfigError::WordCount);
        }
        settings.default_word_count = value;
    }
    if let Some(value) = args.default_mode {
        if value == TextMode::Custom {
            return Err(ConfigError::CustomMode);
        }
        settings.default_mode = value;
    }
    if let Some(value) = &args.default_language {
        settings.language = value.clone();
    }
    if let Some(value) = args.theme {
        settings.theme = value.as_str().to_owned();
    }
    if let Some(value) = args.default_width {
        settings.default_width = value;
    }
    if let Some(value) = args.default_min_wpm {
        settings.default_min_wpm = value;
    }
    if let Some(value) = args.default_punctuation {
        settings.punctuation = value;
    }
    if let Some(value) = args.default_numbers {
        settings.numbers = value;
    }
    if let Some(value) = args.default_blind {
        settings.blind = value;
    }
    if let Some(value) = args.default_zen {
        settings.zen = value;
    }
    if let Some(value) = args.update {
        settings.update = match value {
            UpdateMode::Auto => "auto",
            UpdateMode::Notify => "notify",
            UpdateMode::Off => "off",
        }
        .to_owned();
    }
    Ok(())
}

pub fn resolve_test_config(settings: &Settings, args: &crate::cli::RunArgs) -> TestConfig {
    let word_count = args.words.unwrap_or(settings.default_word_count);
    TestConfig {
        kind: if word_count > 0 {
            TestKind::Words
        } else {
            TestKind::Timed
        },
        duration: std::time::Duration::from_secs(
            args.time.unwrap_or(settings.default_duration).max(1),
        ),
        word_count,
        text_mode: args.mode.unwrap_or(settings.default_mode),
        language: args
            .language
            .clone()
            .unwrap_or_else(|| settings.language.clone()),
        theme: args
            .theme
            .unwrap_or(match_theme(&settings.theme))
            .as_str()
            .to_owned(),
        width: args.width.unwrap_or(settings.default_width),
        punctuation: args.punctuation.unwrap_or(settings.punctuation),
        numbers: args.numbers.unwrap_or(settings.numbers),
        blind: args.blind.unwrap_or(settings.blind),
        zen: args.zen.unwrap_or(settings.zen),
        min_wpm: args.min_wpm.unwrap_or(settings.default_min_wpm),
        seed: args.seed.unwrap_or(0),
        tag: args.tag.clone().unwrap_or_default(),
    }
}

fn match_theme(value: &str) -> ThemeName {
    match value {
        "monokai" => ThemeName::Monokai,
        "dracula" => ThemeName::Dracula,
        _ => ThemeName::Default,
    }
}

pub fn format_settings(settings: &Settings, paths: &Paths) -> String {
    let test = if settings.default_word_count > 0 {
        format!("{} words", settings.default_word_count)
    } else {
        format!("{}s", settings.default_duration)
    };
    format!(
        "  test         {test}\n  mode         {}\n  language     {}\n  theme        {}\n  width        {}\n  min wpm      {}\n  punctuation  {}\n  numbers      {}\n  blind        {}\n  zen          {}\n  update       {}\n\n  config       {}\n  data         {}\n",
        settings.default_mode,
        if settings.language.is_empty() {
            "english (built-in)"
        } else {
            &settings.language
        },
        settings.theme,
        if settings.default_width == 0 {
            "auto".to_owned()
        } else {
            settings.default_width.to_string()
        },
        if settings.default_min_wpm <= 0 {
            "off".to_owned()
        } else {
            settings.default_min_wpm.to_string()
        },
        on_off(settings.punctuation),
        on_off(settings.numbers),
        on_off(settings.blind),
        on_off(settings.zen),
        settings.update,
        paths.config.display(),
        paths.data.display()
    )
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

pub fn atomic_write(path: &std::path::Path, contents: &[u8]) -> Result<(), std::io::Error> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)
}
