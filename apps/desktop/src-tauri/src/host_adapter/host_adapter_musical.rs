use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    MusicalEvent, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeOperation,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEvent;

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_musical_event(
        &mut self,
        event: &MusicalEvent,
    ) -> Result<(), RuntimeAdapterError> {
        let event = match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                validate_musical_channel(*channel)?;
                validate_musical_range(*note, 127, "note")?;
                validate_musical_range(*velocity, 127, "velocity")?;
                if *velocity == 0 {
                    return Err(invalid_musical_event("velocity must be at least 1"));
                }
                let duration_ms = match duration_ms {
                    Some(duration) if (10..=86_400_000).contains(duration) => *duration,
                    Some(_) => {
                        return Err(invalid_musical_event(
                            "note duration must be between 10 and 86400000 ms",
                        ))
                    }
                    None => 86_400_000,
                };
                EngineEvent::NoteOn {
                    instrument_slot: *channel,
                    note: *note,
                    velocity: *velocity,
                    duration_ms,
                }
            }
            MusicalEvent::NoteOff { channel, note } => {
                validate_musical_channel(*channel)?;
                validate_musical_range(*note, 127, "note")?;
                EngineEvent::NoteOff {
                    instrument_slot: *channel,
                    note: *note,
                }
            }
            MusicalEvent::Cc {
                channel,
                controller,
                value,
            } => {
                validate_musical_channel(*channel)?;
                validate_musical_range(*controller, 127, "controller")?;
                validate_musical_range(*value, 127, "CC value")?;
                EngineEvent::Cc {
                    instrument_slot: *channel,
                    controller: *controller,
                    value: *value,
                }
            }
        };
        self.send_engine_event(event, RuntimeOperation::MusicalEvent)
    }
}

fn validate_musical_channel(channel: u8) -> Result<(), RuntimeAdapterError> {
    (usize::from(channel) < INSTRUMENT_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_musical_event(format!("channel is out of range: {channel}")))
}

fn validate_musical_range(value: u8, max: u8, name: &str) -> Result<(), RuntimeAdapterError> {
    (value <= max)
        .then_some(())
        .ok_or_else(|| invalid_musical_event(format!("{name} is out of range: {value}")))
}

fn invalid_musical_event(message: impl Into<String>) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::InvalidPayload,
        RuntimeOperation::MusicalEvent,
        Some(message.into()),
    ))
}
