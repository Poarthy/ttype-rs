use std::fs;
use std::io;
use std::path::Path;

use clap::CommandFactory;
use clap_complete::{generate_to, shells};
use clap_mangen::Man;
use thiserror::Error;

use crate::cli::Cli;

#[derive(Debug, Error)]
pub enum CompletionError {
    #[error("completion I/O: {0}")]
    Io(#[from] io::Error),
}

/// Generates all installed shell completions and the roff page from the same
/// Clap command model used at runtime, eliminating hand-maintained drift.
pub fn generate_artifacts(directory: &Path) -> Result<(), CompletionError> {
    fs::create_dir_all(directory)?;
    let mut command = Cli::command();
    generate_to(shells::Bash, &mut command, "ttype", directory)?;
    let mut command = Cli::command();
    generate_to(shells::Zsh, &mut command, "ttype", directory)?;
    let mut command = Cli::command();
    generate_to(shells::Fish, &mut command, "ttype", directory)?;
    let mut command = Cli::command();
    generate_to(shells::PowerShell, &mut command, "ttype", directory)?;
    let command = Cli::command();
    let mut man = Vec::new();
    Man::new(command).render(&mut man)?;
    fs::write(directory.join("ttype.1"), man)?;
    Ok(())
}
