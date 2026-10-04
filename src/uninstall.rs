use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use thiserror::Error;

use crate::config::Paths;
use crate::update::{InstallKind, InstallPlan};

#[derive(Debug, Error)]
pub enum UninstallError {
    #[error("uninstall I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub struct Targets {
    pub binary: Option<PathBuf>,
    pub man_pages: Vec<PathBuf>,
    pub config: PathBuf,
    pub data: PathBuf,
    pub managed_note: Option<String>,
}

pub fn targets(paths: &Paths, plan: &InstallPlan) -> Targets {
    let managed = plan.kind == InstallKind::PackageManager;
    let mut man_pages = Vec::new();
    if !cfg!(windows) && !managed {
        man_pages.push(PathBuf::from("/usr/local/share/man/man1/ttype.1"));
        if let Some(home) = dirs::home_dir() {
            man_pages.push(home.join(".local/share/man/man1/ttype.1"));
        }
    }
    Targets {
        binary: (!managed).then(|| plan.executable.clone()),
        man_pages,
        config: paths.config.clone(),
        data: paths.data.clone(),
        managed_note: managed.then_some("ttype was installed by a package manager, so its files are left alone; remove it there.".to_owned()),
    }
}

pub fn run(
    mut targets: Targets,
    purge: bool,
    yes: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<(), UninstallError> {
    if let Some(note) = targets.managed_note.take() {
        writeln!(output, "{note}")?;
    }
    if !purge {
        writeln!(
            output,
            "Your settings and history are kept; pass --purge to delete them too."
        )?;
    }
    let existing_man: Vec<_> = targets
        .man_pages
        .into_iter()
        .filter(|path| path.exists())
        .collect();
    let mut names = Vec::new();
    if let Some(binary) = &targets.binary {
        names.push(("binary", binary.clone()));
    }
    names.extend(existing_man.into_iter().map(|path| ("man", path)));
    if purge {
        if targets.config.exists() {
            names.push(("config", targets.config));
        }
        if targets.data.exists() {
            names.push(("data", targets.data));
        }
    }
    if names.is_empty() {
        writeln!(output, "Nothing to remove.")?;
        return Ok(());
    }
    writeln!(output, "This will remove:")?;
    for (kind, path) in &names {
        writeln!(output, "  {kind:<7} {}", path.display())?;
    }
    if !yes {
        write!(output, "Continue? [y/N] ")?;
        output.flush()?;
        let mut line = String::new();
        input.read_line(&mut line)?;
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            writeln!(output, "Nothing was removed.")?;
            return Ok(());
        }
    }
    let mut failures = Vec::new();
    for (_, path) in names {
        let removed = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        match removed {
            Ok(()) => writeln!(output, "  removed {}", path.display())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(UninstallError::Io(std::io::Error::other(
            failures.join("\n"),
        )))
    }
}
