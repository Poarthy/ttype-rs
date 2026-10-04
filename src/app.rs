use std::fs;
use std::io::{self, IsTerminal, Read};

use anyhow::{Context, Result, anyhow};
use clap::CommandFactory;
use clap_complete::{generate, shells};

use crate::assets::builtin_provider;
use crate::cli::{Cli, Command, ConfigArgs, OutputFormat, Shell};
use crate::clock::RealClock;
use crate::config::{self, Paths};
use crate::domain::{RunResult, TextMode};
use crate::session::Session;
use crate::storage;
use crate::tui::TuiApp;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => run_test(cli.run),
        Some(Command::Run(args)) => run_test(args),
        Some(Command::Config(args)) => run_config(args),
        Some(Command::History(args)) => run_history(args),
        Some(Command::Stats(args)) => run_stats(args),
        Some(Command::Clear(args)) => run_clear(args),
        Some(Command::Doctor) => run_doctor(),
        Some(Command::Completion { shell }) => run_completion(shell),
        Some(Command::Version) => {
            println!("ttype {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(Command::Languages { command }) => {
            println!(
                "languages: {}",
                if command.is_some() {
                    "download is not available in this offline build"
                } else {
                    "english (built-in)"
                }
            );
            Ok(())
        }
        Some(Command::Update) => Err(anyhow!("update checking is not available in this build")),
        Some(Command::Uninstall(_)) => Err(anyhow!("uninstall is not available in this build")),
    }
}

fn run_test(args: crate::cli::RunArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let settings = config::load(&paths)?;
    let mut configuration = config::resolve_test_config(&settings, &args);
    let custom = read_custom_text(&args)?;
    let target = if let Some(text) = custom {
        configuration.text_mode = TextMode::Custom;
        text
    } else {
        builtin_provider().generate(&crate::domain::GenerateOptions {
            mode: configuration.text_mode,
            word_limit: configuration.word_count,
            language: configuration.language.clone(),
            punctuation: configuration.punctuation,
            numbers: configuration.numbers,
            seed: configuration.seed,
            ..crate::domain::GenerateOptions::default()
        })?
    };
    if configuration.text_mode == TextMode::Custom
        && configuration.word_count == 0
        && args.time.is_none()
    {
        configuration.kind = crate::domain::TestKind::Words;
        configuration.word_count = target.split_whitespace().count();
    }
    if matches!(args.output, Some(OutputFormat::Json)) || !io::stdout().is_terminal() {
        return Err(anyhow!("interactive typing requires a terminal"));
    }
    let session = Session::new(configuration, target, RealClock::new())?;
    let finished = TuiApp::new(session).run()?;
    if let Ok(result) = finished.result() {
        persist_result(&paths, &args, &result, finished.target(), finished.events())?;
    }
    Ok(())
}

fn persist_result(
    paths: &Paths,
    args: &crate::cli::RunArgs,
    result: &RunResult,
    target: &str,
    events: &[crate::replay::ReplayEvent],
) -> Result<()> {
    if !args.no_save {
        let id = storage::save_result(paths, result)?;
        storage::save_replay(
            paths,
            &id,
            &crate::replay::Replay {
                target: target.to_owned(),
                events: events.to_vec(),
            },
        )?;
    }
    if let Some(path) = &args.result_file {
        let body = serde_json::json!({ "version": 1, "status": if result.failed { "failed_min_wpm" } else { "completed" }, "duration_s": result.duration.as_secs_f64(), "wpm": result.wpm, "raw": result.raw_wpm, "accuracy": result.accuracy, "consistency": result.consistency, "mode": result.config.text_mode.as_str(), "tag": result.config.tag });
        fs::write(path, format!("{}\n", serde_json::to_string_pretty(&body)?))
            .context("write result file")?;
    }
    Ok(())
}

fn read_custom_text(args: &crate::cli::RunArgs) -> Result<Option<String>> {
    let given = usize::from(args.text.is_some()) + usize::from(args.file.is_some());
    if given > 1 {
        return Err(anyhow!("give the text one way: piped in, --file or --text"));
    }
    if let Some(text) = &args.text {
        return clean_custom_text(text).map(Some);
    }
    if let Some(path) = &args.file {
        let raw = if path.as_os_str() == "-" {
            let mut raw = String::new();
            io::stdin().read_to_string(&mut raw)?;
            raw
        } else {
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?
        };
        return clean_custom_text(&raw).map(Some);
    }
    Ok(None)
}

fn clean_custom_text(raw: &str) -> Result<String> {
    let cleaned = raw
        .replace("\r\n", "\n")
        .replace('\t', " ")
        .replace(['“', '”', '„', '«', '»'], "\"")
        .replace(['‘', '’', '‚'], "'")
        .replace(['–', '—', '−'], "-")
        .replace('…', "...");
    let lines: Vec<_> = cleaned
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let out = lines.join("\n").trim_end_matches('\n').to_owned();
    if out.trim().is_empty() {
        return Err(anyhow!("custom text has no text to type"));
    }
    if out.len() > 1 << 20 {
        return Err(anyhow!("more than 1 MiB of text"));
    }
    Ok(out)
}

fn run_config(args: ConfigArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let mut settings = config::load(&paths)?;
    if args == ConfigArgs::default() {
        print!("{}", config::format_settings(&settings, &paths));
        return Ok(());
    }
    config::apply(&mut settings, &args)?;
    config::save(&paths, &settings)?;
    print!("{}", config::format_settings(&settings, &paths));
    Ok(())
}

fn run_history(args: crate::cli::HistoryArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let mut history = storage::load_history(&paths)?;
    history.reverse();
    let limit = args.limit.unwrap_or(args.last);
    println!("Date              Mode       WPM    Raw   Acc  Err");
    for item in history
        .iter()
        .filter(|item| args.tag.as_ref().is_none_or(|tag| &item.tag == tag))
        .take(limit)
    {
        println!(
            "{:<16}  {:<9} {:>5.0} {:>6.0} {:>4.0}% {:>4}",
            item.timestamp.chars().take(16).collect::<String>(),
            item.mode,
            item.result.wpm,
            item.result.raw_wpm,
            item.result.accuracy,
            item.result.incorrect
        );
    }
    Ok(())
}

fn run_stats(args: crate::cli::StatsArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let records: Vec<_> = storage::load_history(&paths)?
        .into_iter()
        .filter(|item| {
            args.mode.is_none_or(|mode| item.mode == mode)
                && args.tag.as_ref().is_none_or(|tag| &item.tag == tag)
                && (!args.exclude_failed || !item.result.failed)
        })
        .collect();
    if records.is_empty() {
        println!("No test history yet.");
        return Ok(());
    }
    if args.export.is_some() {
        println!("timestamp,mode,wpm,raw_wpm,accuracy,failed");
        for item in records {
            println!(
                "{},{},{:.2},{:.2},{:.2},{}",
                item.timestamp,
                item.mode,
                item.result.wpm,
                item.result.raw_wpm,
                item.result.accuracy,
                item.result.failed
            );
        }
        return Ok(());
    }
    let count = records.len() as f64;
    let average_wpm = records.iter().map(|item| item.result.wpm).sum::<f64>() / count;
    let average_accuracy = records.iter().map(|item| item.result.accuracy).sum::<f64>() / count;
    let best = records
        .iter()
        .map(|item| item.result.wpm)
        .fold(0.0, f64::max);
    println!(
        "tests {}\naverage wpm {:.2}\naverage accuracy {:.2}%\nbest wpm {:.2}",
        records.len(),
        average_wpm,
        average_accuracy,
        best
    );
    Ok(())
}

fn run_clear(args: crate::cli::ClearArgs) -> Result<()> {
    let paths = Paths::discover()?;
    if !args.yes {
        return Err(anyhow!("refusing to delete without --yes"));
    }
    storage::clear(&paths, args.target)?;
    println!(
        "Deleted {}.",
        match args.target {
            crate::cli::ClearTarget::History => "history",
            crate::cli::ClearTarget::Languages => "languages",
            crate::cli::ClearTarget::All => "everything",
        }
    );
    Ok(())
}
fn run_doctor() -> Result<()> {
    let paths = Paths::discover()?;
    fs::create_dir_all(&paths.data)?;
    let terminal = io::stdout().is_terminal();
    println!(
        "  [{}] terminal     {}\n  [ok  ] color        {}\n  [ok  ] data dir     {}",
        if terminal { "ok  " } else { "fail" },
        if terminal {
            "terminal"
        } else {
            "not a terminal"
        },
        std::env::var("TERM").unwrap_or_else(|_| "unset".to_owned()),
        paths.data.display()
    );
    if terminal {
        Ok(())
    } else {
        Err(anyhow!("1 check(s) need attention"))
    }
}
fn run_completion(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    match shell {
        Shell::Bash => generate(shells::Bash, &mut command, "ttype", &mut io::stdout()),
        Shell::Zsh => generate(shells::Zsh, &mut command, "ttype", &mut io::stdout()),
        Shell::Fish => generate(shells::Fish, &mut command, "ttype", &mut io::stdout()),
        Shell::Powershell => generate(shells::PowerShell, &mut command, "ttype", &mut io::stdout()),
    };
    Ok(())
}
