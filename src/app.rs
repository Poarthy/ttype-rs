use std::fs;
use std::io::{self, IsTerminal};

use anyhow::{Context, Result, anyhow};
use clap::CommandFactory;
use clap_complete::{generate, shells};

use crate::assets::builtin_provider;
use crate::cli::{Cli, Command, ConfigArgs, OutputFormat, Shell};
use crate::clock::RealClock;
use crate::config::{self, Paths};
use crate::custom_text::{self, CustomInput};
use crate::domain::{RunResult, TextMode};
use crate::result_file::{self, ResultSource};
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
            println!("ttype {}", crate::BUILD_VERSION);
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
        Some(Command::Update) => run_update(),
        Some(Command::Uninstall(args)) => run_uninstall(args),
    }
}

fn run_test(args: crate::cli::RunArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let settings = config::load(&paths)?;
    let mut configuration = config::resolve_test_config(&settings, &args);
    let custom = read_custom_text(&args)?;
    if args.mode == Some(TextMode::Custom) {
        return Err(anyhow!(
            "custom text is piped in or given with --file or --text, not --mode"
        ));
    }
    if custom.is_some() {
        for (set, name) in [
            (args.mode.is_some(), "mode"),
            (args.language.is_some(), "language"),
            (args.punctuation.is_some(), "punctuation"),
            (args.numbers.is_some(), "numbers"),
        ] {
            if set {
                return Err(anyhow!(
                    "--{name} generates text, so it can't be used with your own"
                ));
            }
        }
    }
    let custom_source = custom.clone();
    let target = if let Some(text) = custom {
        configuration = custom_text::with_custom_config(
            configuration,
            &text,
            args.words.is_some(),
            args.time.is_some(),
        );
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        if configuration.text_mode == TextMode::Custom {
            return Err(anyhow!(
                "custom text is piped in or given with --file or --text, not --mode"
            ));
        }
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
    if !io::stdout().is_terminal() && !matches!(args.output, Some(OutputFormat::Json)) {
        return Err(anyhow!("interactive typing requires a terminal"));
    }
    let session = Session::new(configuration, target, RealClock::new())?;
    let application = TuiApp::new(session).with_paths(paths.clone());
    let finished = if matches!(args.output, Some(OutputFormat::Json)) && !io::stdout().is_terminal()
    {
        run_tui_on_controlling_terminal(application)?
    } else {
        application.run()?
    };
    if let Ok(result) = finished.result() {
        persist_result(&paths, &args, &result, finished.target(), finished.events())?;
        if matches!(args.output, Some(OutputFormat::Json)) {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    } else if let Some(path) = &args.result_file {
        let source = ResultSource {
            file: args.file.as_ref().and_then(|path| path.to_str()),
            text: custom_source.as_deref(),
        };
        result_file::write(path, None, false, source)?;
    }
    Ok(())
}

#[cfg(unix)]
fn run_tui_on_controlling_terminal(application: TuiApp) -> Result<Session> {
    let terminal = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .context("open controlling terminal for --output json")?;
    application.run_with_writer(terminal).map_err(Into::into)
}

#[cfg(windows)]
fn run_tui_on_controlling_terminal(application: TuiApp) -> Result<Session> {
    let terminal = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONOUT$")
        .context("open controlling terminal for --output json")?;
    application.run_with_writer(terminal).map_err(Into::into)
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
        if !events.is_empty() {
            storage::save_replay(
                paths,
                &id,
                &crate::replay::Replay {
                    target: target.to_owned(),
                    events: events.to_vec(),
                },
            )?;
        }
    }
    if let Some(path) = &args.result_file {
        let source = if result.config.text_mode == TextMode::Custom {
            ResultSource {
                file: args.file.as_ref().and_then(|path| path.to_str()),
                text: Some(target),
            }
        } else {
            ResultSource::default()
        };
        result_file::write(path, Some(result), true, source)?;
    }
    Ok(())
}

fn read_custom_text(args: &crate::cli::RunArgs) -> Result<Option<String>> {
    let stdin_piped = !io::stdin().is_terminal();
    if !stdin_piped && args.file.is_none() && args.text.is_none() {
        return Ok(None);
    }
    let mut stdin = io::stdin();
    let file = args.file.as_ref().map(|path| path.to_string_lossy());
    custom_text::read(CustomInput {
        stdin: &mut stdin,
        stdin_piped,
        file: file.as_deref(),
        text: args.text.as_deref(),
    })
    .map(Some)
    .map_err(Into::into)
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
    let history: Vec<_> = storage::list_results(&paths, 0)?
        .into_iter()
        .filter(|item| args.tag.as_ref().is_none_or(|tag| &item.tag == tag))
        .collect();
    let limit = args.limit.unwrap_or(args.last);
    let history: Vec<_> = history.into_iter().take(limit).collect();
    if !args.plain && io::stdout().is_terminal() {
        let settings = config::load(&paths)?;
        let theme = match settings.theme.as_str() {
            "monokai" => crate::cli::ThemeName::Monokai,
            "dracula" => crate::cli::ThemeName::Dracula,
            _ => crate::cli::ThemeName::Default,
        };
        return crate::tui::run_history(&paths, history, theme).map_err(Into::into);
    }
    if history.is_empty() {
        println!("No test history yet.");
        return Ok(());
    }
    println!("Date              Lang        Time      WPM    Raw   Acc  Err");
    for item in &history {
        println!(
            "{:<16}  {:<10}  {:<6}  {:>6.0}  {:>6.0}  {:>4.0}%  {:>4}",
            history_date(&item.timestamp),
            history_language(&item.language),
            item.test_label(),
            item.wpm,
            item.raw_wpm,
            item.accuracy,
            item.incorrect
        );
    }
    Ok(())
}

fn run_stats(args: crate::cli::StatsArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let records = storage::filter_results(
        storage::load_history(&paths)?,
        args.mode,
        args.tag.as_deref(),
        args.exclude_failed,
    );
    if records.is_empty() {
        println!("No test history yet.");
        return Ok(());
    }
    if args.export.is_some() {
        print!(
            "{}",
            storage::export_csv(&paths, args.mode, args.tag.as_deref(), args.exclude_failed)?
        );
        return Ok(());
    }
    let count = records.len() as f64;
    let average_wpm = records.iter().map(|item| item.wpm).sum::<f64>() / count;
    let average_accuracy = records.iter().map(|item| item.accuracy).sum::<f64>() / count;
    let best = records.iter().map(|item| item.wpm).fold(0.0, f64::max);
    println!(
        "tests {}\naverage wpm {:.2}\naverage accuracy {:.2}%\nbest wpm {:.2}",
        records.len(),
        average_wpm,
        average_accuracy,
        best
    );
    if args.trend {
        let values: Vec<_> = records.iter().rev().map(|item| item.wpm).collect();
        println!("trend {}", render_sparkline(&values));
    }
    Ok(())
}

fn render_sparkline(values: &[f64]) -> String {
    let glyphs: Vec<char> = "▁▂▃▄▅▆▇█".chars().collect();
    let max = values.iter().copied().fold(0.0, f64::max).max(1.0);
    values
        .iter()
        .map(|value| {
            glyphs
                [((value / max * (glyphs.len() - 1) as f64).round() as usize).min(glyphs.len() - 1)]
        })
        .collect()
}

fn history_language(id: &str) -> String {
    let name = if id.is_empty() {
        "english".to_owned()
    } else {
        id.replace('_', " ")
    };
    let chars: Vec<_> = name.chars().collect();
    if chars.len() <= 10 {
        name
    } else {
        format!("{}…", chars[..9].iter().collect::<String>())
    }
}

fn history_date(timestamp: &str) -> String {
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = timestamp
        .get(5..7)
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|month| months.get(month.saturating_sub(1)))
        .copied()
        .unwrap_or("???");
    let day = timestamp.get(8..10).unwrap_or("??").trim_start_matches('0');
    let time = timestamp.get(11..16).unwrap_or("??:??");
    format!("{month} {day:>2} {time}")
}

fn run_clear(args: crate::cli::ClearArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let target = args.target.unwrap_or(crate::cli::ClearTarget::History);
    if !args.yes {
        use std::io::Write as _;
        print!("Delete {}? [y/N] ", clear_description(target));
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Nothing was deleted.");
            return Ok(());
        }
    }
    storage::clear(&paths, target)?;
    println!("Deleted {}.", clear_description(target));
    Ok(())
}

fn clear_description(target: crate::cli::ClearTarget) -> &'static str {
    match target {
        crate::cli::ClearTarget::History => "your test history, personal bests and replays",
        crate::cli::ClearTarget::Languages => "every downloaded language list",
        crate::cli::ClearTarget::All => {
            "your test history, personal bests, replays and downloaded languages"
        }
    }
}

fn run_update() -> Result<()> {
    let paths = Paths::discover()?;
    let state_path = paths.data.join("update.json");
    let local = crate::BUILD_VERSION;
    let plan = crate::update::InstallPlan::current()?;
    // Match Go's refusal order: package-managed and source binaries never
    // make a network request for an update they cannot apply.
    match plan.kind {
        crate::update::InstallKind::Source => {
            return Err(anyhow!(
                "ttype was built from source; pull and rebuild it to update"
            ));
        }
        crate::update::InstallKind::PackageManager => {
            return Err(anyhow!(
                "ttype was installed by a package manager; update it there"
            ));
        }
        crate::update::InstallKind::ReleaseScript => {}
    }
    let state = crate::update::check_daily(&state_path, local, crate::update::RELEASES_URL)?;
    if state.latest.is_empty() {
        println!("ttype {local} is already the latest version.");
        return Ok(());
    }
    println!("Downloading ttype {}...", state.latest);
    crate::update::install(
        &plan.executable,
        local,
        &state.latest,
        crate::update::RELEASES_URL,
        &plan,
        true,
        &state_path,
    )?;
    println!("Updated to ttype {}.", state.latest);
    Ok(())
}

fn run_uninstall(args: crate::cli::UninstallArgs) -> Result<()> {
    let paths = Paths::discover()?;
    let plan = crate::update::InstallPlan::current()?;
    let targets = crate::uninstall::targets(&paths, &plan);
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    crate::uninstall::run(targets, args.purge, args.yes, &mut input, &mut output)?;
    Ok(())
}
fn run_doctor() -> Result<()> {
    let paths = Paths::discover()?;
    let (output, failed) = crate::doctor::render(&crate::doctor::run(&paths.data));
    print!("{output}");
    if failed == 0 {
        Ok(())
    } else {
        Err(anyhow!("{failed} check(s) need attention"))
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
