use std::fs;
use std::io::{self, Read};

use thiserror::Error;

use crate::domain::{TestConfig, TestKind, TextMode};

pub const MAX_CUSTOM_TEXT: usize = 1 << 20;

pub struct CustomInput<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdin_piped: bool,
    pub file: Option<&'a str>,
    // `Some("")` is intentional: `--text ''` is a supplied, invalid source.
    pub text: Option<&'a str>,
}

#[derive(Debug, Error)]
pub enum CustomTextError {
    #[error("give the text one way: piped in, --file or --text")]
    ConflictingSources,
    #[error("read {origin}: {error}")]
    Read { origin: String, error: io::Error },
    #[error("{origin} has no text to type")]
    Empty { origin: String },
    #[error("more than 1 MiB of text")]
    TooLarge,
}

/// Reads the same three mutually-exclusive sources as Go: a pipe, `--file`
/// (where `-` is stdin), or `--text`.
pub fn read(input: CustomInput<'_>) -> Result<String, CustomTextError> {
    let from_stdin = input.file == Some("-") || (input.stdin_piped && input.file.is_none());
    let from_file = input.file.is_some_and(|file| file != "-");
    let source_count =
        usize::from(from_stdin) + usize::from(from_file) + usize::from(input.text.is_some());
    if source_count > 1 || (input.stdin_piped && from_file) {
        return Err(CustomTextError::ConflictingSources);
    }

    let (raw, source) = if let Some(text) = input.text {
        (text.to_owned(), "--text".to_owned())
    } else if from_stdin {
        (read_limited(input.stdin, "stdin")?, "stdin".to_owned())
    } else if let Some(file) = input.file {
        let file_handle = fs::File::open(file).map_err(|error| CustomTextError::Read {
            origin: file.to_owned(),
            error,
        })?;
        (read_limited(file_handle, file)?, file.to_owned())
    } else {
        return Ok(String::new());
    };

    let text = clean(&raw);
    if text.trim().is_empty() {
        return Err(CustomTextError::Empty { origin: source });
    }
    Ok(text)
}

fn read_limited(mut reader: impl Read, source: &str) -> Result<String, CustomTextError> {
    let mut bytes = Vec::with_capacity(4096);
    let mut limited = reader.by_ref().take((MAX_CUSTOM_TEXT + 1) as u64);
    limited
        .read_to_end(&mut bytes)
        .map_err(|error| CustomTextError::Read {
            origin: source.to_owned(),
            error,
        })?;
    if bytes.len() > MAX_CUSTOM_TEXT {
        return Err(CustomTextError::TooLarge);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Removes ANSI control sequences, maps pasted punctuation to keyboard
/// characters, and keeps blank input lines for custom-text line numbers.
pub fn clean(raw: &str) -> String {
    let no_ansi = strip_ansi(raw);
    let converted = no_ansi
        .replace(['“', '”', '„', '«', '»'], "\"")
        .replace(['‘', '’', '‚'], "'")
        .replace(['–', '—', '−'], "-")
        .replace('…', "...")
        .replace('\u{a0}', " ")
        .replace(['\u{200b}', '\u{feff}'], "");
    let mut lines = Vec::new();
    for line in converted.split('\n') {
        let mapped: String = line
            .chars()
            .filter_map(|character| {
                if character.is_whitespace() {
                    Some(' ')
                } else if character.is_control() {
                    None
                } else {
                    Some(character)
                }
            })
            .collect();
        lines.push(mapped.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    lines.join("\n").trim_end_matches('\n').to_owned()
}

// Go's regexp recognizes CSI, OSC (BEL/ST terminated), and a two-byte ESC
// sequence. This scanner intentionally accepts that same useful superset.
fn strip_ansi(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut output = String::with_capacity(raw.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            let remaining = &raw[index..];
            if let Some(character) = remaining.chars().next() {
                output.push(character);
                index += character.len_utf8();
                continue;
            }
            break;
        }
        index += 1;
        match bytes.get(index).copied() {
            Some(b'[') => {
                index += 1;
                while let Some(byte) = bytes.get(index).copied() {
                    index += 1;
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            Some(b']') => {
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        0x07 => {
                            index += 1;
                            break;
                        }
                        0x1b if bytes.get(index + 1) == Some(&b'\\') => {
                            index += 2;
                            break;
                        }
                        _ => index += 1,
                    }
                }
            }
            Some(byte @ 0x40..=0x5f) => {
                let _ = byte;
                index += 1;
            }
            Some(_) | None => {}
        }
    }
    output
}

/// Applies Go's custom-text config rule after flag/default resolution.
pub fn with_custom_config(
    mut config: TestConfig,
    text: &str,
    words_set: bool,
    time_set: bool,
) -> TestConfig {
    config.text_mode = TextMode::Custom;
    config.language.clear();
    config.punctuation = false;
    config.numbers = false;
    let total = text.split_whitespace().count();
    if time_set {
        config.kind = TestKind::Timed;
        config.word_count = 0;
    } else if words_set && config.word_count > 0 {
        config.kind = TestKind::Words;
        config.word_count = config.word_count.min(total);
    } else {
        config.kind = TestKind::Words;
        config.word_count = total;
    }
    config
}
