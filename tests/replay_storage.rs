use std::time::Duration;

use ttype_core::replay::{Replay, ReplayEvent, ReplayEventKind};
use ttype_core::storage::{decode_replay, encode_replay};

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
