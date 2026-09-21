#[path = "orange_host_adapter_construction.rs"]
mod construction;
#[path = "orange_host_adapter_keyboard.rs"]
mod keyboard;
#[path = "orange_host_adapter_oled.rs"]
mod oled;

use crate::audio::AudioService;
use crate::midi_host::{MidiHost, RuntimeOutputSink};
use crate::oled_frame_cache::OledFrameCache;
use crate::orange_audio::OrangeAudioHost;
use crate::orange_device_apply::OrangeShutdownRequest;
use crate::platform_service::{
    dispatch_midi_effect_messages, dispatch_shared_effect, enqueue_job,
    usb_sd_transfer_output_block_reason, PiPlatformService, PlatformJob, PlatformJobKind,
    QueueFailureStyle,
};
use playback_runtime::{
    DeferredDefaultSave, HostAdapter, HostMessage, MusicalEvent, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

pub(crate) struct OrangeHostAdapter {
    audio: AudioService,
    audio_host: OrangeAudioHost,
    platform_service: PiPlatformService,
    pending_default_save: DeferredDefaultSave,
    pending_default_save_generation: Option<u64>,
    midi: MidiHost,
    oled_frame_cache: OledFrameCache,
    shutdown_request: Option<OrangeShutdownRequest>,
    recovery_save_status: Option<Result<(), String>>,
    keyboard_control: Option<crate::usb_keyboard::KeyboardCaptureControl>,
}

impl OrangeHostAdapter {
    pub(crate) fn handle_transfer_input(&self, message: &HostMessage) -> bool {
        if let HostMessage::DeviceInput { input, .. } = message {
            return self.platform_service.handle_transfer_input(input);
        }
        true
    }

    pub(crate) fn take_transfer_status(&mut self) -> Option<HostMessage> {
        if self
            .pending_default_save_generation
            .is_some_and(|generation| {
                generation != self.platform_service.store_write_generation()
                    || self.platform_service.store_writes_blocked()
            })
        {
            self.pending_default_save.cancel();
            self.pending_default_save_generation = None;
        }
        self.platform_service.take_transfer_status()
    }

    pub(crate) fn audio_service(&self) -> AudioService {
        self.audio.clone()
    }

    pub(crate) fn shutdown_pending(&self) -> bool {
        self.shutdown_request.is_some()
    }

    pub(crate) fn take_shutdown_request(&mut self) -> Option<OrangeShutdownRequest> {
        self.shutdown_request.take()
    }

    pub(crate) fn save_recovery_for_power(&mut self) -> Result<(), String> {
        let recovery = self
            .recovery_save_status
            .take()
            .unwrap_or_else(|| Err("recovery save did not complete".into()));
        let recording = self.audio.stop_recording();
        match (recovery, recording) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(recovery), Ok(())) => Err(recovery),
            (Ok(()), Err(recording)) => Err(format!("recording stop failed: {recording}")),
            (Err(recovery), Err(recording)) => {
                Err(format!("{recovery}; recording stop failed: {recording}"))
            }
        }
    }

    fn recovery_save_ready(&self) -> Result<(), String> {
        self.recovery_save_status
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Err("recovery save did not complete".into()))
    }
    pub(crate) fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = self.platform_service.drain_results(max_results);
        if results.len() < max_results {
            results.extend(
                self.audio
                    .drain_prep_results(max_results.saturating_sub(results.len())),
            );
        }
        results
    }

    pub(crate) fn drain_startup_platform_results(&self, max_results: usize) -> Vec<HostMessage> {
        self.platform_service.drain_results(max_results)
    }

    pub(crate) fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
        if self
            .pending_default_save_generation
            .is_some_and(|generation| {
                generation != self.platform_service.store_write_generation()
                    || self.platform_service.store_writes_blocked()
            })
        {
            self.pending_default_save.cancel();
            self.pending_default_save_generation = None;
            return Ok(Vec::new());
        }
        let Some(entry) = self.pending_default_save.take_due(Instant::now()) else {
            return Ok(Vec::new());
        };
        let generation = self.pending_default_save_generation.take();
        let payload = entry.payload;
        let request = entry.request;
        let job = match generation {
            Some(generation) => PlatformJob::with_store_write_generation(
                request.clone(),
                PlatformJobKind::SaveDefault {
                    payload: payload.clone(),
                    is_auto: Some(true),
                },
                generation,
            ),
            None => PlatformJob::new(
                request.clone(),
                PlatformJobKind::SaveDefault {
                    payload: payload.clone(),
                    is_auto: Some(true),
                },
            ),
        };
        if let Err(message) = self.platform_service.enqueue(job) {
            if self.platform_service.store_writes_blocked()
                || generation.is_some_and(|generation| {
                    generation != self.platform_service.store_write_generation()
                })
            {
                return Ok(Vec::new());
            }
            self.pending_default_save.retry(
                playback_runtime::DeferredDefaultSaveEntry {
                    payload,
                    due_at: Instant::now(),
                    request: request.clone(),
                },
                retry_default_save_at(),
            );
            self.pending_default_save_generation = generation;
            return Ok(vec![failure_message(
                &request,
                format!("Auto-save queue failed: {message}"),
            )]);
        }
        Ok(Vec::new())
    }
    fn midi_status(&self, ok: bool, message: Option<String>) -> RuntimeStoreResult {
        RuntimeStoreResult::MidiStatus {
            ok,
            message,
            selected_out_id: self.midi.selected_output_id(),
            selected_in_id: self.midi.selected_input_id(),
        }
    }
    fn unsupported(&self, request: &RuntimePlatformRequest, message: &str) -> Vec<HostMessage> {
        vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: request.unsupported_facts(message.to_string()),
            },
        }]
    }

    fn request_power(
        &mut self,
        request: &RuntimePlatformRequest,
        shutdown_request: OrangeShutdownRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if let Err(error) = self.recovery_save_ready() {
            return Ok(vec![failure_message(request, error)]);
        }
        let recording_result = self.stop_recording_for_transition(request)?;
        self.shutdown_request = Some(shutdown_request);
        Ok(recording_result
            .into_iter()
            .map(|result| HostMessage::RuntimeResult { result })
            .collect())
    }

    fn stop_recording_for_transition(
        &self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
        self.audio
            .stop_recording_with_outcome()
            .map(|outcome| {
                outcome.map(|outcome| crate::audio_recording::recording_status(outcome.status))
            })
            .map_err(recording_finalization_error)
    }

    fn start_usb_sd_transfer(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if let Some(reason) = usb_sd_transfer_output_block_reason(
            self.audio.usb_output_enabled(),
            self.midi.usb_midi_out_enabled(),
        ) {
            return Ok(vec![failure_message(request, reason.into())]);
        }
        if self.audio.is_recording()? {
            return Ok(vec![failure_message(
                request,
                "USB SD2 transfer blocked while recording is active".into(),
            )]);
        }
        self.silence_internal_audio()?;
        self.panic_external_midi()?;
        Ok(enqueue_job(
            &self.platform_service,
            request,
            PlatformJobKind::UsbSdTransferStart,
            QueueFailureStyle::Orange,
            "USB SD2 transfer start".into(),
        ))
    }
}

impl HostAdapter for OrangeHostAdapter {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.audio_host.handle_musical_event(event)
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if self.shutdown_pending() {
            return match &request.effect {
                RuntimePlatformEffect::StoreSaveDefault { .. }
                | RuntimePlatformEffect::ApplyDeviceConfigReboot { .. } => {
                    Ok(vec![failure_message(
                        request,
                        "Orange shutdown request is already pending".into(),
                    )])
                }
                _ => Ok(Vec::new()),
            };
        }
        if let Some(result) =
            dispatch_shared_effect(&self.platform_service, request, QueueFailureStyle::Orange)
        {
            return Ok(result);
        }
        if let Some(messages) = dispatch_midi_effect_messages(&mut self.midi, &request.effect)? {
            return Ok(messages);
        }
        let result = match &request.effect {
            RuntimePlatformEffect::StoreLoadDefault => {
                self.pending_default_save.cancel();
                self.pending_default_save_generation = None;
                let payload = self
                    .platform_service
                    .load_default_now()
                    .map_err(RuntimeAdapterError::operation_failed)?;
                RuntimeStoreResult::LoadDefaultResult { payload }
            }
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                if let Err(message) =
                    crate::usb_config_validation::validate_pi_audio_outputs_payload(payload)
                {
                    return Ok(vec![failure_message(request, message)]);
                }
                if self.platform_service.store_writes_blocked() {
                    return Ok(vec![failure_message(
                        request,
                        "Save default blocked while restore awaits restored-state acknowledgement"
                            .into(),
                    )]);
                }
                if mode.as_deref() == Some("deferred") {
                    self.pending_default_save.schedule(
                        payload.clone(),
                        deferred_default_save_due_at(),
                        request.clone(),
                    );
                    self.pending_default_save_generation =
                        Some(self.platform_service.store_write_generation());
                    return Ok(Vec::new());
                }
                self.pending_default_save.cancel();
                self.pending_default_save_generation = None;
                return Ok(enqueue_job(
                    &self.platform_service,
                    request,
                    PlatformJobKind::SaveDefault {
                        payload: payload.clone(),
                        is_auto: None,
                    },
                    QueueFailureStyle::Orange,
                    "Save default".into(),
                ));
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                let result = self
                    .platform_service
                    .save_recovery_now(payload)
                    .map_err(|error| format!("Save recovery failed: {error}"));
                self.recovery_save_status = Some(result.clone());
                match result {
                    Ok(()) => RuntimeStoreResult::SaveRecoveryResult { ok: true },
                    Err(error) => {
                        eprintln!("Orange recovery save failed: {error}");
                        return Ok(vec![failure_message(request, error)]);
                    }
                }
            }
            RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } => {
                self.pending_default_save.cancel();
                self.pending_default_save_generation = None;
                let recording_result = self.stop_recording_for_transition(request)?;
                let transaction = self
                    .platform_service
                    .prepare_orange_device_apply(payload)
                    .map_err(RuntimeAdapterError::operation_failed)?;
                self.shutdown_request = Some(OrangeShutdownRequest::ApplyDeviceConfig(transaction));
                return Ok(recording_result
                    .into_iter()
                    .map(|result| HostMessage::RuntimeResult { result })
                    .collect());
            }
            RuntimePlatformEffect::Reboot => {
                return self.request_power(request, OrangeShutdownRequest::Reboot);
            }
            RuntimePlatformEffect::Shutdown => {
                return self.request_power(request, OrangeShutdownRequest::Shutdown);
            }
            RuntimePlatformEffect::MidiPanic => {
                self.silence_internal_audio()?;
                let result = self.midi.panic();
                self.midi_status(result.is_ok(), result.err())
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::RecordingStartAudio { max_minutes } => {
                return crate::audio_recording::recording_start_result(
                    self.audio.start_recording(*max_minutes),
                    request,
                );
            }
            RuntimePlatformEffect::RecordingStartAudioOled { max_minutes } => {
                let seed = self
                    .oled_frame_cache
                    .accepted_frame()
                    .map(|frame| (frame.revision(), frame.pixels().to_vec()));
                return crate::audio_recording::recording_start_result(
                    self.audio
                        .start_recording_audio_oled_with_seed(*max_minutes, seed),
                    request,
                );
            }
            RuntimePlatformEffect::RecordingStop => {
                return Ok(self
                    .stop_recording_for_transition(request)?
                    .into_iter()
                    .map(|result| HostMessage::RuntimeResult { result })
                    .collect());
            }
            RuntimePlatformEffect::UsbSdTransferStart => {
                return self.start_usb_sd_transfer(request);
            }
            RuntimePlatformEffect::UsbSdTransferStop => {
                return Ok(enqueue_job(
                    &self.platform_service,
                    request,
                    PlatformJobKind::UsbSdTransferStop,
                    QueueFailureStyle::Orange,
                    "USB SD2 transfer stop".into(),
                ));
            }
            _ => return Ok(self.unsupported(request, "unsupported in Orange foreground runtime")),
        };
        Ok(vec![HostMessage::RuntimeResult { result }])
    }

    fn acknowledge_restored_state(&mut self) -> Result<(), RuntimeAdapterError> {
        self.platform_service.acknowledge_restored_state();
        Ok(())
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.audio_host.handle_audio_command(command)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.midi
            .send(bytes)
            .map_err(RuntimeAdapterError::operation_failed)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.audio_host.silence_internal_audio()
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.midi
            .panic()
            .map_err(RuntimeAdapterError::operation_failed)
    }
}

impl RuntimeOutputSink for OrangeHostAdapter {
    fn dispatch_output(
        &mut self,
        playback: &mut playback_runtime::PlaybackRuntime,
        runner: &mut playback_runtime::NativeRunner,
        output: playback_runtime::RuntimeIngest,
    ) -> Result<(), String> {
        crate::orange_candidate::process_runtime_output(playback, runner, self, output)
    }
}

fn deferred_default_save_due_at() -> Instant {
    Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS)
}

fn retry_default_save_at() -> Instant {
    Instant::now() + Duration::from_secs(1)
}

fn failure_message(request: &RuntimePlatformRequest, message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}

fn recording_finalization_error(error: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Recording,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::Recording,
        Some(error),
    ))
}

#[cfg(test)]
#[path = "orange_host_adapter_apply_tests.rs"]
mod apply_tests;
#[cfg(test)]
#[path = "orange_host_adapter_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "orange_host_adapter_update_tests.rs"]
mod update_tests;
