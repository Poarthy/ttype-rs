use std::collections::BTreeMap;

use ttype_core::domain::{GenerateOptions, TextMode};
use ttype_core::text::TextProvider;

#[test]
fn generator_repeats_items_and_is_deterministic_for_a_seed() {
    let provider = TextProvider::new(BTreeMap::from([(
        TextMode::Words,
        vec!["alpha".to_owned(), "beta".to_owned()],
    )]));
    let options = GenerateOptions {
        word_limit: 25,
        seed: 42,
        ..GenerateOptions::default()
    };
    let first = match provider.generate(&options) {
        Ok(value) => value,
        Err(error) => panic!("target should generate: {error}"),
    };
    let second = match provider.generate(&options) {
        Ok(value) => value,
        Err(error) => panic!("target should generate: {error}"),
    };
    assert_eq!(first, second);
    assert_eq!(first.split_whitespace().count(), 25);
}
