use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    clamp_musical_event, ClampedMusicalEvent, MusicalEvent, RuntimeAdapterError, RuntimeOperation,
};
use rodio_engine_source::EngineEvent;
use std::time::Instant;

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_musical_event(
        &mut self,
        event: &MusicalEvent,
        at: Option<Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        let event = match clamp_musical_event(event) {
            ClampedMusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => EngineEvent::NoteOn {
                instrument_slot: channel,
                note,
                velocity,
                duration_ms,
                at,
            },
            ClampedMusicalEvent::NoteOff { channel, note } => EngineEvent::NoteOff {
                instrument_slot: channel,
                note,
                at,
            },
            ClampedMusicalEvent::Cc {
                channel,
                controller,
                value,
            } => EngineEvent::Cc {
                instrument_slot: channel,
                controller,
                value,
                at,
            },
        };
        self.send_engine_event(event, RuntimeOperation::MusicalEvent)
    }
}
