#[path = "orange_host_adapter_construction.rs"]
mod construction;
#[path = "orange_host_adapter_native_persistence.rs"]
mod native_persistence;

use crate::audio::AudioService;
use crate::midi_host::RuntimeOutputSink;
use crate::orange_audio::OrangeAudioHost;
use crate::orange_device_apply::OrangeShutdownRequest;
use crate::pi_host_core::{
    failure_message, power_save_result, shutdown_pending_messages, unsupported_messages, PiHostCore,
};
use crate::platform_service::{
    dispatch_midi_effect_messages, dispatch_shared_effect, enqueue_job,
    usb_sd_transfer_output_block_reason, PlatformJobKind,
};
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};

pub(crate) struct OrangeHostAdapter {
    audio: AudioService,
    audio_host: OrangeAudioHost,
    pub(crate) core: PiHostCore,
    shutdown_request: Option<OrangeShutdownRequest>,
}

impl OrangeHostAdapter {
    pub(crate) fn handle_transfer_input(&self, message: &HostMessage) -> bool {
        self.core.handle_transfer_input(message)
    }

    pub(crate) fn take_transfer_status(&mut self) -> Option<HostMessage> {
        self.core.take_transfer_status()
    }

    pub(crate) fn poll_recording_status(&self) -> Option<RuntimeStoreResult> {
        self.audio.poll_recording_status()
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
        let recovery = self.core.take_recovery_save_status();
        power_save_result(recovery, self.audio.stop_recording())
    }
    pub(crate) fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = self.core.platform_service.drain_results(max_results);
        if results.len() < max_results {
            results.extend(
                self.audio
                    .drain_prep_results(max_results.saturating_sub(results.len())),
            );
        }
        results
    }

    pub(crate) fn drain_startup_platform_results(&self, max_results: usize) -> Vec<HostMessage> {
        self.core.platform_service.drain_results(max_results)
    }

    fn request_power(
        &mut self,
        request: &RuntimePlatformRequest,
        shutdown_request: OrangeShutdownRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if let Err(error) = self.core.recovery_save_ready() {
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
            .map_err(crate::audio_recording::recording_finalization_error)
    }

    fn start_usb_sd_transfer(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if let Some(reason) = usb_sd_transfer_output_block_reason(
            self.audio.usb_output_enabled(),
            self.core.midi.usb_midi_out_enabled(),
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
            &self.core.platform_service,
            request,
            PlatformJobKind::UsbSdTransferStart,
            "USB SD2 transfer start".into(),
        ))
    }
}

impl crate::timing_input::TimingHost for OrangeHostAdapter {
    const STUDY_BOARD: &'static str = "Orange";
    const REPORT_PREFIX: &'static str = "orange-autoaux";

    fn timing_evidence(&mut self) -> &mut Option<crate::timing_input::TimingStudyEvidence> {
        self.audio_host.timing_evidence()
    }
}

impl HostAdapter for OrangeHostAdapter {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.audio_host.handle_musical_event(event)
    }

    fn handle_drum_hit(
        &mut self,
        hit: &playback_runtime::DrumHit,
    ) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.audio_host.handle_drum_hit(hit)
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
                let transaction = self
                    .core
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
                self.core.midi_panic_status()
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
                return crate::audio_recording::recording_start_result(
                    self.audio
                        .start_recording_audio_oled_from_latest(*max_minutes),
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
        self.audio_host.handle_audio_command(command)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.core.send_midi(bytes)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.audio_host.silence_internal_audio()
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.core.panic_midi()
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

#[cfg(test)]
#[path = "orange_host_adapter_apply_tests.rs"]
mod apply_tests;
#[cfg(test)]
#[path = "orange_host_adapter_system_store_tests.rs"]
mod system_store_tests;
#[cfg(test)]
#[path = "orange_host_adapter_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "orange_host_adapter_update_tests.rs"]
mod update_tests;
