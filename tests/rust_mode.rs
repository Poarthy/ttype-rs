use ttype_core::assets::{RUST_SNIPPETS, builtin_provider, load_rust};
use ttype_core::domain::{GenerateOptions, TextMode};

#[test]
fn rust_mode_has_a_safe_and_varied_embedded_corpus() {
    let snippets = load_rust();
    assert!((40..=60).contains(&snippets.len()));
    assert!(!RUST_SNIPPETS.contains("unsafe"));
    assert!(snippets.iter().any(|snippet| snippet.contains("async")));
    assert!(snippets.iter().any(|snippet| snippet.contains("<'a>")));
    assert!(snippets.iter().any(|snippet| snippet.contains("Result")));
    assert!(snippets.iter().any(|snippet| snippet.contains("match")));
}

#[test]
fn rust_mode_is_a_literal_code_mode() {
    assert!(!TextMode::Rust.commits_words_on_space());
    let provider = builtin_provider();
    let target = match provider.generate(&GenerateOptions {
        mode: TextMode::Rust,
        word_limit: 1,
        seed: 42,
        ..GenerateOptions::default()
    }) {
        Ok(value) => value,
        Err(error) => panic!("rust target should load: {error}"),
    };
    assert!(!target.is_empty());
}
