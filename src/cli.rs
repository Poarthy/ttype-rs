use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::domain::TextMode;

#[derive(Debug, Parser)]
#[command(name = "ttype", about = "Terminal typing practice", version)]
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
    #[arg(long)]
    pub time: Option<u64>,
    #[arg(long)]
    pub words: Option<usize>,
    #[arg(long)]
    pub language: Option<String>,
    #[arg(long)]
    pub mode: Option<TextMode>,
    #[arg(long)]
    pub theme: Option<ThemeName>,
    #[arg(long)]
    pub width: Option<usize>,
    #[arg(long)]
    pub punctuation: Option<bool>,
    #[arg(long)]
    pub numbers: Option<bool>,
    #[arg(long)]
    pub blind: Option<bool>,
    #[arg(long)]
    pub zen: Option<bool>,
    #[arg(long = "min-wpm")]
    pub min_wpm: Option<i32>,
    #[arg(long)]
    pub seed: Option<i64>,
    #[arg(long)]
    pub output: Option<OutputFormat>,
    #[arg(long)]
    pub file: Option<PathBuf>,
    #[arg(long)]
    pub text: Option<String>,
    #[arg(long = "no-save")]
    pub no_save: bool,
    #[arg(long = "result-file")]
    pub result_file: Option<PathBuf>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long = "allow-skip", hide = true)]
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
    #[arg(long = "default-punctuation")]
    pub default_punctuation: Option<bool>,
    #[arg(long = "default-numbers")]
    pub default_numbers: Option<bool>,
    #[arg(long = "default-blind")]
    pub default_blind: Option<bool>,
    #[arg(long = "default-zen")]
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
    #[arg(value_enum, default_value_t = ClearTarget::History)]
    pub target: ClearTarget,
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
