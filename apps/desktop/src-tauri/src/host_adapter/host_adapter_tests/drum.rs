use super::{next_event, test_adapter};
use playback_runtime::{RuntimeErrorCode, TileHit};
use rodio_engine_source::EngineEvent;

#[test]
fn desktop_tile_hit_is_typed_musical_audio_not_midi_and_rejects_invalid_fields() {
    let (mut adapter, mut rx) = test_adapter();
    let hit = TileHit {
        instrument_slot: 1,
        voice: 7,
        tile: 9,
        pitch_cents: -2450,
        velocity: 100,
    };
    adapter.handle_runtime_tile_hit(&hit, None).unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::TileHit {
            instrument_slot: 1,
            voice: 7,
            tile: 9,
            pitch_cents: -2450,
            velocity: 100,
            ..
        }
    ));
    adapter
        .handle_runtime_tile_hit(
            &TileHit {
                velocity: 0,
                ..hit.clone()
            },
            None,
        )
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::TileHit { velocity: 0, .. }
    ));
    for invalid in [
        TileHit {
            instrument_slot: 8,
            ..hit.clone()
        },
        TileHit {
            voice: 8,
            ..hit.clone()
        },
        TileHit {
            pitch_cents: 4801,
            ..hit.clone()
        },
        TileHit {
            velocity: 128,
            ..hit
        },
    ] {
        assert_eq!(
            adapter
                .handle_runtime_tile_hit(&invalid, None)
                .unwrap_err()
                .facts
                .code,
            RuntimeErrorCode::InvalidPayload
        );
    }
    assert!(rx.try_recv().is_err());
}
