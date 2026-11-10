use super::{AudioOptimization, AudioOutputSet, NativeRunnerConfig, UsbDataRole};

/// The audio and USB I/O settings (outputs, USB role and MIDI out, audio optimization, the
/// recording limit) and what the host allows of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAudioIoSettings {
    pub(super) audio_outputs: AudioOutputSet,
    pub(super) usb_data_role: UsbDataRole,
    pub(super) usb_midi_out_enabled: bool,
    pub(super) audio_optimization: AudioOptimization,
    pub(super) recording_max_minutes: u16,
    pub(super) jack_audio_required: bool,
    pub(super) audio_optimization_capacity_available: bool,
    pub(super) usb_data_role_available: bool,
}

impl NativeAudioIoSettings {
    pub(super) fn new(config: &NativeRunnerConfig) -> Self {
        Self {
            audio_outputs: AudioOutputSet::default(),
            usb_data_role: UsbDataRole::default(),
            usb_midi_out_enabled: false,
            audio_optimization: config.audio_optimization,
            recording_max_minutes: 10,
            jack_audio_required: config.jack_audio_required,
            audio_optimization_capacity_available: config.audio_optimization_capacity_available,
            usb_data_role_available: config.usb_data_role_available,
        }
    }

    pub(super) fn supports_optimization(&self, optimization: AudioOptimization) -> bool {
        optimization.is_supported(self.audio_optimization_capacity_available)
    }
}
