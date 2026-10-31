use crate::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, RunnerMessage, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStatus, RuntimeStatusState,
    RuntimeStoreResult, RuntimeTransportState, SyncSource,
};
use serde_json::{json, Value};
use std::time::Instant;

pub(crate) fn canonical_oled_snapshot(title: &str) -> Value {
    json!({
        "display": {
            "off": false,
            "splash": "",
            "bodyLayout": "rows",
            "title": title,
            "titleColor": 65535,
            "lines": ["line"],
            "colors": [65535],
            "barValues": [null],
            "rows": [null],
            "scrollOffset": null,
            "totalRows": null,
            "visibleRows": null,
            "editing": false,
            "toast": ""
        },
        "settings": {
            "displayBrightness": 100,
            "autoSaveFlash": "none",
            "autoSaveFlashSerial": 0,
            "cpuWarnPercent": 85
        },
        "selectedRow": null,
        "eventDotLevel": 0,
        "transportIcon": "stop",
        "transportFlash": "none"
    })
}

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
            HostMessage::DeviceInput { .. }
            | HostMessage::PresentedRuntimeErrorInput { .. }
            | HostMessage::MidiRealtimeContinue => Ok(vec![]),
        }
    }

    fn send_system_store_result(
        &mut self,
        _message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>), String> {
        Ok((Vec::new(), None))
    }
}

#[derive(Default)]
pub(crate) struct FakeHost {
    pub midi_messages: Vec<Vec<u8>>,
    pub musical_events: Vec<MusicalEvent>,
    pub cell_hits: Vec<crate::protocol::CellHit>,
    pub audio_commands: Vec<RuntimeAudioCommand>,
    pub effects: Vec<RuntimePlatformEffect>,
    pub silence_calls: usize,
    pub fail_internal_silence: bool,
    pub fail_midi_message: bool,
    pub fail_recording_stop: bool,
    pub fail_recording_transition: bool,
    pub setup_portal_result: Option<RuntimeStoreResult>,
    pub setup_portal_results_sent: Vec<RuntimeStoreResult>,
    pub clock: Option<Instant>,
    pub musical_event_at: Vec<Option<Instant>>,
    pub midi_message_at: Vec<Option<Instant>>,
    pub midi_flushes: usize,
}

impl HostAdapter for FakeHost {
    fn now(&self) -> Instant {
        self.clock.unwrap_or_else(Instant::now)
    }

    fn handle_musical_event(
        &mut self,
        event: &MusicalEvent,
        at: Option<Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        self.musical_events.push(event.clone());
        self.musical_event_at.push(at);
        Ok(())
    }

    fn handle_cell_hit(
        &mut self,
        hit: &crate::protocol::CellHit,
        _at: Option<std::time::Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        self.cell_hits.push(hit.clone());
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        let effect = &request.effect;
        self.effects.push(effect.clone());
        if self.fail_recording_stop && matches!(effect, RuntimePlatformEffect::RecordingStop) {
            return Err(RuntimeAdapterError::operation_failed(
                "stop finalize failed".into(),
            ));
        }
        if self.fail_recording_transition
            && matches!(
                effect,
                RuntimePlatformEffect::Reboot
                    | RuntimePlatformEffect::Shutdown
                    | RuntimePlatformEffect::ApplyDeviceConfig { .. }
            )
        {
            return Err(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
                RuntimeErrorDomain::Recording,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::Recording,
                Some("stop finalize failed".into()),
            )));
        }
        if matches!(effect, RuntimePlatformEffect::SetupPortalOpen) {
            if let Some(result) = self.setup_portal_result.take() {
                let result = result.with_identity(request.request_id.clone(), request.revision);
                self.setup_portal_results_sent.push(result.clone());
                return Ok(vec![HostMessage::RuntimeResult { result }]);
            }
        }
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

    fn handle_midi_message(
        &mut self,
        bytes: &[u8],
        at: Option<Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        if self.fail_midi_message {
            return Err(RuntimeAdapterError::operation_failed(
                "MIDI message send failed".into(),
            ));
        }
        self.midi_messages.push(bytes.to_vec());
        self.midi_message_at.push(at);
        Ok(())
    }

    fn flush_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.midi_flushes += 1;
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
        self.handle_midi_message(&[0xFC], None)?;
        for channel in 0..16_u8 {
            self.handle_midi_message(&[0xB0 | channel, 120, 0], None)?;
            self.handle_midi_message(&[0xB0 | channel, 123, 0], None)?;
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
