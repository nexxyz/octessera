use super::{next_event, test_adapter};
use playback_runtime::{DrumHit, RuntimeErrorCode};
use rodio_engine_source::EngineEvent;

#[test]
fn desktop_drum_hit_is_typed_musical_audio_not_midi_and_rejects_invalid_fields() {
    let (mut adapter, mut rx) = test_adapter();
    let hit = DrumHit {
        instrument_slot: 1,
        voice: 7,
        tune_semis: -24,
        velocity: 100,
    };
    adapter.handle_runtime_drum_hit(&hit).unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::DrumHit {
            instrument_slot: 1,
            voice: 7,
            tune_semis: -24,
            velocity: 100,
        }
    ));
    for invalid in [
        DrumHit {
            instrument_slot: 8,
            ..hit.clone()
        },
        DrumHit {
            voice: 8,
            ..hit.clone()
        },
        DrumHit {
            tune_semis: 25,
            ..hit.clone()
        },
        DrumHit { velocity: 0, ..hit },
    ] {
        assert_eq!(
            adapter
                .handle_runtime_drum_hit(&invalid)
                .unwrap_err()
                .facts
                .code,
            RuntimeErrorCode::InvalidPayload
        );
    }
    assert!(rx.try_recv().is_err());
}
