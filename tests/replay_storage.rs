use std::time::Duration;

use ttype_core::replay::{Replay, ReplayEvent, ReplayEventKind};
use ttype_core::storage::{decode_replay, encode_replay};
use ttype_core::{config::Paths, storage};

#[test]
fn v1_replay_round_trips_target_unicode_and_millisecond_deltas() {
    let replay = Replay {
        target: "a b".to_owned(),
        events: vec![
            ReplayEvent {
                offset: Duration::from_millis(120),
                kind: ReplayEventKind::Rune,
                character: Some('a'),
            },
            ReplayEvent {
                offset: Duration::from_millis(430),
                kind: ReplayEventKind::Backspace,
                character: None,
            },
            ReplayEvent {
                offset: Duration::from_millis(431),
                kind: ReplayEventKind::Rune,
                character: Some('ß'),
            },
        ],
    };
    let encoded = encode_replay(&replay);
    assert_eq!(&encoded[..5], b"TTRP\x01");
    let decoded = match decode_replay(&encoded) {
        Ok(value) => value,
        Err(error) => panic!("round trip: {error}"),
    };
    assert_eq!(decoded, replay);
}

#[test]
fn replay_files_round_trip_and_reject_path_traversal() {
    let root = std::env::temp_dir().join(format!("ttype-replay-{}", std::process::id()));
    let paths = Paths {
        config: root.join("config"),
        data: root.join("data"),
    };
    let replay = Replay {
        target: "x".to_owned(),
        events: vec![ReplayEvent {
            offset: Duration::ZERO,
            kind: ReplayEventKind::Rune,
            character: Some('x'),
        }],
    };
    assert!(storage::save_replay(&paths, "abc123", &replay).is_ok());
    let restored = match storage::load_replay(&paths, "abc123") {
        Ok(value) => value,
        Err(error) => panic!("load replay: {error}"),
    };
    assert_eq!(restored, replay);
    assert!(storage::save_replay(&paths, "../escape", &replay).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn replay_retention_keeps_only_the_newest_fifty_sidecars() {
    let root = std::env::temp_dir().join(format!("ttype-replay-retain-{}", std::process::id()));
    let paths = Paths {
        config: root.join("config"),
        data: root.join("data"),
    };
    let replay = Replay {
        target: "x".to_owned(),
        events: vec![ReplayEvent {
            offset: Duration::ZERO,
            kind: ReplayEventKind::Rune,
            character: Some('x'),
        }],
    };
    for index in 0..51 {
        let id = format!("recording{index:02}");
        if let Err(error) = storage::save_replay(&paths, &id, &replay) {
            panic!("save replay {id}: {error}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(storage::load_replay(&paths, "recording00").is_err());
    assert!(storage::load_replay(&paths, "recording50").is_ok());
    let files = match std::fs::read_dir(paths.data.join("replays")) {
        Ok(entries) => entries.count(),
        Err(error) => panic!("read replays: {error}"),
    };
    assert_eq!(files, 50);
    let _ = std::fs::remove_dir_all(root);
}
