use clap::Parser;
use ttype_core::cli::Cli;

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
