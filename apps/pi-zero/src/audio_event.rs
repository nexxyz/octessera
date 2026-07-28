use playback_runtime::MusicalEvent;
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

#[cfg(test)]
mod tests {
    use super::musical_event_to_engine_event;
    use playback_runtime::MusicalEvent;
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
}
