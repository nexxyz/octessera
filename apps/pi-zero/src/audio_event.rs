use playback_runtime::{
    DrumHit, MusicalEvent, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeOperation,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEvent;

pub(crate) fn musical_event_to_engine_event(event: &MusicalEvent) -> EngineEvent {
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
        },
        MusicalEvent::NoteOff { channel, note } => EngineEvent::NoteOff {
            instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
            note: (*note).min(127),
        },
        MusicalEvent::Cc {
            channel,
            controller,
            value,
        } => EngineEvent::Cc {
            instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
            controller: (*controller).min(127),
            value: (*value).min(127),
        },
    }
}

pub(crate) fn drum_hit_to_engine_event(hit: &DrumHit) -> Result<EngineEvent, RuntimeAdapterError> {
    if usize::from(hit.instrument_slot) >= INSTRUMENT_SLOT_COUNT
        || hit.voice >= 8
        || !(-24..=24).contains(&hit.tune_semis)
        || !(1..=127).contains(&hit.velocity)
    {
        return Err(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::MusicalEvent,
            Some("invalid Drum hit slot, voice, tune, or velocity".into()),
        )));
    }
    Ok(EngineEvent::DrumHit {
        instrument_slot: hit.instrument_slot,
        voice: hit.voice,
        tune_semis: hit.tune_semis,
        velocity: hit.velocity,
    })
}

#[cfg(test)]
mod tests {
    use super::{drum_hit_to_engine_event, musical_event_to_engine_event};
    use playback_runtime::{DrumHit, MusicalEvent};
    use rodio_engine_source::EngineEvent;

    #[test]
    fn shared_musical_event_mapping_clamps_engine_values() {
        assert!(matches!(
            musical_event_to_engine_event(&MusicalEvent::NoteOn {
                channel: u8::MAX,
                note: u8::MAX,
                velocity: 0,
                duration_ms: None,
            }),
            EngineEvent::NoteOn {
                note: 127,
                velocity: 1,
                duration_ms: 86_400_000,
                ..
            }
        ));
    }

    #[test]
    fn drum_hit_mapping_preserves_fields_and_rejects_invalid_hit() {
        let hit = DrumHit {
            instrument_slot: 2,
            voice: 7,
            tune_semis: -24,
            velocity: 120,
        };
        assert!(matches!(
            drum_hit_to_engine_event(&hit).unwrap(),
            EngineEvent::DrumHit {
                instrument_slot: 2,
                voice: 7,
                tune_semis: -24,
                velocity: 120
            }
        ));
        assert!(matches!(
            drum_hit_to_engine_event(&DrumHit {
                tune_semis: 24,
                ..hit.clone()
            }),
            Ok(EngineEvent::DrumHit {
                instrument_slot: 2,
                voice: 7,
                tune_semis: 24,
                velocity: 120
            })
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
            assert!(drum_hit_to_engine_event(&invalid).is_err());
        }
    }
}
