#[path = "host_adapter_construction.rs"]
mod host_adapter_construction;
#[path = "host_adapter_recording.rs"]
mod host_adapter_recording;
#[path = "host_adapter_store.rs"]
mod host_adapter_store;

use crate::audio::AudioService;
use crate::audio_event::{drum_hit_to_engine_event, musical_event_to_engine_event};
use crate::host_audio_command::send_audio_command;
use crate::midi_host::RuntimeOutputSink;
use crate::pi_host_core::{
    failure_message, power_save_result, shutdown_pending_messages, unsupported_messages, PiHostCore,
};
use crate::platform_service::{
    dispatch_midi_effect_messages, dispatch_shared_effect, enqueue_job,
    usb_sd_transfer_output_block_reason, PiPlatformService, PlatformJobKind,
};
use playback_runtime::{
    AudioOutputSet, DrumHit, HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent,
    RuntimeAdapterError, RuntimeAudioCommand, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult, UsbDataRole,
};
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::Arc;

pub struct PiPlaybackHostAdapter {
    audio: Option<AudioService>,
    samples_dir: PathBuf,
    pub(crate) core: PiHostCore,
    usb_midi_out_enabled: bool,
    audio_outputs: AudioOutputSet,
    usb_data_role: UsbDataRole,
    power_request: Option<PiPowerRequest>,
    pub(super) timing_evidence: Option<crate::timing_input::TimingStudyEvidence>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PiPowerRequest {
    Reboot,
    Shutdown,
    ApplyDeviceConfigReboot,
}

impl PiPlaybackHostAdapter {
    pub(crate) fn handle_runtime_drum_hit(
        &mut self,
        hit: &DrumHit,
    ) -> Result<(), RuntimeAdapterError> {
        let event = drum_hit_to_engine_event(hit)?;
        if self.shutdown_pending() {
            return Ok(());
        }
        if let Some(audio) = &self.audio {
            audio.send_realtime(event)
        } else {
            Ok(())
        }
    }

    pub(crate) fn handle_transfer_input(&self, message: &HostMessage) -> bool {
        self.core.handle_transfer_input(message)
    }

    pub(crate) fn take_transfer_status(&mut self) -> Option<HostMessage> {
        self.core.take_transfer_status()
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
        let recovery = self.core.take_recovery_save_status();
        let recording = self
            .audio
            .as_ref()
            .map_or(Ok(()), AudioService::stop_recording);
        power_save_result(recovery, recording)
    }

    pub(crate) fn poll_recording_status(&self) -> Option<RuntimeStoreResult> {
        self.audio
            .as_ref()
            .and_then(AudioService::poll_recording_status)
    }

    pub(crate) fn audio_service(&self) -> Option<AudioService> {
        self.audio.clone()
    }

    pub fn drain_platform_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = self.core.platform_service.drain_results(max_results);
        if results.len() < max_results {
            if let Some(audio) = &self.audio {
                results.extend(audio.drain_prep_results(max_results - results.len()));
            }
        }
        results
    }

    fn request_power(
        &mut self,
        request: &RuntimePlatformRequest,
        power_request: PiPowerRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if let Err(error) = self.core.recovery_save_ready() {
            return Ok(vec![failure_message(request, error)]);
        }
        let recording_result = self.stop_recording_for_transition(request)?;
        self.power_request = Some(power_request);
        Ok(recording_result
            .into_iter()
            .map(|result| HostMessage::RuntimeResult { result })
            .collect())
    }

    fn start_usb_sd_transfer(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if self.usb_data_role == UsbDataRole::Host {
            return Ok(vec![crate::rpi_device_apply::unavailable(request)]);
        }
        if let Some(reason) =
            usb_sd_transfer_output_block_reason(self.audio_outputs.usb(), self.usb_midi_out_enabled)
        {
            return Ok(vec![failure_message(request, reason.into())]);
        }
        if self
            .audio
            .as_ref()
            .map(AudioService::is_recording)
            .transpose()?
            .unwrap_or(false)
        {
            return Ok(vec![failure_message(
                request,
                "USB SD2 transfer blocked while recording is active".into(),
            )]);
        }
        self.silence_internal_audio()?;
        self.panic_external_midi()?;
        Ok(enqueue_job(
            &self.core.platform_service,
            request,
            PlatformJobKind::UsbSdTransferStart,
            "USB SD2 transfer start".into(),
        ))
    }
}

impl crate::timing_input::TimingHost for PiPlaybackHostAdapter {
    const STUDY_BOARD: &'static str = "Raspberry";
    const REPORT_PREFIX: &'static str = "raspberry-autoaux";

    fn timing_evidence(&mut self) -> &mut Option<crate::timing_input::TimingStudyEvidence> {
        &mut self.timing_evidence
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

    fn handle_drum_hit(&mut self, hit: &DrumHit) -> Result<(), RuntimeAdapterError> {
        self.handle_runtime_drum_hit(hit)
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(shutdown_pending_messages(request));
        }
        if let Some(result) = dispatch_shared_effect(&self.core.platform_service, request) {
            return Ok(result);
        }
        if let Some(messages) = dispatch_midi_effect_messages(&mut self.core.midi, &request.effect)?
        {
            return Ok(messages);
        }
        if let Some(messages) = self.core.handle_store_effect(request) {
            return Ok(messages);
        }
        let result = match &request.effect {
            RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } => {
                self.core.pending_default_save.cancel();
                let recording_result = self.stop_recording_for_transition(request)?;
                let mut messages = recording_result
                    .into_iter()
                    .map(|result| HostMessage::RuntimeResult { result })
                    .collect::<Vec<_>>();
                if let Err(message) =
                    crate::rpi_device_apply::apply(&self.core.platform_service, payload)
                {
                    messages.push(failure_message(
                        request,
                        format!("device/audio apply save failed: {message}"),
                    ));
                    return Ok(messages);
                }
                self.power_request = Some(PiPowerRequest::ApplyDeviceConfigReboot);
                return Ok(messages);
            }
            RuntimePlatformEffect::RecordingStartAudio { .. }
            | RuntimePlatformEffect::RecordingStartAudioOled { .. }
            | RuntimePlatformEffect::RecordingStop => return self.handle_recording_effect(request),
            RuntimePlatformEffect::UsbSdTransferStart => {
                return self.start_usb_sd_transfer(request);
            }
            RuntimePlatformEffect::MidiPanic => {
                self.silence_internal_audio()?;
                self.core.midi_panic_status()
            }
            RuntimePlatformEffect::Reboot => {
                return self.request_power(request, PiPowerRequest::Reboot);
            }
            RuntimePlatformEffect::Shutdown => {
                return self.request_power(request, PiPowerRequest::Shutdown);
            }
            RuntimePlatformEffect::HardwareTest => {
                println!("system.hardwareTest requested (planned guided hardware diagnostic)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
            _ => return Ok(unsupported_messages(request)),
        };
        Ok(vec![HostMessage::RuntimeResult { result }])
    }

    fn acknowledge_restored_state(&mut self) -> Result<(), RuntimeAdapterError> {
        self.core.platform_service.acknowledge_restored_state();
        Ok(())
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        send_audio_command(self.audio.clone(), command, &self.samples_dir)?;
        if let Some(evidence) = self.timing_evidence.as_mut() {
            evidence.record_command(command);
        }
        Ok(())
    }
    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.core.send_midi(bytes)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        if let Some(audio) = &self.audio {
            audio.send_realtime(EngineEvent::AllNotesOff)?;
        }
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.core.panic_midi()
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

#[cfg(test)]
#[path = "host_adapter_power_tests.rs"]
mod power_tests;
#[cfg(test)]
#[path = "host_adapter_system_store_tests.rs"]
mod system_store_tests;
#[cfg(test)]
#[path = "host_adapter_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "host_adapter_usb_role_tests.rs"]
mod usb_role_tests;
