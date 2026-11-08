use playback_runtime::{
    clamp_musical_event, MusicalEvent, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeOperation, TileHit, MAX_NOTE_DURATION_MS,
};
use realtime_engine::synth::{INSTRUMENT_SLOT_COUNT, TILE_PITCH_CENTS_LIMIT};
use rodio_engine_source::EngineEvent;
use std::time::Instant;

pub(crate) fn musical_event_to_engine_event(
    event: &MusicalEvent,
    at: Option<Instant>,
) -> EngineEvent {
    match clamp_musical_event(event) {
        MusicalEvent::NoteOn {
            channel,
            note,
            velocity,
            duration_ms,
        } => EngineEvent::NoteOn {
            instrument_slot: channel,
            note,
            velocity,
            duration_ms: duration_ms.unwrap_or(MAX_NOTE_DURATION_MS),
            at,
        },
        MusicalEvent::NoteOff { channel, note } => EngineEvent::NoteOff {
            instrument_slot: channel,
            note,
            at,
        },
        MusicalEvent::Cc {
            channel,
            controller,
            value,
        } => EngineEvent::Cc {
            instrument_slot: channel,
            controller,
            value,
            at,
        },
    }
}

pub(crate) fn tile_hit_to_engine_event(
    hit: &TileHit,
    at: Option<Instant>,
) -> Result<EngineEvent, RuntimeAdapterError> {
    if usize::from(hit.instrument_slot) >= INSTRUMENT_SLOT_COUNT
        || hit.voice >= 8
        || !(-TILE_PITCH_CENTS_LIMIT..=TILE_PITCH_CENTS_LIMIT).contains(&hit.pitch_cents)
        || hit.velocity > 127
    {
        return Err(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::MusicalEvent,
            Some("invalid tile hit slot, voice, pitch, or velocity".into()),
        )));
    }
    Ok(EngineEvent::TileHit {
        instrument_slot: hit.instrument_slot,
        voice: hit.voice,
        tile: hit.tile,
        pitch_cents: hit.pitch_cents,
        velocity: hit.velocity,
        at,
    })
}

#[cfg(test)]
mod tests {
    use super::{musical_event_to_engine_event, tile_hit_to_engine_event};
    use playback_runtime::{MusicalEvent, TileHit};
    use rodio_engine_source::EngineEvent;

    #[test]
    fn shared_musical_event_mapping_clamps_engine_values() {
        assert!(matches!(
            musical_event_to_engine_event(
                &MusicalEvent::NoteOn {
                    channel: u8::MAX,
                    note: u8::MAX,
                    velocity: 0,
                    duration_ms: None,
                },
                None
            ),
            EngineEvent::NoteOn {
                note: 127,
                velocity: 1,
                duration_ms: 86_400_000,
                ..
            }
        ));
    }

    #[test]
    fn tile_hit_mapping_preserves_fields_and_rejects_invalid_hit() {
        let hit = TileHit {
            instrument_slot: 2,
            voice: 7,
            tile: 12,
            pitch_cents: -2450,
            velocity: 120,
            midi_note: 36,
        };
        assert!(matches!(
            tile_hit_to_engine_event(&hit, None).unwrap(),
            EngineEvent::TileHit {
                instrument_slot: 2,
                voice: 7,
                tile: 12,
                pitch_cents: -2450,
                velocity: 120,
                ..
            }
        ));
        assert!(matches!(
            tile_hit_to_engine_event(
                &TileHit {
                    velocity: 0,
                    ..hit.clone()
                },
                None
            ),
            Ok(EngineEvent::TileHit { velocity: 0, .. })
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
            assert!(tile_hit_to_engine_event(&invalid, None).is_err());
        }
    }
}
