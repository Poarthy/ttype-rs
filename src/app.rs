use anyhow::Result;

use crate::cli::{Cli, Command};

/// Dispatches parsed commands. Interactive behavior is added once the engine
/// and terminal application are available; keeping dispatch here isolates it
/// from Clap's parsing and exit behavior.
pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None | Some(Command::Run(_)) => Ok(()),
        Some(Command::Config) => Ok(()),
        Some(Command::Clear) => Ok(()),
        Some(Command::Doctor) => Ok(()),
        Some(Command::Update) => Ok(()),
        Some(Command::Completion) => Ok(()),
        Some(Command::Version) => Ok(()),
    }
}
