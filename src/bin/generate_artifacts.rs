use std::path::PathBuf;

fn main() {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("contrib/generated"));
    if let Err(error) = ttype_core::completion::generate_artifacts(&directory) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
