#[path = "host_adapter_construction.rs"]
mod host_adapter_construction;
#[path = "host_adapter_store.rs"]
mod host_adapter_store;
#[path = "host_adapter_system_info.rs"]
mod host_adapter_system_info;

use crate::audio::AudioService;
use crate::audio_event::musical_event_to_engine_event;
use crate::host_audio_command::send_audio_command;
use crate::midi_host::MidiHost;
use crate::platform_service::{PiPlatformService, PlatformJob, PlatformJobKind};
use crate::setup_portal::start_failure_message;
use crate::usb_config::UsbAudioOut;
use playback_runtime::{
    DeferredDefaultSave, HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent,
    RuntimeAdapterError, RuntimeAudioCommand, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub struct PiPlaybackHostAdapter {
    audio: Option<AudioService>,
    store_dir: PathBuf,
    samples_dir: PathBuf,
    pub(crate) platform_service: PiPlatformService,
    pending_default_save: DeferredDefaultSave,
    midi: MidiHost,
    usb_midi_out_enabled: bool,
    usb_audio_out: UsbAudioOut,
    power_request: Option<PiPowerRequest>,
    latest_recovery_payload: Option<serde_json::Value>,
}
#[derive(Clone, Copy)]
pub enum PiPowerRequest {
    Reboot,
    Shutdown,
}
impl PiPlaybackHostAdapter {
    pub fn new(
        audio: Option<AudioService>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        usb_audio_out: UsbAudioOut,
    ) -> Self {
        let platform_service = PiPlatformService::new(store_dir.clone(), samples_dir.clone());
        Self::with_platform_service(
            audio,
            store_dir,
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            usb_audio_out,
            platform_service,
        )
    }

    pub fn take_power_request(&mut self) -> Option<PiPowerRequest> {
        self.power_request.take()
    }
    pub fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
        let Some(entry) = self.pending_default_save.take_due(Instant::now()) else {
            return Ok(Vec::new());
        };
        let payload = entry.payload;
        let request = entry.request;
        if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
            request.clone(),
            PlatformJobKind::SaveDefault {
                payload: payload.clone(),
                is_auto: Some(true),
            },
        )) {
            self.pending_default_save.retry(
                playback_runtime::DeferredDefaultSaveEntry {
                    payload,
                    due_at: Instant::now(),
                    request: request.clone(),
                },
                retry_default_save_at(),
            );
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
        let Some(audio) = &self.audio else {
            return Ok(());
        };
        audio.send_realtime(musical_event_to_engine_event(event))
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        let effect = &request.effect;
        let result = match effect {
            RuntimePlatformEffect::StoreListPresets => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::ListPresets,
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Preset list queued failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::LoadPreset { name: name.clone() },
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Load {name} queued failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::SavePreset {
                        name: name.clone(),
                        payload: payload.clone(),
                    },
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Save {name} queued failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreDeletePreset { name } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::DeletePreset { name: name.clone() },
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Delete {name} queued failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreLoadDefault => self.load_default_result()?,
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                match self.save_default_result(request, payload, mode.as_deref())? {
                    Some(result) => result,
                    None => return Ok(Vec::new()),
                }
            }
            RuntimePlatformEffect::UsbApplyReboot { payload } => {
                self.pending_default_save.cancel();
                if let Err(message) = self.platform_service.save_default_now(payload) {
                    return Ok(vec![store_error(format!(
                        "USB apply save failed: {message}"
                    ))]);
                }
                self.power_request = Some(PiPowerRequest::Reboot);
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
                if matches!(self.usb_audio_out, UsbAudioOut::Usb | UsbAudioOut::Both) {
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
            RuntimePlatformEffect::StoreSaveBackup { payload } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::SaveBackup {
                        payload: payload.clone(),
                    },
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Save backup queued failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                self.latest_recovery_payload = Some(payload.clone());
                if let Err(message) = self.platform_service.save_recovery_now(payload) {
                    return Ok(vec![store_error(format!(
                        "Save recovery failed: {message}"
                    ))]);
                }
                return Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SaveRecoveryResult { ok: true },
                }]);
            }
            RuntimePlatformEffect::MidiListOutputsRequest => {
                RuntimeStoreResult::MidiListOutputsResult {
                    outputs: self.midi.list_outputs()?,
                }
            }
            RuntimePlatformEffect::MidiListInputsRequest => {
                RuntimeStoreResult::MidiListInputsResult {
                    inputs: self.midi.list_inputs()?,
                }
            }
            RuntimePlatformEffect::SystemInfoRequest => {
                return Ok(host_adapter_system_info::request(
                    &self.platform_service,
                    request,
                ))
            }
            RuntimePlatformEffect::SetupPortalOpen => {
                if let Err(failure) = self.platform_service.start_setup_portal(request) {
                    return Ok(vec![start_failure_message(request, failure)]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                let result = self.midi.select_output(id.clone());
                RuntimeStoreResult::MidiStatus {
                    ok: result.is_ok(),
                    message: result.err(),
                    selected_out_id: self.midi.selected_output_id(),
                    selected_in_id: self.midi.selected_input_id(),
                }
            }
            RuntimePlatformEffect::MidiSelectInput { id } => {
                let result = self.midi.select_input(id.clone());
                RuntimeStoreResult::MidiStatus {
                    ok: result.is_ok(),
                    message: result.err(),
                    selected_out_id: self.midi.selected_output_id(),
                    selected_in_id: self.midi.selected_input_id(),
                }
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
                if let Some(payload) = &self.latest_recovery_payload {
                    if let Err(message) = self.platform_service.save_recovery_now(payload) {
                        return Ok(vec![store_error(format!(
                            "Save recovery failed: {message}"
                        ))]);
                    }
                }
                self.power_request = Some(PiPowerRequest::Reboot);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Shutdown => {
                if let Some(payload) = &self.latest_recovery_payload {
                    if let Err(message) = self.platform_service.save_recovery_now(payload) {
                        return Ok(vec![store_error(format!(
                            "Save recovery failed: {message}"
                        ))]);
                    }
                }
                self.power_request = Some(PiPowerRequest::Shutdown);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::HardwareTest => {
                println!("system.hardwareTest requested (planned guided hardware diagnostic)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::UpdateCheck => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::UpdateCheck,
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Update check queue failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::UpdateApply => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::UpdateApply,
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Update apply queue failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Rollback => {
                if let Err(message) = self
                    .platform_service
                    .enqueue(PlatformJob::new(request.clone(), PlatformJobKind::Rollback))
                {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Rollback queue failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
                    request.clone(),
                    PlatformJobKind::ListSamples {
                        instrument_slot: *instrument_slot,
                        sample_slot: *sample_slot,
                        dir: dir.clone(),
                    },
                )) {
                    return Ok(vec![identified_failure(
                        request,
                        format!("Sample list queued failed: {message}"),
                    )]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
        };
        Ok(vec![HostMessage::RuntimeResult { result }])
    }
    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        send_audio_command(self.audio.clone(), command, &self.samples_dir)
    }
    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
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
