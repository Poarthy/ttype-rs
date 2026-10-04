use std::fs;

use ttype_core::langcache::{LanguageCache, display_name};

fn test_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("ttype-{label}-{}", std::process::id()))
}

#[test]
fn cached_language_words_are_loaded_without_network() {
    let data = test_dir("language-cache");
    let languages = data.join("languages");
    match fs::create_dir_all(&languages) {
        Ok(()) => {}
        Err(error) => panic!("create language cache: {error}"),
    }
    match fs::write(
        languages.join("spanish.json"),
        r#"{"words":["uno","dos","tres"]}"#,
    ) {
        Ok(()) => {}
        Err(error) => panic!("write language list: {error}"),
    }
    match fs::write(languages.join("_manifest.json"), "[]") {
        Ok(()) => {}
        Err(error) => panic!("write manifest: {error}"),
    }

    let cache = LanguageCache::new(&data);
    let words = match cache.words("spanish") {
        Ok(words) => words,
        Err(error) => panic!("load cached words: {error}"),
    };
    assert_eq!(words, ["uno", "dos", "tres"]);
    assert_eq!(cache.installed(), ["spanish"]);
    assert_eq!(display_name("code_python"), "code python");

    let _ = fs::remove_dir_all(data);
}

#[test]
fn cached_language_rejects_empty_lists() {
    let data = test_dir("language-empty");
    let languages = data.join("languages");
    match fs::create_dir_all(&languages) {
        Ok(()) => {}
        Err(error) => panic!("create language cache: {error}"),
    }
    match fs::write(languages.join("empty.json"), r#"{"words":[]}"#) {
        Ok(()) => {}
        Err(error) => panic!("write empty language list: {error}"),
    }

    assert!(LanguageCache::new(&data).words("empty").is_err());
    let _ = fs::remove_dir_all(data);
}
