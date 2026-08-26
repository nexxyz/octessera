#[path = "host_adapter_construction.rs"]
mod host_adapter_construction;
#[path = "host_adapter_oled.rs"]
mod host_adapter_oled;
#[path = "host_adapter_store.rs"]
mod host_adapter_store;

use crate::audio::AudioService;
use crate::audio_event::musical_event_to_engine_event;
use crate::host_audio_command::send_audio_command;
use crate::midi_host::{MidiHost, RuntimeOutputSink};
use crate::oled_frame_cache::OledFrameCache;
use crate::platform_service::{
    dispatch_midi_effect, dispatch_shared_effect, PiPlatformService, PlatformJob, PlatformJobKind,
    QueueFailureStyle,
};
use playback_runtime::{
    AudioOutputSet, DeferredDefaultSave, HostAdapter, HostMessage,
    MusicalEvent as RuntimeMusicalEvent, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub struct PiPlaybackHostAdapter {
    audio: Option<AudioService>,
    samples_dir: PathBuf,
    pub(crate) platform_service: PiPlatformService,
    pending_default_save: DeferredDefaultSave,
    pending_default_save_generation: Option<u64>,
    midi: MidiHost,
    usb_midi_out_enabled: bool,
    audio_outputs: AudioOutputSet,
    power_request: Option<PiPowerRequest>,
    recovery_save_status: Option<Result<(), String>>,
    pub(crate) oled_frame_cache: OledFrameCache,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PiPowerRequest {
    Reboot,
    Shutdown,
    ApplyDeviceConfigReboot,
}
impl PiPlaybackHostAdapter {
    pub(crate) fn handle_transfer_input(&self, message: &playback_runtime::HostMessage) -> bool {
        if let playback_runtime::HostMessage::DeviceInput { input, .. } = message {
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

    pub fn new<T: Into<AudioOutputSet>>(
        audio: Option<AudioService>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        audio_outputs: T,
    ) -> Self {
        let platform_service = PiPlatformService::new(store_dir.clone(), samples_dir.clone());
        Self::with_platform_service(
            audio,
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            audio_outputs.into(),
            platform_service,
        )
    }

    pub fn take_power_request(&mut self) -> Option<PiPowerRequest> {
        self.power_request.take()
    }

    pub(crate) fn shutdown_pending(&self) -> bool {
        self.power_request.is_some()
    }

    pub(crate) fn save_recovery_for_power(&mut self) -> Result<(), String> {
        self.recovery_save_status
            .take()
            .unwrap_or_else(|| Err("recovery save did not complete".into()))
    }

    fn recovery_save_ready(&self) -> Result<(), String> {
        self.recovery_save_status
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Err("recovery save did not complete".into()))
    }

    pub(crate) fn audio_service(&self) -> Option<AudioService> {
        self.audio.clone()
    }

    pub fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
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
            return Ok(vec![identified_failure(
                &request,
                format!("Auto-save queue failed: {message}"),
            )]);
        }
        Ok(Vec::new())
    }
    pub fn drain_platform_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = self.platform_service.drain_results(max_results);
        if results.len() < max_results {
            if let Some(audio) = &self.audio {
                results.extend(audio.drain_prep_results(max_results - results.len()));
            }
        }
        results
    }
}

impl HostAdapter for PiPlaybackHostAdapter {
    fn handle_musical_event(
        &mut self,
        event: &RuntimeMusicalEvent,
    ) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        let Some(audio) = &self.audio else {
            return Ok(());
        };
        audio.send_realtime(musical_event_to_engine_event(event))
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if self.shutdown_pending() {
            return match &request.effect {
                RuntimePlatformEffect::Reboot | RuntimePlatformEffect::Shutdown => {
                    Ok(vec![identified_failure(
                        request,
                        "ordinary power request is already pending".into(),
                    )])
                }
                _ => Ok(Vec::new()),
            };
        }
        if let Some(result) =
            dispatch_shared_effect(&self.platform_service, request, QueueFailureStyle::Pi)
        {
            return Ok(result);
        }
        if let Some(result) = dispatch_midi_effect(&mut self.midi, &request.effect)? {
            return Ok(vec![HostMessage::RuntimeResult { result }]);
        }
        let effect = &request.effect;
        let result = match effect {
            RuntimePlatformEffect::StoreLoadDefault => self.load_default_result()?,
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                match self.save_default_result(request, payload, mode.as_deref())? {
                    Some(result) => result,
                    None => return Ok(Vec::new()),
                }
            }
            RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } => {
                self.pending_default_save.cancel();
                self.pending_default_save_generation = None;
                if let Err(message) = self.platform_service.save_default_now(payload) {
                    return Ok(vec![store_error(format!(
                        "device/audio apply save failed: {message}"
                    ))]);
                }
                self.power_request = Some(PiPowerRequest::ApplyDeviceConfigReboot);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::RecordingStartAudio { max_minutes } => {
                if let Some(audio) = &self.audio {
                    audio.start_recording(*max_minutes)?;
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::RecordingStop => {
                if let Some(audio) = &self.audio {
                    audio.stop_recording()?;
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::UsbSdTransferStart => {
                if self.audio_outputs.usb() {
                    return Ok(vec![store_error(
                        "USB SD2 transfer blocked while USB audio out is active".into(),
                    )]);
                }
                if self.usb_midi_out_enabled {
                    return Ok(vec![store_error(
                        "USB SD2 transfer blocked while USB MIDI out is enabled".into(),
                    )]);
                }
                if self
                    .audio
                    .as_ref()
                    .map(AudioService::is_recording)
                    .transpose()?
                    .unwrap_or(false)
                {
                    return Ok(vec![store_error(
                        "USB SD2 transfer blocked while recording is active".into(),
                    )]);
                }
                self.silence_internal_audio()?;
                self.panic_external_midi()?;
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::UsbSdTransferStart,
                )) {
                    return Ok(vec![store_error(format!(
                        "USB SD2 transfer start queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::UsbSdTransferStop => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::UsbSdTransferStop,
                )) {
                    return Ok(vec![store_error(format!(
                        "USB SD2 transfer stop queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                let result = self
                    .platform_service
                    .save_recovery_now(payload)
                    .map_err(|message| format!("Save recovery failed: {message}"));
                self.recovery_save_status = Some(result.clone());
                return match result {
                    Ok(()) => Ok(vec![HostMessage::RuntimeResult {
                        result: RuntimeStoreResult::SaveRecoveryResult { ok: true },
                    }]),
                    Err(error) => {
                        eprintln!("pi recovery save failed: {error}");
                        Ok(vec![store_error(error)])
                    }
                };
            }
            RuntimePlatformEffect::MidiPanic => {
                let audio_error = self.silence_internal_audio().err();
                let midi_error = self.panic_external_midi().err();
                if let Some(error) = audio_error.or(midi_error) {
                    return Err(error);
                }
                RuntimeStoreResult::MidiStatus {
                    ok: true,
                    message: Some("Panic sent".into()),
                    selected_out_id: self.midi.selected_output_id(),
                    selected_in_id: self.midi.selected_input_id(),
                }
            }
            RuntimePlatformEffect::Reboot => {
                if let Err(error) = self.recovery_save_ready() {
                    return Ok(vec![identified_failure(request, error)]);
                }
                self.power_request = Some(PiPowerRequest::Reboot);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Shutdown => {
                if let Err(error) = self.recovery_save_ready() {
                    return Ok(vec![identified_failure(request, error)]);
                }
                self.power_request = Some(PiPowerRequest::Shutdown);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::HardwareTest => {
                println!("system.hardwareTest requested (planned guided hardware diagnostic)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
            _ => unreachable!("shared platform effect was not dispatched"),
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
        send_audio_command(self.audio.clone(), command, &self.samples_dir)
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
        if let Some(audio) = &self.audio {
            audio.send_realtime(EngineEvent::AllNotesOff)?;
        }
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.midi
            .panic()
            .map_err(RuntimeAdapterError::operation_failed)
    }
}

impl RuntimeOutputSink for PiPlaybackHostAdapter {
    fn dispatch_output(
        &mut self,
        playback: &mut playback_runtime::PlaybackRuntime,
        runner: &mut playback_runtime::NativeRunner,
        output: playback_runtime::RuntimeIngest,
    ) -> Result<(), String> {
        crate::runtime_loop::process_runtime_output(playback, runner, self, output)
    }
}

fn retry_default_save_at() -> Instant {
    Instant::now() + std::time::Duration::from_millis(1_000)
}

fn store_error(message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::StoreError { message },
    }
}

fn identified_failure(request: &RuntimePlatformRequest, message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}

#[cfg(test)]
#[path = "host_adapter_deferred_default_save_tests.rs"]
mod deferred_default_save_tests;
#[cfg(test)]
#[path = "host_adapter_tests.rs"]
mod tests;
