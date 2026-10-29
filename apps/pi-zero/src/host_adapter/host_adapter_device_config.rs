use super::PiHostAdapter;
use crate::device_apply::UsbChange;
use crate::pi_host_core::failure_message;
use crate::platform_service::{enqueue_job, PlatformJobKind};
use playback_runtime::{HostMessage, RuntimePlatformRequest};

impl PiHostAdapter {
    /// Applies the audio outputs, USB MIDI and USB role at once on the runtime
    /// side, then lets the platform worker save System and run the USB helper.
    pub(super) fn apply_device_config(
        &mut self,
        request: &RuntimePlatformRequest,
        payload: &serde_json::Value,
    ) -> Vec<HostMessage> {
        let config = match crate::usb_config::parse_usb_runtime_config(payload) {
            Ok(config) => config,
            Err(error) => return vec![failure_message(request, error.to_string())],
        };
        let gadget_changed = config.audio_outputs.usb() != self.audio_outputs.usb()
            || config.midi_out_enabled != self.usb_midi_out_enabled;
        let usb_change = if config.data_role != self.usb_data_role {
            Some(UsbChange::Role(config.data_role))
        } else {
            gadget_changed.then_some(UsbChange::Gadget)
        };
        self.audio_outputs = config.audio_outputs;
        if let Some(audio) = &self.audio {
            audio.set_audio_outputs(config.audio_outputs);
        }
        self.usb_midi_out_enabled = config.midi_out_enabled;
        self.core
            .midi
            .set_usb_midi_out_enabled(config.midi_out_enabled);
        if config.data_role != self.usb_data_role {
            self.usb_data_role = config.data_role;
            self.core.observe_usb_host_role(config.data_role.is_host());
        }
        enqueue_job(
            &self.core.platform_service,
            request,
            PlatformJobKind::ApplyDeviceConfig {
                payload: payload.clone(),
                usb_change,
            },
            "device apply".into(),
        )
    }
}
