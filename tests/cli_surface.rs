use clap::Parser;
use std::process::Command as ProcessCommand;
use ttype_core::cli::{Cli, Command};

#[test]
fn root_without_a_subcommand_selects_a_run() {
    let parsed = Cli::try_parse_from(["ttype"]);
    assert!(parsed.is_ok());
    let cli = match parsed {
        Ok(value) => value,
        Err(error) => panic!("root CLI should parse: {error}"),
    };
    assert!(cli.command.is_none());
}

#[test]
fn explicit_run_subcommand_parses() {
    let parsed = Cli::try_parse_from(["ttype", "run"]);
    assert!(parsed.is_ok());
    let cli = match parsed {
        Ok(value) => value,
        Err(error) => panic!("run CLI should parse: {error}"),
    };
    assert!(matches!(cli.command, Some(Command::Run(_))));
}

#[test]
fn invalid_subcommand_is_a_clap_usage_error() {
    let parsed = Cli::try_parse_from(["ttype", "no-such-command"]);
    assert!(parsed.is_err());
}

#[test]
fn parse_errors_use_the_go_cli_exit_status() {
    let output = match ProcessCommand::new(env!("CARGO_BIN_EXE_ttype"))
        .arg("no-such-command")
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("run ttype: {error}"),
    };
    assert_eq!(output.status.code(), Some(1));
}
