use super::{
    CoreRunner, HostAdapter, PlaybackRuntime, RuntimeDispatchInput, RuntimeIngest,
    MAX_BUFFERED_UI_RED,
};
use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeAudioCommand, RuntimeErrorDomain, RuntimeErrorMetadata,
    RuntimeOperation, RuntimeRecovery,
};

impl PlaybackRuntime {
    pub fn dispatch<R: CoreRunner, H: HostAdapter>(
        &mut self,
        input: RuntimeDispatchInput,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let mut output = RuntimeIngest::default();
        let mut queue = match input {
            RuntimeDispatchInput::HostMessage(message) => {
                std::collections::VecDeque::from([message])
            }
            RuntimeDispatchInput::RunnerMessages(messages) => {
                let ingest = self.ingest_core_messages(messages, runner, host)?;
                output.messages.extend(ingest.messages);
                std::collections::VecDeque::from(ingest.follow_ups)
            }
        };
        while let Some(next) = queue.pop_front() {
            self.observe_host_message(&next, None);
            let responses = match runner.send(next) {
                Ok(responses) => responses,
                Err(message) => {
                    let error = RuntimeErrorMetadata::operation_failed(
                        RuntimeErrorDomain::Runtime,
                        RuntimeOperation::RuntimeDispatch,
                        RuntimeRecovery::StopAndSilence,
                        message.clone(),
                    );
                    self.latch_error(error);
                    output.merge(self.best_effort_stop_and_silence(runner, host));
                    break;
                }
            };
            let ingest = self.ingest_core_messages(responses, runner, host)?;
            output.messages.extend(ingest.messages);
            queue.extend(ingest.follow_ups);
        }
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
        }
        self.append_presentations(&mut output);
        Ok(output)
    }

    pub fn dispatch_host_message<R: CoreRunner, H: HostAdapter>(
        &mut self,
        message: HostMessage,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.dispatch(RuntimeDispatchInput::HostMessage(message), runner, host)
    }

    pub fn dispatch_runner_messages<R: CoreRunner, H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.dispatch(RuntimeDispatchInput::RunnerMessages(messages), runner, host)
    }

    pub(super) fn ingest_core_messages<R: CoreRunner, H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.ingest_core_messages_with_runner(messages, Some(runner), host)
    }

    pub(super) fn ingest_core_messages_without_runner<H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.ingest_core_messages_with_runner(messages, None, host)
    }

    pub(super) fn ingest_core_messages_with_runner<H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        mut runner: Option<&mut dyn CoreRunner>,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let mut output = RuntimeIngest::default();
        for message in messages {
            match message {
                RunnerMessage::Snapshot { snapshot } => {
                    if snapshot.is_object() {
                        self.last_good_snapshot = Some(snapshot);
                        self.refresh_presented_snapshot();
                        self.append_snapshot(&mut output);
                    } else {
                        let error = Self::snapshot_failure();
                        self.latch_error(error.clone());
                        self.apply_recovery(error.recovery, &mut runner, host, &mut output);
                        self.append_presentations(&mut output);
                    }
                }
                RunnerMessage::PlatformEffects { effects } => {
                    for effect in effects {
                        let request = self.next_platform_request(effect);
                        match host.handle_platform_effect(&request) {
                            Ok(messages) => {
                                for follow_up in messages {
                                    let follow_up = self.identify_result(follow_up, &request);
                                    output.follow_ups.push(follow_up);
                                }
                            }
                            Err(error) => {
                                let error = self.adapter_error_metadata(
                                    error,
                                    request.error_domain(),
                                    request.operation(),
                                    RuntimeRecovery::RetainLastGood,
                                    Some(request.request_id.clone()),
                                    request.revision,
                                );
                                let recovery = error.recovery.clone();
                                self.latch_error(error);
                                self.apply_recovery(recovery, &mut runner, host, &mut output);
                                self.append_presentations(&mut output);
                            }
                        }
                    }
                }
                RunnerMessage::MusicalEvents { events } => {
                    if let Err(error) = self.schedule_musical_events(events, host) {
                        let error = self.adapter_error_metadata(
                            error,
                            RuntimeErrorDomain::Audio,
                            RuntimeOperation::MusicalEvent,
                            RuntimeRecovery::RetainLastGood,
                            None,
                            None,
                        );
                        self.latch_error(error.clone());
                        self.apply_recovery(error.recovery, &mut runner, host, &mut output);
                        self.append_presentations(&mut output);
                    }
                }
                RunnerMessage::MidiEvents { events } => {
                    if let Err(error) = self.schedule_midi_events(events, host) {
                        let error = self.adapter_error_metadata(
                            error,
                            RuntimeErrorDomain::Midi,
                            RuntimeOperation::MidiEvent,
                            RuntimeRecovery::RetainLastGood,
                            None,
                            None,
                        );
                        self.latch_error(error.clone());
                        self.apply_recovery(error.recovery, &mut runner, host, &mut output);
                        self.append_presentations(&mut output);
                    }
                }
                RunnerMessage::AudioCommands { commands } => {
                    for command in commands {
                        let command = self.identify_audio_command(command);
                        match host.handle_audio_command(&command) {
                            Ok(()) => {
                                let (request_id, revision) = match &command {
                                    RuntimeAudioCommand::SetAudioConfig {
                                        request_id,
                                        revision,
                                        ..
                                    } => (request_id.as_deref(), Some(*revision)),
                                    _ => (None, None),
                                };
                                self.clear_error_with_identity(
                                    RuntimeOperation::AudioCommand,
                                    request_id,
                                    revision,
                                );
                            }
                            Err(error) => {
                                let (request_id, revision) = match &command {
                                    RuntimeAudioCommand::SetAudioConfig {
                                        request_id,
                                        revision,
                                        ..
                                    } => (request_id.clone(), Some(*revision)),
                                    _ => (None, None),
                                };
                                let error = self.adapter_error_metadata(
                                    error,
                                    RuntimeErrorDomain::Audio,
                                    RuntimeOperation::AudioCommand,
                                    RuntimeRecovery::RetainLastGood,
                                    request_id,
                                    revision,
                                );
                                let recovery = error.recovery.clone();
                                self.latch_error(error.clone());
                                self.apply_recovery(recovery, &mut runner, host, &mut output);
                                self.append_presentations(&mut output);
                            }
                        }
                    }
                }
                RunnerMessage::UiPulse { pulse } => {
                    if self.ui_pulses.len() >= MAX_BUFFERED_UI_RED {
                        self.ui_pulses.pop_front();
                    }
                    self.ui_pulses.push_back(pulse);
                    if let Some(pulse) = self.ui_pulses.back().cloned() {
                        output.messages.push(RunnerMessage::UiPulse { pulse });
                    }
                }
                RunnerMessage::RuntimeStatus { status } => {
                    if let Err(error) = self.apply_runtime_status(status, host) {
                        let error = self.adapter_error_metadata(
                            error,
                            RuntimeErrorDomain::Midi,
                            RuntimeOperation::MidiMessage,
                            RuntimeRecovery::RetainLastGood,
                            None,
                            None,
                        );
                        let recovery = error.recovery.clone();
                        self.latch_error(error);
                        self.apply_recovery(recovery, &mut runner, host, &mut output);
                    }
                    self.append_status(&mut output);
                }
            }
        }
        Ok(output)
    }
}
