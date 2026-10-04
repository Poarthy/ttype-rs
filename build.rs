fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-env-changed=TTYPE_VERSION");
    println!("cargo:rerun-if-env-changed=TTYPE_RELEASE");
    let version = std::env::var("TTYPE_VERSION").unwrap_or_else(|_| {
        std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "dev".to_owned())
    });
    println!("cargo:rustc-env=TTYPE_VERSION={version}");
}
