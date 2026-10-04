use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use crate::domain::TextMode;

#[derive(Debug, Parser)]
#[command(
    name = "ttype",
    about = "Terminal typing practice",
    long_about = "A terminal-first typing test. Use --time for timed tests or --words for word count tests.",
    version = crate::BUILD_VERSION
)]
pub struct Cli {
    #[command(flatten)]
    pub run: RunArgs,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Config(ConfigArgs),
    History(HistoryArgs),
    Stats(StatsArgs),
    Languages {
        #[command(subcommand)]
        command: Option<LanguagesCommand>,
    },
    Clear(ClearArgs),
    Doctor,
    Update,
    Completion {
        shell: Shell,
    },
    Version,
    Uninstall(UninstallArgs),
}

#[derive(Clone, Debug, Args, Default, PartialEq)]
pub struct RunArgs {
    #[arg(long, help = "Timed test duration in seconds")]
    pub time: Option<u64>,
    #[arg(long, help = "Word count test")]
    pub words: Option<usize>,
    #[arg(long, help = "Language id")]
    pub language: Option<String>,
    #[arg(long, help = "Text mode")]
    pub mode: Option<TextMode>,
    #[arg(long, help = "Color theme")]
    pub theme: Option<ThemeName>,
    #[arg(long, help = "Typing area width in characters")]
    pub width: Option<usize>,
    #[arg(long, action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "Inject punctuation between words")]
    pub punctuation: Option<bool>,
    #[arg(long, action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "Inject numbers into word tests")]
    pub numbers: Option<bool>,
    #[arg(long, action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "Hide correctness while typing")]
    pub blind: Option<bool>,
    #[arg(long, action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "Words only: no header or hints")]
    pub zen: Option<bool>,
    #[arg(long = "min-wpm", help = "Fail the test if WPM drops below this")]
    pub min_wpm: Option<i32>,
    #[arg(long, help = "Random seed for word generation")]
    pub seed: Option<i64>,
    #[arg(long, help = "Print the result instead of showing it")]
    pub output: Option<OutputFormat>,
    #[arg(long, help = "Type the text of a file (- for stdin)")]
    pub file: Option<PathBuf>,
    #[arg(long, help = "Type this text")]
    pub text: Option<String>,
    #[arg(
        long = "no-save",
        help = "Keep this run out of history, bests and replays"
    )]
    pub no_save: bool,
    #[arg(long = "result-file", help = "Write the last run as JSON on exit")]
    pub result_file: Option<PathBuf>,
    #[arg(long, help = "Label this run")]
    pub tag: Option<String>,
    #[arg(long = "allow-skip", hide = true, help = "Deprecated, has no effect")]
    pub allow_skip: bool,
}

#[derive(Clone, Debug, Args, Default, PartialEq)]
pub struct ConfigArgs {
    #[arg(long = "default-time")]
    pub default_time: Option<u64>,
    #[arg(long = "default-words")]
    pub default_words: Option<usize>,
    #[arg(long = "default-mode")]
    pub default_mode: Option<TextMode>,
    #[arg(long = "default-language")]
    pub default_language: Option<String>,
    #[arg(long)]
    pub theme: Option<ThemeName>,
    #[arg(long = "default-width")]
    pub default_width: Option<usize>,
    #[arg(long = "default-min-wpm")]
    pub default_min_wpm: Option<i32>,
    #[arg(long = "default-punctuation", action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    pub default_punctuation: Option<bool>,
    #[arg(long = "default-numbers", action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    pub default_numbers: Option<bool>,
    #[arg(long = "default-blind", action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    pub default_blind: Option<bool>,
    #[arg(long = "default-zen", action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    pub default_zen: Option<bool>,
    #[arg(long)]
    pub update: Option<UpdateMode>,
}

#[derive(Clone, Debug, Args)]
pub struct HistoryArgs {
    #[arg(long, default_value_t = 10)]
    pub last: usize,
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub plain: bool,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct StatsArgs {
    #[arg(long)]
    pub mode: Option<TextMode>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long = "exclude-failed")]
    pub exclude_failed: bool,
    #[arg(long)]
    pub trend: bool,
    #[arg(long)]
    pub export: Option<ExportFormat>,
}

#[derive(Clone, Debug, Args)]
pub struct ClearArgs {
    #[arg(value_enum)]
    pub target: Option<ClearTarget>,
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub struct UninstallArgs {
    #[arg(long)]
    pub purge: bool,
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ThemeName {
    Default,
    Monokai,
    Dracula,
}
impl ThemeName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Monokai => "monokai",
            Self::Dracula => "dracula",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum UpdateMode {
    Auto,
    Notify,
    Off,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Json,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ExportFormat {
    Csv,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ClearTarget {
    History,
    Languages,
    All,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

#[derive(Clone, Debug, Subcommand)]
pub enum LanguagesCommand {
    Download {
        #[arg(short, long)]
        yes: bool,
        #[arg(short, long, default_value_t = 8)]
        jobs: usize,
    },
}
