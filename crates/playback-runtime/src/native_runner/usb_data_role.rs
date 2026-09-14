use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UsbDataRole {
    #[default]
    Gadget,
    Host,
}

impl UsbDataRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gadget => "gadget",
            Self::Host => "host",
        }
    }

    pub const fn is_host(self) -> bool {
        matches!(self, Self::Host)
    }

    pub fn from_menu_value(value: &str) -> Option<Self> {
        match value {
            "Gadget" | "gadget" => Some(Self::Gadget),
            "Host" | "host" => Some(Self::Host),
            _ => None,
        }
    }
}

impl super::NativeRunner {
    pub(super) fn validate_usb_data_role(&self) -> Result<(), String> {
        if self.usb_data_role.is_host() {
            if self.audio_outputs.usb() {
                return Err(
                    "runtimeConfig.audioOutputs.usb must be false when runtimeConfig.usb.dataRole is host"
                        .into(),
                );
            }
            if self.usb_midi_out_enabled {
                return Err(
                    "runtimeConfig.usb.midiOutEnabled must be false when runtimeConfig.usb.dataRole is host"
                        .into(),
                );
            }
        }
        Ok(())
    }
}
