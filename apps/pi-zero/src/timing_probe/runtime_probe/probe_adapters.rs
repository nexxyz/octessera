use super::cadence::AdvanceCorrelation;
use super::timing_probe_report::event_key;
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, RunnerMessage,
    RuntimeAudioCommand, RuntimePlatformRequest,
};
use serde::Serialize;
use std::time::Instant;

#[derive(Default)]
pub(super) struct ProbeHost {
    pub(super) now_ms: u64,
    pub(super) event_times_ms: Vec<u64>,
    pub(super) events: Vec<EventRecord>,
    pub(super) audio_commands: u64,
    pub(super) platform_effects: u64,
    pub(super) midi_messages: u64,
}

#[derive(Clone, Debug)]
pub(super) struct EventRecord {
    pub(super) time_ms: u64,
    pub(super) key: String,
}

pub(super) struct ProbeRunner {
    pub(super) inner: NativeRunner,
    pub(super) sends: Vec<SendMetric>,
    pub(super) batches: Vec<usize>,
    pub(super) advance_correlations: Vec<AdvanceCorrelation>,
    pub(super) playing_statuses: u64,
}

#[derive(Clone, Default, Serialize)]
pub(super) struct SendMetric {
    pub(super) pulses: Option<u32>,
    pub(super) duration_us: u128,
}

impl CoreRunner for ProbeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let pulses = match &message {
            HostMessage::TransportPulseStep { pulses, .. } => Some(*pulses),
            _ => None,
        };
        let started = Instant::now();
        let responses = self.inner.send(message)?;
        let duration_us = started.elapsed().as_micros();
        for response in &responses {
            match response {
                RunnerMessage::MusicalEvents { events } | RunnerMessage::MidiEvents { events } => {
                    self.batches.push(events.len());
                }
                RunnerMessage::RuntimeStatus { status }
                    if status.transport == playback_runtime::RuntimeTransportState::Playing
                        && status.error.is_none() =>
                {
                    self.playing_statuses = self.playing_statuses.saturating_add(1);
                }
                _ => {}
            }
        }
        self.sends.push(SendMetric {
            pulses,
            duration_us,
        });
        Ok(responses)
    }

    fn register_platform_request(&mut self, request: &RuntimePlatformRequest) {
        self.inner.register_platform_request(request);
    }

    fn send_system_store_result(
        &mut self,
        message: HostMessage,
    ) -> Result<
        (
            Vec<RunnerMessage>,
            Option<playback_runtime::RuntimeStoreResult>,
        ),
        String,
    > {
        <NativeRunner as CoreRunner>::send_system_store_result(&mut self.inner, message)
    }
}

impl HostAdapter for ProbeHost {
    fn handle_musical_event(
        &mut self,
        event: &MusicalEvent,
        _at: Option<std::time::Instant>,
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.event_times_ms.push(self.now_ms);
        self.events.push(EventRecord {
            time_ms: self.now_ms,
            key: event_key(event),
        });
        Ok(())
    }

    fn handle_cell_hit(
        &mut self,
        hit: &playback_runtime::CellHit,
        _at: Option<std::time::Instant>,
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        if hit.velocity > 0 {
            self.event_times_ms.push(self.now_ms);
            self.events.push(EventRecord {
                time_ms: self.now_ms,
                key: format!("cell:{}:{}:{}", hit.instrument_slot, hit.voice, hit.cell),
            });
        }
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, playback_runtime::RuntimeAdapterError> {
        self.platform_effects += 1;
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.audio_commands += 1;
        Ok(())
    }

    fn handle_midi_message(
        &mut self,
        _bytes: &[u8],
        _at: Option<std::time::Instant>,
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.midi_messages += 1;
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
        Ok(())
    }
}
