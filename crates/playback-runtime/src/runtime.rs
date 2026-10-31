use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorMetadata,
    RuntimeOperation, RuntimePlatformRequest, RuntimeStatus, RuntimeStoreResult, SyncSource,
};
use platform_core::MusicalEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

mod api;
#[cfg(test)]
mod default_load_tests;
mod dispatch;
mod dispatch_profile;
mod midi;
mod midi_realtime_input;
mod music_first;
mod oled;
mod presentation;
#[cfg(test)]
mod presentation_state_tests;
mod pulse_phase;
mod request_identity;
#[cfg(test)]
mod restore_handoff_tests;
mod sequenced_time;
mod sounding_notes;
mod status;
#[cfg(test)]
mod system_store_aux_tests;
#[cfg(test)]
mod system_store_tests;

pub use music_first::NativeStoreRequest;

pub(super) const PPQN: f64 = 24.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeIngest {
    pub messages: Vec<RunnerMessage>,
    pub follow_ups: Vec<HostMessage>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePresentationMetrics {
    pub audio_load_ratio: f32,
    pub worker_utilization: Option<f32>,
    pub missed_quantum_flash: bool,
}

pub enum RuntimeDispatchInput {
    HostMessage(HostMessage),
    RunnerMessages(Vec<RunnerMessage>),
}

impl RuntimeIngest {
    pub(super) fn merge(&mut self, other: Self) {
        self.messages.extend(other.messages);
        self.follow_ups.extend(other.follow_ups);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    pub bpm: f64,
    pub sync_source: SyncSource,
    pub midi_clock_out_enabled: bool,
    pub midi_out_enabled: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            sync_source: SyncSource::Internal,
            midi_clock_out_enabled: false,
            midi_out_enabled: true,
        }
    }
}

pub trait CoreRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String>;

    fn register_platform_request(&mut self, _request: &RuntimePlatformRequest) {}

    fn send_system_store_result(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>), String>;

    fn send_store_result_handoff(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>, bool), String> {
        let is_system_result = matches!(&message,
            HostMessage::RuntimeResult { result }
                if matches!(result.operation(), RuntimeOperation::StoreLoadSystem | RuntimeOperation::StoreSaveSystem));
        if is_system_result {
            let (messages, accepted) = self.send_system_store_result(message)?;
            return Ok((messages, accepted, false));
        }
        let accepted = match &message {
            HostMessage::RuntimeResult { result }
                if result.operation() == RuntimeOperation::StoreLoadDefault =>
            {
                Some(result.clone())
            }
            _ => None,
        };
        Ok((self.send(message)?, accepted, false))
    }
}

/// `at` is the host-clock instant a sequenced event should sound; `None` plays it now.
pub trait HostAdapter {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn handle_musical_event(
        &mut self,
        event: &MusicalEvent,
        at: Option<Instant>,
    ) -> Result<(), RuntimeAdapterError>;

    fn handle_cell_hit(
        &mut self,
        _hit: &crate::protocol::CellHit,
        _at: Option<Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        Err(RuntimeAdapterError::operation_failed(
            "CellHit host adapter not connected".into(),
        ))
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError>;

    fn acknowledge_restored_state(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError>;

    fn handle_midi_message(
        &mut self,
        bytes: &[u8],
        at: Option<Instant>,
    ) -> Result<(), RuntimeAdapterError>;

    /// Sends pending note-offs now and drops other future MIDI output.
    fn flush_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError>;

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntime {
    config: RuntimeConfig,
    pulse_phase: pulse_phase::PulsePhase,
    now: Duration,
    clock_anchor: Option<(Instant, Duration)>,
    sequenced_at: Option<Instant>,
    sequenced_delay: Duration,
    last_good_status: Option<RuntimeStatus>,
    presented_status: Option<RuntimeStatus>,
    last_good_snapshot: Option<Value>,
    presented_snapshot: Option<Value>,
    latched_errors: Vec<RuntimeErrorMetadata>,
    processed_result_keys: Vec<(bool, RuntimeOperation, Option<String>, Option<u64>)>,
    last_snapshot_revision: u64,
    scheduled_note_offs: VecDeque<ScheduledMidiMessage>,
    scheduled_note_offs_dirty: bool,
    sounding_notes: sounding_notes::SoundingNotes,
    request_next_snapshot: bool,
    next_request_id: u64,
    oled: oled::RuntimeOled,
    dispatch_profile: Option<dispatch_profile::RuntimeDispatchProfile>,
}

impl PlaybackRuntime {
    pub(super) fn reset_pulse_phase(&mut self) {
        self.pulse_phase.reset();
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ScheduledMidiMessage {
    due_at: Duration,
    at: Option<Instant>,
    bytes: Vec<u8>,
}
