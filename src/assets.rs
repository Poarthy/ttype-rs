use std::collections::BTreeMap;

use crate::domain::TextMode;
use crate::text::TextProvider;

const WORDS: &str = include_str!("../assets/words/en.txt");
const SENTENCES: &str = include_str!("../assets/sentences/en.txt");
const SQL: &str = include_str!("../assets/sql/snippets.txt");
const GO: &str = include_str!("../assets/go/snippets.txt");
const BACKEND: &str = include_str!("../assets/backend/terms.txt");
const PYTHON: &str = include_str!("../assets/python/snippets.txt");
const SHELL: &str = include_str!("../assets/shell/snippets.txt");
const REGEX: &str = include_str!("../assets/regex/patterns.txt");
pub const RUST_SNIPPETS: &str = include_str!("../assets/rust/snippets.txt");

fn load_lines(source: &str) -> Vec<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

fn load_regex() -> Vec<String> {
    let mut snippets = Vec::new();
    let mut comment = None;
    for line in REGEX.lines().map(|line| line.trim_end_matches('\r')) {
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            comment = Some(line.to_owned());
        } else if let Some(description) = comment.take() {
            snippets.push(format!("{description} {line}"));
        } else {
            snippets.push(line.to_owned());
        }
    }
    snippets
}

pub fn load_rust() -> Vec<String> {
    RUST_SNIPPETS
        .split("\n\n")
        .map(str::trim)
        .filter(|snippet| !snippet.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn builtin_provider() -> TextProvider {
    TextProvider::new(BTreeMap::from([
        (TextMode::Words, load_lines(WORDS)),
        (TextMode::Sentences, load_lines(SENTENCES)),
        (TextMode::Sql, load_lines(SQL)),
        (TextMode::Go, load_lines(GO)),
        (TextMode::Backend, load_lines(BACKEND)),
        (TextMode::Python, load_lines(PYTHON)),
        (TextMode::Shell, load_lines(SHELL)),
        (TextMode::Regex, load_regex()),
        (TextMode::Rust, load_rust()),
    ]))
}
