use super::{next_event, test_adapter};
use playback_runtime::{CellHit, RuntimeErrorCode};
use rodio_engine_source::EngineEvent;

#[test]
fn desktop_cell_hit_is_typed_musical_audio_not_midi_and_rejects_invalid_fields() {
    let (mut adapter, mut rx) = test_adapter();
    let hit = CellHit {
        instrument_slot: 1,
        voice: 7,
        cell: 9,
        pitch_cents: -2450,
        velocity: 100,
    };
    adapter.handle_runtime_cell_hit(&hit).unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::CellHit {
            instrument_slot: 1,
            voice: 7,
            cell: 9,
            pitch_cents: -2450,
            velocity: 100,
        }
    ));
    adapter
        .handle_runtime_cell_hit(&CellHit {
            velocity: 0,
            ..hit.clone()
        })
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::CellHit { velocity: 0, .. }
    ));
    for invalid in [
        CellHit {
            instrument_slot: 8,
            ..hit.clone()
        },
        CellHit {
            voice: 8,
            ..hit.clone()
        },
        CellHit {
            pitch_cents: 4801,
            ..hit.clone()
        },
        CellHit {
            velocity: 128,
            ..hit
        },
    ] {
        assert_eq!(
            adapter
                .handle_runtime_cell_hit(&invalid)
                .unwrap_err()
                .facts
                .code,
            RuntimeErrorCode::InvalidPayload
        );
    }
    assert!(rx.try_recv().is_err());
}
