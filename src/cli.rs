use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "ttype",
    about = "Terminal typing practice",
    long_about = "A terminal-first typing test. Use --time for timed tests or --words for word count tests.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start a typing test.
    Run(RunArgs),
    /// Show or change saved defaults.
    Config,
    /// Delete saved history or downloaded languages.
    Clear,
    /// Check terminal and environment setup.
    Doctor,
    /// Update ttype to the latest release.
    Update,
    /// Print a shell completion script.
    Completion,
    /// Print version information.
    Version,
}

#[derive(Debug, Args)]
pub struct RunArgs {}
