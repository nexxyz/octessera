use playback_runtime::{RuntimeBluetoothDevice, RuntimeBluetoothDeviceKind};

const HID_UUID: &str = "00001124-0000-1000-8000-00805f9b34fb";
const AUDIO_SINK_UUID: &str = "0000110b-0000-1000-8000-00805f9b34fb";
const MAJOR_CLASS_AUDIO: u32 = 0x04;
const MAJOR_CLASS_PERIPHERAL: u32 = 0x05;
const PERIPHERAL_KEYBOARD_BIT: u32 = 0x40;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DeviceRecord {
    pub(crate) address: String,
    pub(crate) name: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) class: Option<u32>,
    pub(crate) uuids: Vec<String>,
    pub(crate) paired: bool,
    pub(crate) connected: bool,
}

impl DeviceRecord {
    pub(crate) fn kind(&self) -> RuntimeBluetoothDeviceKind {
        match self.icon.as_deref() {
            Some("input-keyboard") => return RuntimeBluetoothDeviceKind::Keyboard,
            Some(icon) if icon.starts_with("audio-") => return RuntimeBluetoothDeviceKind::Audio,
            _ => {}
        }
        if self.has_uuid(AUDIO_SINK_UUID) {
            return RuntimeBluetoothDeviceKind::Audio;
        }
        if let Some(class) = self.class {
            let major = (class >> 8) & 0x1f;
            if major == MAJOR_CLASS_AUDIO {
                return RuntimeBluetoothDeviceKind::Audio;
            }
            if major == MAJOR_CLASS_PERIPHERAL && class & PERIPHERAL_KEYBOARD_BIT != 0 {
                return RuntimeBluetoothDeviceKind::Keyboard;
            }
        }
        if self.has_uuid(HID_UUID) && self.icon.as_deref() != Some("input-mouse") {
            return RuntimeBluetoothDeviceKind::Keyboard;
        }
        RuntimeBluetoothDeviceKind::Other
    }

    fn has_uuid(&self, uuid: &str) -> bool {
        self.uuids
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(uuid))
    }

    pub(crate) fn to_runtime(&self) -> RuntimeBluetoothDevice {
        RuntimeBluetoothDevice {
            address: self.address.clone(),
            name: self.name.clone().unwrap_or_else(|| self.address.clone()),
            kind: self.kind(),
            paired: self.paired,
            connected: self.connected,
        }
    }
}
