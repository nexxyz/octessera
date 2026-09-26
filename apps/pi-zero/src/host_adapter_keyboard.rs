use super::PiPlaybackHostAdapter;
use crate::usb_keyboard::KeyboardCaptureControl;
use serde_json::Value;

impl PiPlaybackHostAdapter {
    pub(crate) fn set_keyboard_capture_control(&mut self, control: KeyboardCaptureControl) {
        self.keyboard_control = Some(control);
    }

    pub(crate) fn observe_keyboard_capture_snapshot(&self, snapshot: &Value) {
        if let Some(control) = &self.keyboard_control {
            control.observe_snapshot(snapshot);
        }
    }

    pub(crate) fn observe_keyboard_capture_mode(&self, mode: playback_runtime::NativeHdmiMode) {
        if let Some(control) = &self.keyboard_control {
            control.observe_hdmi_mode(mode);
        }
    }
}
