use std::fs;
use std::io::Cursor;

use ttype_core::uninstall::{Targets, run};

fn fixture(label: &str) -> (std::path::PathBuf, Targets) {
    let root = std::env::temp_dir().join(format!("ttype-uninstall-{label}-{}", std::process::id()));
    let binary = root.join("ttype");
    let man = root.join("ttype.1");
    let config = root.join("config");
    let data = root.join("data");
    for path in [&config, &data] {
        if let Err(error) = fs::create_dir_all(path) {
            panic!("create fixture directory: {error}");
        }
    }
    for path in [&binary, &man] {
        if let Err(error) = fs::write(path, b"x") {
            panic!("write fixture file: {error}");
        }
    }
    (
        root,
        Targets {
            binary: Some(binary),
            man_pages: vec![man],
            config,
            data,
            managed_note: None,
        },
    )
}

#[test]
fn uninstall_keeps_data_without_purge_and_removes_it_with_purge() {
    let (root, targets) = fixture("keep");
    let mut output = Vec::new();
    let mut input = Cursor::new(Vec::new());
    if let Err(error) = run(targets.clone(), false, true, &mut input, &mut output) {
        panic!("uninstall without purge: {error}");
    }
    assert!(!targets.binary.as_ref().is_some_and(|path| path.exists()));
    assert!(targets.config.exists());
    assert!(targets.data.exists());

    let (purge_root, purge_targets) = fixture("purge");
    let mut output = Vec::new();
    let mut input = Cursor::new(Vec::new());
    if let Err(error) = run(purge_targets.clone(), true, true, &mut input, &mut output) {
        panic!("uninstall with purge: {error}");
    }
    assert!(!purge_targets.config.exists());
    assert!(!purge_targets.data.exists());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(purge_root);
}

#[test]
fn uninstall_stops_when_confirmation_is_declined() {
    let (root, targets) = fixture("decline");
    let mut output = Vec::new();
    let mut input = Cursor::new(b"n\n".to_vec());
    if let Err(error) = run(targets.clone(), true, false, &mut input, &mut output) {
        panic!("declined uninstall: {error}");
    }
    assert!(targets.binary.as_ref().is_some_and(|path| path.exists()));
    assert!(String::from_utf8_lossy(&output).contains("Nothing was removed."));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn package_manager_uninstall_leaves_files_alone_without_purge() {
    let (root, mut targets) = fixture("managed");
    targets.binary = None;
    targets.man_pages.clear();
    targets.managed_note = Some("remove it with your package manager".to_owned());
    let mut output = Vec::new();
    let mut input = Cursor::new(Vec::new());
    if let Err(error) = run(targets.clone(), false, true, &mut input, &mut output) {
        panic!("managed uninstall: {error}");
    }
    assert!(targets.config.exists());
    assert!(targets.data.exists());
    assert!(String::from_utf8_lossy(&output).contains("package manager"));
    let _ = fs::remove_dir_all(root);
}
