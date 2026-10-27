use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    CellHit, HostAdapter, HostMessage, MusicalEvent, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimePlatformRequest,
};

/// A note-on or cell hit as it reached the engine, before the session stamps its time.
pub(super) struct HeardEvent {
    pub(super) slot: u8,
    pub(super) note: u8,
    pub(super) voice: Option<u8>,
    pub(super) velocity: u8,
    pub(super) pitch_cents: i16,
}

/// The desktop host adapter, unchanged, plus a record of what it was asked to play.
pub(super) struct LoggingAdapter {
    pub(super) inner: DesktopPlaybackHostAdapter,
    pub(super) heard: Vec<HeardEvent>,
}

impl HostAdapter for LoggingAdapter {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        if let MusicalEvent::NoteOn {
            channel,
            note,
            velocity,
            ..
        } = event
        {
            self.heard.push(HeardEvent {
                slot: *channel,
                note: *note,
                voice: None,
                velocity: *velocity,
                pitch_cents: 0,
            });
        }
        self.inner.handle_musical_event(event)
    }

    fn handle_cell_hit(&mut self, hit: &CellHit) -> Result<(), RuntimeAdapterError> {
        if hit.velocity > 0 {
            self.heard.push(HeardEvent {
                slot: hit.instrument_slot,
                note: hit.cell,
                voice: Some(hit.voice),
                velocity: hit.velocity,
                pitch_cents: hit.pitch_cents,
            });
        }
        self.inner.handle_cell_hit(hit)
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        self.inner.handle_platform_effect(request)
    }

    fn acknowledge_restored_state(&mut self) -> Result<(), RuntimeAdapterError> {
        self.inner.acknowledge_restored_state()
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        self.inner.handle_audio_command(command)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        self.inner.handle_midi_message(bytes)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.inner.silence_internal_audio()
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.inner.panic_external_midi()
    }
}
