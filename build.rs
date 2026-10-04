use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    let _ = fs::create_dir_all("man");
}
