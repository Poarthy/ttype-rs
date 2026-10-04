use std::fmt::Write as _;
use std::io::IsTerminal;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub note: String,
    pub hint: String,
}

pub fn run(data_path: &Path) -> Vec<Check> {
    vec![
        terminal(),
        color(),
        locale(),
        clipboard(),
        data_dir(data_path),
    ]
}

pub fn terminal() -> Check {
    if !std::io::stdout().is_terminal() {
        return Check {
            name: "terminal",
            ok: false,
            note: "not a terminal".to_owned(),
            hint: "run ttype directly rather than through a pipe".to_owned(),
        };
    }
    match crossterm::terminal::size() {
        Ok((width, height)) if width >= 40 && height >= 12 => Check {
            name: "terminal",
            ok: true,
            note: format!("{width}x{height}"),
            hint: String::new(),
        },
        Ok((width, height)) => Check {
            name: "terminal",
            ok: false,
            note: format!("{width}x{height}"),
            hint: "ttype wants at least 40x12; the layout degrades below that".to_owned(),
        },
        Err(error) => Check {
            name: "terminal",
            ok: false,
            note: "size unknown".to_owned(),
            hint: error.to_string(),
        },
    }
}

pub fn color() -> Check {
    color_with(
        std::env::var("TERM").unwrap_or_default(),
        std::env::var_os("NO_COLOR").is_some(),
    )
}

pub fn color_with(terminal: String, no_color: bool) -> Check {
    if terminal.is_empty() || terminal == "dumb" {
        Check {
            name: "color",
            ok: false,
            note: "no color support".to_owned(),
            hint: "set TERM (for example xterm-256color)".to_owned(),
        }
    } else if no_color {
        Check {
            name: "color",
            ok: true,
            note: "disabled by NO_COLOR".to_owned(),
            hint: String::new(),
        }
    } else {
        Check {
            name: "color",
            ok: true,
            note: terminal,
            hint: String::new(),
        }
    }
}

pub fn locale() -> Check {
    locale_with(
        ["LC_ALL", "LC_CTYPE", "LANG"]
            .into_iter()
            .map(|key| std::env::var(key).unwrap_or_default()),
    )
}

pub fn locale_with(values: impl IntoIterator<Item = String>) -> Check {
    for value in values {
        if value.is_empty() {
            continue;
        }
        let uppercase = value.to_uppercase();
        if uppercase.contains("UTF-8") || uppercase.contains("UTF8") {
            return Check {
                name: "locale",
                ok: true,
                note: value,
                hint: String::new(),
            };
        }
        return Check {
            name: "locale",
            ok: false,
            note: value,
            hint: "set a UTF-8 locale for the chart and big digits".to_owned(),
        };
    }
    Check {
        name: "locale",
        ok: true,
        note: "unset, assuming UTF-8".to_owned(),
        hint: String::new(),
    }
}

pub fn clipboard() -> Check {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["pbcopy"]
    } else if cfg!(windows) {
        &["clip"]
    } else {
        &["wl-copy", "xclip", "xsel"]
    };
    if let Some(name) = candidates
        .iter()
        .copied()
        .find(|name| executable_in_path(name))
    {
        return Check {
            name: "clipboard",
            ok: true,
            note: name.to_owned(),
            hint: String::new(),
        };
    }
    Check {
        name: "clipboard",
        ok: false,
        note: "no clipboard tool".to_owned(),
        hint: format!("install one of: {}", candidates.join(", ")),
    }
}

fn executable_in_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| {
        let direct = directory.join(name);
        if direct.is_file() {
            return true;
        }
        cfg!(windows) && directory.join(format!("{name}.exe")).is_file()
    })
}

pub fn data_dir(path: &Path) -> Check {
    let probe = path.join(".doctor");
    let outcome = std::fs::create_dir_all(path).and_then(|()| std::fs::write(&probe, b"ok"));
    match outcome {
        Ok(()) => {
            let _ = std::fs::remove_file(probe);
            Check {
                name: "data dir",
                ok: true,
                note: path.display().to_string(),
                hint: String::new(),
            }
        }
        Err(error) => Check {
            name: "data dir",
            ok: false,
            note: path.display().to_string(),
            hint: error.to_string(),
        },
    }
}

pub fn render(checks: &[Check]) -> (String, usize) {
    let mut output = String::new();
    let mut failed = 0;
    for check in checks {
        let mark = if check.ok {
            "ok  "
        } else {
            failed += 1;
            "fail"
        };
        let _ = writeln!(output, "  [{mark}] {:<12} {}", check.name, check.note);
        if !check.ok && !check.hint.is_empty() {
            let _ = writeln!(output, "         {}", check.hint);
        }
    }
    (output, failed)
}
