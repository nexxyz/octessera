use super::{
    CoreRunner, HostAdapter, PlaybackRuntime, RuntimeConfig, RuntimeDispatchInput, RuntimeIngest,
};
use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeAdapterError, RuntimeErrorDomain, RuntimeErrorMetadata,
    RuntimeOperation, RuntimeRecovery, RuntimeStatus, RuntimeStoreResult, RuntimeTransportState,
    SyncSource,
};
use serde_json::Value;
use std::time::Duration;

impl PlaybackRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            pulse_phase: super::pulse_phase::PulsePhase::default(),
            now: Duration::ZERO,
            clock_anchor: None,
            sequenced_at: None,
            sequenced_delay: crate::AudioOptimization::Latency.sequenced_output_delay(),
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
            next_request_id: 0,
            oled: super::oled::RuntimeOled::default(),
            dispatch_profile: super::dispatch_profile::RuntimeDispatchProfile::from_environment(),
        }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: RuntimeConfig) {
        self.config = config;
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

    pub fn dispatch_runtime_tick<R: CoreRunner, H: HostAdapter>(
        &mut self,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let status_before_tick = self.last_status().cloned();
        let mut output = self.dispatch(
            RuntimeDispatchInput::HostMessage(HostMessage::TransportPulseStep {
                pulses: 0,
                source: self.config.sync_source.clone(),
                at_ppqn_pulse: self.last_status().map(|status| status.current_ppqn_pulse),
                request_snapshot: Some(false),
            }),
            runner,
            host,
        )?;
        if let Some(status_before_tick) = status_before_tick {
            if let Some(index) = output.messages.iter().position(|message| {
                matches!(
                    message,
                    RunnerMessage::RuntimeStatus { status }
                        if status == &status_before_tick
                )
            }) {
                output.messages.remove(index);
            }
        }
        Ok(output)
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
        let output = self.advance_duration_with_output(elapsed, runner, host)?;
        self.requeue_discarded_output(&output);
        Ok(())
    }

    pub fn advance_duration_with_output<R: CoreRunner, H: HostAdapter>(
        &mut self,
        elapsed: Duration,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.now = self.now.saturating_add(elapsed);
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

        let pulses = self.pulse_phase.advance(elapsed, self.config.bpm);
        if pulses == 0 {
            if self.request_next_snapshot {
                self.request_next_snapshot = false;
                return self.dispatch(
                    RuntimeDispatchInput::HostMessage(HostMessage::TransportPulseStep {
                        pulses: 0,
                        source: SyncSource::Internal,
                        at_ppqn_pulse: self
                            .last_good_status
                            .as_ref()
                            .map(|status| status.current_ppqn_pulse),
                        request_snapshot: Some(true),
                    }),
                    runner,
                    host,
                );
            }
            return Ok(RuntimeIngest::default());
        }

        let request_snapshot = if self.request_next_snapshot {
            self.request_next_snapshot = false;
            Some(true)
        } else {
            None
        };
        let step_pulses = super::sequenced_time::pulses_per_step(pulses);
        for step in 0..pulses / step_pulses {
            let last_index = (step + 1) * step_pulses - 1;
            self.sequenced_at = self.pulse_stamp(last_index, pulses);
            let step = self.dispatch(
                RuntimeDispatchInput::HostMessage(HostMessage::TransportPulseStep {
                    pulses: step_pulses,
                    source: SyncSource::Internal,
                    at_ppqn_pulse: self
                        .last_good_status
                        .as_ref()
                        .map(|status| status.current_ppqn_pulse),
                    request_snapshot: request_snapshot.filter(|_| last_index + 1 == pulses),
                }),
                runner,
                host,
            );
            self.sequenced_at = None;
            let step = step?;
            if step.messages.iter().any(is_status) {
                output.messages.retain(|message| !is_status(message));
            }
            output.merge(step);
        }
        Ok(output)
    }

    pub fn handle_midi_realtime_bytes<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        let output = self.handle_midi_realtime_bytes_with_output(bytes, runner, host)?;
        self.requeue_discarded_output(&output);
        Ok(())
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
        let output = self.ingest_runner_messages_with_output(messages, host)?;
        self.requeue_discarded_output(&output);
        Ok(output.follow_ups)
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
            self.best_effort_host_silence(host, &mut output, true);
            output.follow_ups.push(HostMessage::TransportStop);
            self.append_presentations(&mut output);
        }
        Ok(output)
    }

    fn requeue_discarded_output(&mut self, output: &RuntimeIngest) {
        self.oled.requeue_if_current_frame_was_published(output);
    }
}

fn is_status(message: &RunnerMessage) -> bool {
    matches!(message, RunnerMessage::RuntimeStatus { .. })
}

fn same_error_identity(left: &RuntimeErrorMetadata, right: &RuntimeErrorMetadata) -> bool {
    left.operation == right.operation
        && left.request_id == right.request_id
        && left.revision == right.revision
}
