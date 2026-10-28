use playback_runtime::{
    CellHit, MusicalEvent, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeOperation,
};
use realtime_engine::synth::{CELL_PITCH_CENTS_LIMIT, INSTRUMENT_SLOT_COUNT};
use rodio_engine_source::EngineEvent;
use std::time::Instant;

pub(crate) fn musical_event_to_engine_event(
    event: &MusicalEvent,
    at: Option<Instant>,
) -> EngineEvent {
    match event {
        MusicalEvent::NoteOn {
            channel,
            note,
            velocity,
            duration_ms,
        } => EngineEvent::NoteOn {
            instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
            note: (*note).min(127),
            velocity: (*velocity).clamp(1, 127),
            duration_ms: duration_ms.unwrap_or(86_400_000).clamp(10, 86_400_000),
            at,
        },
        MusicalEvent::NoteOff { channel, note } => EngineEvent::NoteOff {
            instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
            note: (*note).min(127),
            at,
        },
        MusicalEvent::Cc {
            channel,
            controller,
            value,
        } => EngineEvent::Cc {
            instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
            controller: (*controller).min(127),
            value: (*value).min(127),
            at,
        },
    }
}

pub(crate) fn cell_hit_to_engine_event(
    hit: &CellHit,
    at: Option<Instant>,
) -> Result<EngineEvent, RuntimeAdapterError> {
    if usize::from(hit.instrument_slot) >= INSTRUMENT_SLOT_COUNT
        || hit.voice >= 8
        || !(-CELL_PITCH_CENTS_LIMIT..=CELL_PITCH_CENTS_LIMIT).contains(&hit.pitch_cents)
        || hit.velocity > 127
    {
        return Err(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::MusicalEvent,
            Some("invalid cell hit slot, voice, pitch, or velocity".into()),
        )));
    }
    Ok(EngineEvent::CellHit {
        instrument_slot: hit.instrument_slot,
        voice: hit.voice,
        cell: hit.cell,
        pitch_cents: hit.pitch_cents,
        velocity: hit.velocity,
        at,
    })
}

#[cfg(test)]
mod tests {
    use super::{cell_hit_to_engine_event, musical_event_to_engine_event};
    use playback_runtime::{CellHit, MusicalEvent};
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
    fn cell_hit_mapping_preserves_fields_and_rejects_invalid_hit() {
        let hit = CellHit {
            instrument_slot: 2,
            voice: 7,
            cell: 12,
            pitch_cents: -2450,
            velocity: 120,
        };
        assert!(matches!(
            cell_hit_to_engine_event(&hit, None).unwrap(),
            EngineEvent::CellHit {
                instrument_slot: 2,
                voice: 7,
                cell: 12,
                pitch_cents: -2450,
                velocity: 120,
                ..
            }
        ));
        assert!(matches!(
            cell_hit_to_engine_event(
                &CellHit {
                    velocity: 0,
                    ..hit.clone()
                },
                None
            ),
            Ok(EngineEvent::CellHit { velocity: 0, .. })
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
            assert!(cell_hit_to_engine_event(&invalid, None).is_err());
        }
    }
}
