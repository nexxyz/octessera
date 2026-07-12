use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsbAudioOut {
    Jack,
    Usb,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsbRuntimeConfig {
    pub(crate) audio_out: UsbAudioOut,
    pub(crate) midi_out_enabled: bool,
}

impl Default for UsbRuntimeConfig {
    fn default() -> Self {
        Self {
            audio_out: UsbAudioOut::Jack,
            midi_out_enabled: false,
        }
    }
}

#[derive(Deserialize)]
struct UsbPayload {
    #[serde(default, rename = "audioOut")]
    audio_out: Option<String>,
    #[serde(default, rename = "midiOutEnabled")]
    midi_out_enabled: bool,
}

pub(crate) fn read_usb_runtime_config(store_dir: &Path) -> UsbRuntimeConfig {
    let Some(payload) = std::fs::read_to_string(store_dir.join("default.json"))
        .ok()
        .and_then(|payload| serde_json::from_str::<serde_json::Value>(&payload).ok())
    else {
        return UsbRuntimeConfig::default();
    };
    parse_usb_runtime_config(&payload)
}

pub(crate) fn parse_usb_runtime_config(payload: &serde_json::Value) -> UsbRuntimeConfig {
    let Some(usb) = payload
        .get("runtimeConfig")
        .unwrap_or(payload)
        .get("usb")
        .cloned()
    else {
        return UsbRuntimeConfig::default();
    };
    let Ok(usb) = serde_json::from_value::<UsbPayload>(usb) else {
        return UsbRuntimeConfig::default();
    };
    UsbRuntimeConfig {
        audio_out: match usb.audio_out.as_deref() {
            Some("usb") => UsbAudioOut::Usb,
            Some("both") => UsbAudioOut::Both,
            _ => UsbAudioOut::Jack,
        },
        midi_out_enabled: usb.midi_out_enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_jack_and_midi_off() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({})),
            UsbRuntimeConfig::default()
        );
    }

    #[test]
    fn parses_nested_usb_runtime_config() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": { "usb": { "audioOut": "both", "midiOutEnabled": true } }
            })),
            UsbRuntimeConfig {
                audio_out: UsbAudioOut::Both,
                midi_out_enabled: true,
            }
        );
    }
}
