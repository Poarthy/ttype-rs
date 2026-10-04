use std::fs;

use ttype_core::completion::generate_artifacts;

#[test]
fn generated_shells_and_man_include_the_rust_mode_and_command_surface() {
    let directory =
        std::env::temp_dir().join(format!("ttype-rs-completion-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    generate_artifacts(&directory).unwrap_or_else(|error| panic!("artifacts: {error}"));
    for name in [
        "ttype.bash",
        "_ttype",
        "ttype.fish",
        "_ttype.ps1",
        "ttype.1",
    ] {
        let path = directory.join(name);
        assert!(path.exists(), "missing {}", path.display());
    }
    let man = fs::read_to_string(directory.join("ttype.1"))
        .unwrap_or_else(|error| panic!("man: {error}"));
    assert!(man.contains("rust"));
    assert!(man.contains("completion"));
}
