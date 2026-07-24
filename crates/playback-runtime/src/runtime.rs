use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorMetadata,
    RuntimeOperation, RuntimePlatformRequest, RuntimeStatus, RuntimeUiPulse, SyncSource,
};
use platform_core::MusicalEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

#[path = "runtime_api.rs"]
mod api;
#[path = "runtime_dispatch.rs"]
mod dispatch;
#[path = "runtime_midi.rs"]
mod midi;
#[path = "runtime_status.rs"]
mod status;

pub(super) const MAX_BUFFERED_UI_RED: usize = 16;
pub(super) const PPQN: f64 = 24.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeIngest {
    pub messages: Vec<RunnerMessage>,
    pub follow_ups: Vec<HostMessage>,
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
}

pub trait HostAdapter {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError>;

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError>;

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError>;

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError>;

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError>;

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntime {
    config: RuntimeConfig,
    pulse_remainder: f64,
    now_ms: u64,
    last_good_status: Option<RuntimeStatus>,
    presented_status: Option<RuntimeStatus>,
    last_good_snapshot: Option<Value>,
    presented_snapshot: Option<Value>,
    latched_errors: Vec<RuntimeErrorMetadata>,
    processed_result_keys: Vec<(bool, RuntimeOperation, Option<String>, Option<u64>)>,
    last_snapshot_revision: u64,
    scheduled_note_offs: VecDeque<ScheduledMidiMessage>,
    scheduled_note_offs_dirty: bool,
    request_next_snapshot: bool,
    ui_pulses: VecDeque<RuntimeUiPulse>,
    next_request_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct ScheduledMidiMessage {
    due_at_ms: u64,
    bytes: Vec<u8>,
}
