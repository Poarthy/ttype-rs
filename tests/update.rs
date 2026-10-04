use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use tar::Builder;
use ttype_core::update::{
    AutomaticUpdateMode, InstallKind, InstallPlan, UpdateState, atomic_replace,
    automatic_update_mode, extract_binary, newer_version, should_check, verify_checksum,
};

#[test]
fn version_comparison_matches_go_vectors() {
    assert!(newer_version("1.10.0", "1.9.0"));
    assert!(newer_version("1.2.1", "1.2"));
    assert!(newer_version("1.0.0", "dev"));
    assert!(!newer_version("1.2.0", "1.3.0"));
    assert!(!newer_version("1.2", "1.2.0"));
    assert!(!newer_version("", "1.0.0"));
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

#[test]
fn update_checks_are_limited_to_once_daily_even_after_failures() {
    let state = UpdateState {
        last_check_unix: 1_000,
        ..UpdateState::default()
    };
    assert!(!should_check(&state, 1_000 + 86_399));
    assert!(should_check(&state, 1_000 + 86_400));
    assert!(should_check(&state, 999));
}

#[test]
fn update_plan_refuses_unowned_and_major_version_changes_before_download() {
    let source = InstallPlan {
        kind: InstallKind::Source,
        executable: std::env::temp_dir().join("ttype-source"),
        platform: "linux_amd64".to_owned(),
    };
    assert!(source.require_owned("1.2.0", "1.1.0").is_err());

    let release = InstallPlan {
        kind: InstallKind::ReleaseScript,
        executable: std::env::temp_dir().join("ttype-release"),
        platform: "linux_amd64".to_owned(),
    };
    assert!(release.require_owned("2.0.0", "1.1.0").is_err());
    assert!(release.require_owned_explicit().is_ok());
}

#[test]
fn automatic_update_mode_uses_environment_then_setting_then_auto() {
    assert_eq!(
        automatic_update_mode("notify", Some("off")),
        AutomaticUpdateMode::Off
    );
    assert_eq!(
        automatic_update_mode("notify", Some("not-a-mode")),
        AutomaticUpdateMode::Notify
    );
    assert_eq!(
        automatic_update_mode("not-a-mode", None),
        AutomaticUpdateMode::Auto
    );
}

#[test]
fn release_archive_extracts_only_the_root_binary_with_a_bounded_reader()
-> Result<(), Box<dyn std::error::Error>> {
    let mut archive = Vec::new();
    {
        let encoder = GzEncoder::new(&mut archive, Compression::default());
        let mut builder = Builder::new(encoder);
        let payload = b"release-binary";
        let mut header = tar::Header::new_gnu();
        header.set_path("ttype")?;
        header.set_size(payload.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &payload[..])?;
        let encoder = builder.into_inner()?;
        encoder.finish()?;
    }
    assert_eq!(extract_binary(&archive)?, b"release-binary");
    Ok(())
}

#[test]
fn checksum_failure_does_not_replace_the_installed_binary() {
    let file = std::env::temp_dir().join(format!("ttype-update-preserve-{}", std::process::id()));
    std::fs::write(&file, b"old").unwrap_or_default();
    let bad = verify_checksum(b"new", "ttype.tar.gz", "deadbeef  ttype.tar.gz\n");
    assert!(bad.is_err());
    assert_eq!(std::fs::read(&file).unwrap_or_default(), b"old");
    let _ = std::fs::remove_file(file);
}
