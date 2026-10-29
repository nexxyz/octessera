use crate::protocol::RuntimePlatformEffect;
use serde_json::Value;

use super::NativeRunner;

/// System settings the board applies live: the side audio outputs, the USB
/// MIDI gadget, and the USB data role.
pub(super) fn is_device_config_key(key: &str) -> bool {
    matches!(
        key,
        "audioOutputs.usb" | "audioOutputs.hdmi" | "usb.midiOutEnabled" | "usb.dataRole"
    )
}

impl NativeRunner {
    pub(super) fn apply_device_config(&mut self, system: Value) {
        self.show_toast("Applying...");
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::ApplyDeviceConfig { payload: system });
    }
}
