use super::{
    CoreRunner, HostAdapter, PlaybackRuntime, RuntimeConfig, RuntimeDispatchInput, RuntimeIngest,
    PPQN,
};
use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorDomain,
    RuntimeErrorMetadata, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeRecovery, RuntimeStatus, RuntimeStoreResult, RuntimeTransportState, SyncSource,
};
use serde_json::Value;
use std::time::Duration;

impl PlaybackRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            pulse_remainder: 0.0,
            now_ms: 0,
            last_good_status: None,
            presented_status: None,
            last_good_snapshot: None,
            presented_snapshot: None,
            latched_errors: Vec::new(),
            processed_result_keys: Vec::new(),
            last_snapshot_revision: 0,
            scheduled_note_offs: std::collections::VecDeque::new(),
            scheduled_note_offs_dirty: false,
            request_next_snapshot: false,
            ui_pulses: std::collections::VecDeque::new(),
            next_request_id: 0,
        }
    }

    pub fn drain_ui_pulses(&mut self) -> Vec<crate::RuntimeUiPulse> {
        self.ui_pulses.drain(..).collect()
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: RuntimeConfig) {
        self.config = config;
    }

    pub(super) fn next_platform_request(
        &mut self,
        effect: RuntimePlatformEffect,
    ) -> RuntimePlatformRequest {
        self.next_request_id = self.next_request_id.saturating_add(1);
        let revision = match &effect {
            RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::SetAudioConfig { revision, .. },
            } => Some(*revision),
            RuntimePlatformEffect::StoreSavePreset { payload, .. }
            | RuntimePlatformEffect::StoreSaveDefault { payload, .. }
            | RuntimePlatformEffect::StoreSaveBackup { payload }
            | RuntimePlatformEffect::StoreSaveRecovery { payload }
            | RuntimePlatformEffect::UsbApplyReboot { payload } => {
                payload.get("revision").and_then(Value::as_u64)
            }
            _ => None,
        };
        RuntimePlatformRequest::new(
            effect,
            format!("platform-{}", self.next_request_id),
            revision,
        )
    }

    pub(super) fn identify_audio_command(
        &mut self,
        command: RuntimeAudioCommand,
    ) -> RuntimeAudioCommand {
        let RuntimeAudioCommand::SetAudioConfig {
            revision,
            request_id: None,
            config,
        } = command
        else {
            return command;
        };
        self.next_request_id = self.next_request_id.saturating_add(1);
        RuntimeAudioCommand::SetAudioConfig {
            revision,
            request_id: Some(format!("audio-{}", self.next_request_id)),
            config,
        }
    }

    pub(super) fn identify_result(
        &self,
        message: HostMessage,
        request: &RuntimePlatformRequest,
    ) -> HostMessage {
        match message {
            HostMessage::RuntimeResult { result } => HostMessage::RuntimeResult {
                result: match result {
                    RuntimeStoreResult::StoreError { message } => {
                        RuntimeStoreResult::RuntimeFailure {
                            error: request.failure_facts(message),
                        }
                    }
                    result => result,
                }
                .with_identity(request.request_id.clone(), request.revision),
            },
            other => other,
        }
    }

    pub(super) fn adapter_error_metadata(
        &self,
        error: RuntimeAdapterError,
        domain: RuntimeErrorDomain,
        operation: RuntimeOperation,
        recovery: RuntimeRecovery,
        request_id: Option<String>,
        revision: Option<u64>,
    ) -> RuntimeErrorMetadata {
        error.into_metadata(domain, operation, recovery, request_id, revision)
    }

    pub fn request_next_snapshot(&mut self) {
        self.request_next_snapshot = true;
    }

    pub fn has_scheduled_midi(&self) -> bool {
        !self.scheduled_note_offs.is_empty()
    }

    pub fn last_snapshot(&self) -> Option<&Value> {
        self.presented_snapshot.as_ref()
    }

    pub fn last_good_snapshot(&self) -> Option<&Value> {
        self.last_good_snapshot.as_ref()
    }

    pub fn last_snapshot_revision(&self) -> u64 {
        self.last_snapshot_revision
    }

    pub fn last_status(&self) -> Option<&RuntimeStatus> {
        self.presented_status.as_ref()
    }

    pub fn last_good_status(&self) -> Option<&RuntimeStatus> {
        self.last_good_status.as_ref()
    }

    pub fn latched_errors(&self) -> &[RuntimeErrorMetadata] {
        &self.latched_errors
    }

    pub fn latch_error(&mut self, error: RuntimeErrorMetadata) {
        self.latched_errors
            .retain(|current| !same_error_identity(current, &error));
        self.latched_errors.push(error);
        self.refresh_presentations();
    }

    pub fn clear_error(&mut self, operation: RuntimeOperation) {
        self.clear_error_with_identity(operation, None, None);
    }

    pub fn clear_error_with_identity(
        &mut self,
        operation: RuntimeOperation,
        request_id: Option<&str>,
        revision: Option<u64>,
    ) {
        let previous_len = self.latched_errors.len();
        self.latched_errors.retain(|error| {
            error.operation != operation
                || error.request_id.as_deref() != request_id
                || error.revision != revision
        });
        if self.latched_errors.len() != previous_len {
            self.refresh_presentations();
        }
    }

    pub fn ingest_runtime_result(&mut self, result: &RuntimeStoreResult) {
        self.apply_runtime_result(result, None);
    }

    pub fn advance<R: CoreRunner, H: HostAdapter>(
        &mut self,
        elapsed_ms: u64,
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        self.advance_duration(Duration::from_millis(elapsed_ms), runner, host)
    }

    pub fn advance_duration<R: CoreRunner, H: HostAdapter>(
        &mut self,
        elapsed: Duration,
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        self.advance_duration_with_output(elapsed, runner, host)
            .map(|_| ())
    }

    pub fn advance_duration_with_output<R: CoreRunner, H: HostAdapter>(
        &mut self,
        elapsed: Duration,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let elapsed_ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
        let mut output = RuntimeIngest::default();
        if let Err(error) = self.flush_scheduled_midi(host) {
            let metadata = self.adapter_error_metadata(
                error,
                RuntimeErrorDomain::Midi,
                RuntimeOperation::MidiMessage,
                RuntimeRecovery::StopAndSilence,
                None,
                None,
            );
            self.latch_error(metadata);
            output.merge(self.best_effort_stop_and_silence(runner, host));
            self.append_presentations(&mut output);
            return Ok(output);
        }
        if self.config.sync_source == SyncSource::External {
            return Ok(RuntimeIngest::default());
        }
        if self
            .last_good_status
            .as_ref()
            .is_none_or(|status| status.transport != RuntimeTransportState::Playing)
        {
            return Ok(RuntimeIngest::default());
        }

        let pulses_per_second = (self.config.bpm * PPQN) / 60.0;
        self.pulse_remainder += pulses_per_second * elapsed.as_secs_f64();
        let pulses = self.pulse_remainder.floor() as u32;
        self.pulse_remainder -= pulses as f64;
        if pulses == 0 {
            return Ok(RuntimeIngest::default());
        }

        let request_snapshot = if self.request_next_snapshot {
            self.request_next_snapshot = false;
            Some(true)
        } else {
            None
        };
        self.dispatch(
            RuntimeDispatchInput::HostMessage(HostMessage::TransportPulseStep {
                pulses,
                source: SyncSource::Internal,
                at_ppqn_pulse: self
                    .last_good_status
                    .as_ref()
                    .map(|status| status.current_ppqn_pulse),
                request_snapshot,
            }),
            runner,
            host,
        )
    }

    pub fn handle_midi_realtime_bytes<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        self.handle_midi_realtime_bytes_with_output(bytes, runner, host)
            .map(|_| ())
    }

    pub fn handle_midi_realtime_bytes_with_output<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let mut output = RuntimeIngest::default();
        let mut clock_pulses = 0_u32;
        for byte in bytes {
            match *byte {
                0xF8 => clock_pulses = clock_pulses.saturating_add(1),
                0xFA => {
                    if clock_pulses > 0 {
                        output.merge(self.dispatch(
                            RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                                pulses: clock_pulses,
                            }),
                            runner,
                            host,
                        )?);
                        clock_pulses = 0;
                    }
                    output.merge(self.dispatch(
                        RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeStart),
                        runner,
                        host,
                    )?);
                }
                0xFB => {
                    if clock_pulses > 0 {
                        output.merge(self.dispatch(
                            RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                                pulses: clock_pulses,
                            }),
                            runner,
                            host,
                        )?);
                        clock_pulses = 0;
                    }
                    output.merge(self.dispatch(
                        RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeContinue),
                        runner,
                        host,
                    )?);
                }
                0xFC => {
                    if clock_pulses > 0 {
                        output.merge(self.dispatch(
                            RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                                pulses: clock_pulses,
                            }),
                            runner,
                            host,
                        )?);
                        clock_pulses = 0;
                    }
                    output.merge(self.dispatch(
                        RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeStop),
                        runner,
                        host,
                    )?);
                }
                _ => {}
            }
        }

        if clock_pulses > 0 {
            output.merge(self.dispatch(
                RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                    pulses: clock_pulses,
                }),
                runner,
                host,
            )?);
        }
        Ok(output)
    }

    pub fn panic<H: HostAdapter>(&mut self, host: &mut H) -> Result<(), String> {
        self.scheduled_note_offs.clear();
        self.scheduled_note_offs_dirty = false;
        host.panic_external_midi()
            .map_err(|error| error.to_string())
    }

    pub fn ingest_runner_messages<H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        host: &mut H,
    ) -> Result<Vec<HostMessage>, String> {
        Ok(self
            .ingest_runner_messages_with_output(messages, host)?
            .follow_ups)
    }

    pub fn ingest_runner_messages_with_output<H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let mut output = self.ingest_core_messages_without_runner(messages, host)?;
        if let Err(error) = self.flush_scheduled_midi(host) {
            let metadata = self.adapter_error_metadata(
                error,
                RuntimeErrorDomain::Midi,
                RuntimeOperation::MidiMessage,
                RuntimeRecovery::StopAndSilence,
                None,
                None,
            );
            self.latch_error(metadata);
            self.best_effort_host_silence(host, &mut output);
            output.follow_ups.push(HostMessage::TransportStop);
            self.append_presentations(&mut output);
        }
        Ok(output)
    }
}

fn same_error_identity(left: &RuntimeErrorMetadata, right: &RuntimeErrorMetadata) -> bool {
    left.operation == right.operation
        && left.request_id == right.request_id
        && left.revision == right.revision
}
