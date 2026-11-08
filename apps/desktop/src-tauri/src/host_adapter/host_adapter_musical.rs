use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    clamp_musical_event, MusicalEvent, RuntimeAdapterError, RuntimeOperation, MAX_NOTE_DURATION_MS,
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
        };
        self.send_engine_event(event, RuntimeOperation::MusicalEvent)
    }
}
