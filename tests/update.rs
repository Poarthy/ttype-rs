use sha2::{Digest, Sha256};
use ttype_core::update::{atomic_replace, newer_version, verify_checksum};

#[test]
fn version_comparison_matches_go_vectors() {
    assert!(newer_version("1.10.0", "1.9.0"));
    assert!(newer_version("1.2.1", "1.2"));
    assert!(newer_version("1.0.0", "dev"));
    assert!(!newer_version("1.2.0", "1.3.0"));
}
#[test]
fn checksum_and_atomic_swap_preserve_expected_content() {
    let payload = b"new";
    let digest = format!("{:x}", Sha256::digest(payload));
    assert!(
        verify_checksum(
            payload,
            "ttype.tar.gz",
            &format!("{digest}  ttype.tar.gz\n")
        )
        .is_ok()
    );
    assert!(verify_checksum(b"old", "ttype.tar.gz", &format!("{digest}  ttype.tar.gz\n")).is_err());
    let file = std::env::temp_dir().join(format!("ttype-update-{}", std::process::id()));
    std::fs::write(&file, b"old").unwrap_or_default();
    assert!(atomic_replace(&file, payload).is_ok());
    assert_eq!(std::fs::read(&file).unwrap_or_default(), payload);
    let _ = std::fs::remove_file(file);
}
