use clap::Parser;
use ttype_core::cli::{Cli, ConfigArgs, RunArgs};
use ttype_core::config::{ConfigError, Settings, resolve_test_config};

#[test]
fn boolean_test_flags_match_go_bare_flag_syntax() {
    let parsed = Cli::try_parse_from(["ttype", "--blind", "--zen", "--numbers", "--punctuation"])
        .unwrap_or_else(|error| panic!("parse bare booleans: {error}"));
    assert_eq!(parsed.run.blind, Some(true));
    assert_eq!(parsed.run.zen, Some(true));
    assert_eq!(parsed.run.numbers, Some(true));
    assert_eq!(parsed.run.punctuation, Some(true));

    let parsed = Cli::try_parse_from(["ttype", "--blind=false"])
        .unwrap_or_else(|error| panic!("parse explicit false: {error}"));
    assert_eq!(parsed.run.blind, Some(false));
}

#[test]
fn zero_time_flag_uses_the_saved_default_like_go() {
    let settings = Settings {
        default_duration: 37,
        ..Settings::default()
    };
    let args = RunArgs {
        time: Some(0),
        ..RunArgs::default()
    };
    assert_eq!(resolve_test_config(&settings, &args).duration.as_secs(), 37);
}

#[test]
fn zero_default_time_is_rejected_instead_of_becoming_a_one_second_test() {
    let mut settings = Settings::default();
    let args = ConfigArgs {
        default_time: Some(0),
        ..ConfigArgs::default()
    };
    assert!(matches!(
        ttype_core::config::apply(&mut settings, &args),
        Err(ConfigError::Duration)
    ));
}
