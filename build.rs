fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-env-changed=TTYPE_VERSION");
    println!("cargo:rerun-if-env-changed=TTYPE_RELEASE");
    let version = std::env::var("TTYPE_VERSION").unwrap_or_else(|_| {
        std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "dev".to_owned())
    });
    println!("cargo:rustc-env=TTYPE_VERSION={version}");
    if std::env::var("TTYPE_RELEASE").as_deref() == Ok("1") {
        // `option_env!` in the updater distinguishes signed release archives
        // from local/source builds, so carry this build-only marker to rustc.
        println!("cargo:rustc-env=TTYPE_RELEASE=1");
    }
}
