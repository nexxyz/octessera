use playback_runtime::AudioOutputSet;
use std::fmt::{Display, Formatter};
use std::path::Path;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsbAudioOut {
    Jack,
    Usb,
    Both,
}

#[cfg(test)]
impl UsbAudioOut {
    pub(crate) fn outputs(self) -> AudioOutputSet {
        match self {
            Self::Jack => AudioOutputSet::jack(),
            Self::Usb => AudioOutputSet::from_flags(false, true, false).unwrap(),
            Self::Both => AudioOutputSet::from_flags(true, true, false).unwrap(),
        }
    }
}

#[cfg(test)]
impl From<UsbAudioOut> for AudioOutputSet {
    fn from(value: UsbAudioOut) -> Self {
        value.outputs()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsbRuntimeConfig {
    pub(crate) audio_outputs: AudioOutputSet,
    pub(crate) midi_out_enabled: bool,
}

impl Default for UsbRuntimeConfig {
    fn default() -> Self {
        Self {
            audio_outputs: AudioOutputSet::jack(),
            midi_out_enabled: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UsbConfigError {
    Read { path: String, message: String },
    Parse { path: String, message: String },
    Invalid(String),
}

impl Display for UsbConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, message } => {
                write!(
                    formatter,
                    "cannot read USB runtime config {path}: {message}"
                )
            }
            Self::Parse { path, message } => {
                write!(
                    formatter,
                    "cannot parse USB runtime config {path}: {message}"
                )
            }
            Self::Invalid(message) => write!(formatter, "invalid USB runtime config: {message}"),
        }
    }
}

pub(crate) fn read_usb_runtime_config(
    store_dir: &Path,
) -> Result<UsbRuntimeConfig, UsbConfigError> {
    let path = store_dir.join("default.json");
    let path_display = path.display().to_string();
    let payload = std::fs::read_to_string(&path).map_err(|error| UsbConfigError::Read {
        path: path_display.clone(),
        message: error.to_string(),
    })?;
    let payload = serde_json::from_str::<serde_json::Value>(&payload).map_err(|error| {
        UsbConfigError::Parse {
            path: path_display,
            message: error.to_string(),
        }
    })?;
    parse_usb_runtime_config(&payload)
}

pub(crate) fn audio_output_buffer_frames_from_default_config(store_dir: &Path) -> Option<u32> {
    let payload = std::fs::read_to_string(store_dir.join("default.json")).ok()?;
    let payload: serde_json::Value = serde_json::from_str(&payload).ok()?;
    payload
        .get("runtimeConfig")
        .unwrap_or(&payload)
        .get("sound")
        .and_then(|sound| sound.get("audioOutputBufferFrames"))
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as u32)
}

pub(crate) fn parse_usb_runtime_config(
    payload: &serde_json::Value,
) -> Result<UsbRuntimeConfig, UsbConfigError> {
    let root = payload.get("runtimeConfig").unwrap_or(payload);
    let Some(root) = root.as_object() else {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig must be an object".into(),
        ));
    };
    let usb = match root.get("usb") {
        None => None,
        Some(usb) => Some(usb.as_object().ok_or_else(|| {
            UsbConfigError::Invalid("runtimeConfig.usb must be an object".into())
        })?),
    };
    let audio_outputs = if root.contains_key("audioOutputs")
        || usb.is_some_and(|usb| usb.contains_key("audioOut"))
    {
        AudioOutputSet::decode_runtime_config(payload).map_err(UsbConfigError::Invalid)?
    } else {
        AudioOutputSet::jack()
    };
    let midi_out_enabled = match usb.and_then(|usb| usb.get("midiOutEnabled")) {
        None => false,
        Some(serde_json::Value::Bool(value)) => *value,
        Some(_) => {
            return Err(UsbConfigError::Invalid(
                "runtimeConfig.usb.midiOutEnabled must be boolean".into(),
            ))
        }
    };
    Ok(UsbRuntimeConfig {
        audio_outputs,
        midi_out_enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_jack_and_midi_off() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({})).unwrap(),
            UsbRuntimeConfig::default()
        );
    }

    #[test]
    fn parses_nested_usb_runtime_config() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": { "usb": { "audioOut": "both", "midiOutEnabled": true } }
            }))
            .unwrap(),
            UsbRuntimeConfig {
                audio_outputs: AudioOutputSet::from_flags(true, true, false).unwrap(),
                midi_out_enabled: true,
            }
        );
    }

    #[test]
    fn preserves_explicit_usb_policy_for_product_level_validation() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": { "usb": { "audioOut": "usb" } }
            }))
            .unwrap()
            .audio_outputs,
            AudioOutputSet::from_flags(false, true, false).unwrap()
        );
    }

    #[test]
    fn maps_canonical_audio_outputs_at_the_pi_boundary() {
        for (outputs, expected) in [
            (
                serde_json::json!({ "dac": true, "usb": false, "hdmi": false }),
                UsbAudioOut::Jack,
            ),
            (
                serde_json::json!({ "dac": false, "usb": true, "hdmi": false }),
                UsbAudioOut::Usb,
            ),
            (
                serde_json::json!({ "dac": true, "usb": true, "hdmi": false }),
                UsbAudioOut::Both,
            ),
        ] {
            assert_eq!(
                parse_usb_runtime_config(&serde_json::json!({
                    "runtimeConfig": { "audioOutputs": outputs }
                }))
                .unwrap()
                .audio_outputs,
                expected.outputs()
            );
        }
    }

    #[test]
    fn canonical_and_legacy_audio_outputs_must_agree() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": true, "hdmi": false },
                    "usb": { "audioOut": "both", "midiOutEnabled": true }
                }
            }))
            .unwrap(),
            UsbRuntimeConfig {
                audio_outputs: UsbAudioOut::Both.outputs(),
                midi_out_enabled: true,
            }
        );
        assert!(matches!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "audioOut": "usb" }
                }
            })),
            Err(UsbConfigError::Invalid(_))
        ));
    }

    #[test]
    fn canonical_hdmi_and_unrepresentable_outputs_are_profile_checked_later() {
        let audio_outputs = serde_json::json!({ "dac": true, "usb": false, "hdmi": true });
        assert!(parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": { "audioOutputs": audio_outputs }
        }))
        .is_ok());
        for audio_outputs in [
            serde_json::json!({ "dac": false, "usb": false, "hdmi": false }),
            serde_json::json!({ "dac": true, "usb": false, "hdmi": false, "extra": false }),
        ] {
            assert!(parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": { "audioOutputs": audio_outputs }
            }))
            .is_err());
        }
    }

    #[test]
    fn canonical_audio_outputs_do_not_change_legacy_midi_parsing() {
        let config = parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": {
                "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                "usb": { "midiOutEnabled": true }
            }
        }))
        .unwrap();

        assert_eq!(config.audio_outputs, UsbAudioOut::Jack.outputs());
        assert!(config.midi_out_enabled);
    }

    #[test]
    fn reads_audio_policy_from_the_persisted_default_store() {
        let store_dir = std::env::temp_dir().join(format!(
            "octessera-usb-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::write(
            store_dir.join("default.json"),
            r#"{"runtimeConfig":{"usb":{"audioOut":"both"}}}"#,
        )
        .unwrap();

        assert_eq!(
            read_usb_runtime_config(&store_dir).unwrap().audio_outputs,
            UsbAudioOut::Both.outputs()
        );
        let _ = std::fs::remove_dir_all(store_dir);
    }

    #[test]
    fn orange_startup_reads_persisted_audio_output_buffer_frames() {
        let store_dir = std::env::temp_dir().join(format!(
            "octessera-audio-buffer-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::write(
            store_dir.join("default.json"),
            r#"{"runtimeConfig":{"sound":{"audioOutputBufferFrames":1024}}}"#,
        )
        .unwrap();

        assert_eq!(
            audio_output_buffer_frames_from_default_config(&store_dir),
            Some(1024)
        );
        let _ = std::fs::remove_dir_all(store_dir);
    }

    #[test]
    fn rejects_malformed_and_wrong_shaped_usb_config() {
        assert!(parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": { "usb": { "audioOut": "mystery" } }
        }))
        .is_err());
        assert!(parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": { "usb": [] }
        }))
        .is_err());
        assert!(parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": []
        }))
        .is_err());
    }

    #[test]
    fn missing_usb_fields_keep_safe_defaults() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": { "usb": {} }
            }))
            .unwrap(),
            UsbRuntimeConfig::default()
        );
    }

    #[test]
    fn reports_wrong_store_path_and_malformed_file() {
        let missing = std::env::temp_dir().join(format!(
            "octessera-usb-config-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let error = read_usb_runtime_config(&missing).unwrap_err();
        assert!(matches!(error, UsbConfigError::Read { .. }));

        let malformed = std::env::temp_dir().join(format!(
            "octessera-usb-config-malformed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&malformed).unwrap();
        std::fs::write(malformed.join("default.json"), "{").unwrap();
        let error = read_usb_runtime_config(&malformed).unwrap_err();
        assert!(matches!(error, UsbConfigError::Parse { .. }));
        let _ = std::fs::remove_dir_all(malformed);
    }
}
