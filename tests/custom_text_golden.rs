use std::io::Cursor;

use ttype_core::custom_text::{CustomInput, clean, read, with_custom_config};
use ttype_core::domain::{TestConfig, TestKind, TextMode};

#[test]
fn cleans_the_same_paste_artifacts_as_go() {
    let cases = [
        ("hello\r\nworld\n", "hello\nworld"),
        ("a\tb   c\n\n\nd", "a b c\n\n\nd"),
        ("\x1b[31mred\x1b[0m and \x1b[1;32mgreen", "red and green"),
        (
            "\u{201c}quoted\u{201d} \u{2018}single\u{2019} \u{2013} dash \u{2014} em\u{2026}",
            "\"quoted\" 'single' - dash - em...",
        ),
        ("  \u{a0}padded\u{200b}\u{feff}  ", "padded"),
        ("bell\x07 and nul\0", "bell and nul"),
        (
            "\u{0633}\u{0644}\u{0627}\u{0645}\u{200c}\u{062f}\u{0646}\u{06cc}\u{0627}",
            "\u{0633}\u{0644}\u{0627}\u{0645}\u{200c}\u{062f}\u{0646}\u{06cc}\u{0627}",
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(clean(raw), expected, "{raw:?}");
    }
}

#[test]
fn accepts_exactly_one_custom_text_source_and_reports_its_name() {
    let piped = || Cursor::new(b"piped\ttext\n".to_vec());
    assert_eq!(
        read(CustomInput {
            stdin: &mut piped(),
            stdin_piped: true,
            file: None,
            text: None
        })
        .unwrap_or_else(|error| panic!("piped: {error}")),
        "piped text"
    );
    assert_eq!(
        read(CustomInput {
            stdin: &mut Cursor::new(b"dash".to_vec()),
            stdin_piped: true,
            file: Some("-"),
            text: None
        })
        .unwrap_or_else(|error| panic!("dash: {error}")),
        "dash"
    );
    let error = read(CustomInput {
        stdin: &mut Cursor::new(b"x".to_vec()),
        stdin_piped: true,
        file: Some("notes.txt"),
        text: Some("y"),
    })
    .err()
    .unwrap_or_else(|| panic!("conflicting inputs should fail"));
    assert!(error.to_string().contains("one way"));

    let error = read(CustomInput {
        stdin: &mut Cursor::new(b" \n".to_vec()),
        stdin_piped: true,
        file: None,
        text: None,
    })
    .err()
    .unwrap_or_else(|| panic!("blank input should fail"));
    assert!(error.to_string().contains("stdin has no text"));
}

#[test]
fn custom_text_replaces_generation_options_and_clamps_word_count() {
    let cfg = TestConfig {
        kind: TestKind::Timed,
        text_mode: TextMode::Go,
        language: "spanish".to_owned(),
        punctuation: true,
        numbers: true,
        ..TestConfig::default()
    };
    let whole = with_custom_config(cfg.clone(), "one two three", false, false);
    assert_eq!(whole.text_mode, TextMode::Custom);
    assert_eq!(whole.kind, TestKind::Words);
    assert_eq!(whole.word_count, 3);
    assert!(whole.language.is_empty());
    assert!(!whole.punctuation && !whole.numbers);

    let words = with_custom_config(cfg, "one two three", true, false);
    assert_eq!(words.kind, TestKind::Words);
    assert_eq!(words.word_count, 3);
}
