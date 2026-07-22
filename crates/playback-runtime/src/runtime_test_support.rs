use crate::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, RunnerMessage, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStatus,
    RuntimeStatusState, RuntimeStoreResult, RuntimeTransportState, SyncSource,
};
use serde_json::json;

#[derive(Default)]
pub(super) struct FakeRunner {
    pub seen: Vec<HostMessage>,
}

impl CoreRunner for FakeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        self.seen.push(message.clone());
        match message {
            HostMessage::TransportPulseStep { pulses, .. } => Ok(vec![
                RunnerMessage::MusicalEvents {
                    events: vec![MusicalEvent::NoteOn {
                        channel: 0,
                        note: 60,
                        velocity: 100,
                        duration_ms: Some(30),
                    }],
                },
                RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Running,
                        transport: RuntimeTransportState::Playing,
                        current_ppqn_pulse: pulses as u64,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                },
            ]),
            HostMessage::MidiRealtimeStart => Ok(vec![RunnerMessage::RuntimeStatus {
                status: RuntimeStatus {
                    state: RuntimeStatusState::Running,
                    transport: RuntimeTransportState::Playing,
                    current_ppqn_pulse: 0,
                    pending_resync: false,
                    sync_source: SyncSource::External,
                    message: None,
                    error: None,
                },
            }]),
            HostMessage::MidiRealtimeClock { pulses } => Ok(vec![RunnerMessage::RuntimeStatus {
                status: RuntimeStatus {
                    state: RuntimeStatusState::Running,
                    transport: RuntimeTransportState::Playing,
                    current_ppqn_pulse: pulses as u64,
                    pending_resync: false,
                    sync_source: SyncSource::External,
                    message: None,
                    error: None,
                },
            }]),
            HostMessage::MidiRealtimeStop => Ok(vec![RunnerMessage::RuntimeStatus {
                status: RuntimeStatus {
                    state: RuntimeStatusState::Paused,
                    transport: RuntimeTransportState::Stopped,
                    current_ppqn_pulse: 2,
                    pending_resync: false,
                    sync_source: SyncSource::External,
                    message: None,
                    error: None,
                },
            }]),
            HostMessage::RuntimeResult { result } => Ok(vec![
                RunnerMessage::Snapshot {
                    snapshot: json!({ "result": result }),
                },
                RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Idle,
                        transport: RuntimeTransportState::Stopped,
                        current_ppqn_pulse: 0,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                },
            ]),
            HostMessage::TransportStop => Ok(vec![RunnerMessage::RuntimeStatus {
                status: RuntimeStatus {
                    state: RuntimeStatusState::Paused,
                    transport: RuntimeTransportState::Stopped,
                    current_ppqn_pulse: 0,
                    pending_resync: false,
                    sync_source: SyncSource::Internal,
                    message: None,
                    error: None,
                },
            }]),
            HostMessage::DeviceInput { .. } | HostMessage::MidiRealtimeContinue => Ok(vec![]),
        }
    }
}

#[derive(Default)]
pub(super) struct FakeHost {
    pub midi_messages: Vec<Vec<u8>>,
    pub musical_events: Vec<MusicalEvent>,
    pub audio_commands: Vec<RuntimeAudioCommand>,
    pub effects: Vec<RuntimePlatformEffect>,
    pub silence_calls: usize,
    pub fail_internal_silence: bool,
}

impl HostAdapter for FakeHost {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        self.musical_events.push(event.clone());
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        let effect = &request.effect;
        self.effects.push(effect.clone());
        if matches!(effect, RuntimePlatformEffect::StoreListPresets) {
            return Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::ListPresetsResult {
                    names: vec!["Factory".into(), "Live Set".into()],
                },
            }]);
        }
        Ok(vec![])
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio_commands.push(command.clone());
        Ok(())
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        self.midi_messages.push(bytes.to_vec());
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.silence_calls += 1;
        if self.fail_internal_silence {
            Err(RuntimeAdapterError::from("internal audio silence failed"))
        } else {
            Ok(())
        }
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.handle_midi_message(&[0xFC])?;
        for channel in 0..16_u8 {
            self.handle_midi_message(&[0xB0 | channel, 120, 0])?;
            self.handle_midi_message(&[0xB0 | channel, 123, 0])?;
        }
        Ok(())
    }
}

pub(super) fn set_runtime_playing(runtime: &mut crate::PlaybackRuntime, host: &mut FakeHost) {
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::RuntimeStatus {
                status: RuntimeStatus {
                    state: RuntimeStatusState::Running,
                    transport: RuntimeTransportState::Playing,
                    current_ppqn_pulse: 0,
                    pending_resync: false,
                    sync_source: SyncSource::Internal,
                    message: None,
                    error: None,
                },
            }],
            host,
        )
        .unwrap();
    host.midi_messages.clear();
}
