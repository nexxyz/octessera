#[cfg(any(test, feature = "native-audio"))]
use playback_runtime::AudioOptimization;
use playback_runtime::{AudioOutputSet, UsbDataRole};
#[cfg(any(test, feature = "native-audio"))]
use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::path::Path;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsbAudioOut {
    Jack,
    Both,
}

#[cfg(test)]
impl UsbAudioOut {
    pub(crate) fn outputs(self) -> AudioOutputSet {
        match self {
            Self::Jack => AudioOutputSet::jack(),
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
    pub(crate) data_role: UsbDataRole,
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

#[cfg(feature = "native-audio")]
pub(crate) fn read_audio_optimization_from_default_config(
    store_dir: &Path,
) -> Result<AudioOptimization, UsbConfigError> {
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
    parse_audio_optimization(&payload)
}

#[cfg(any(test, feature = "native-audio"))]
pub(crate) fn parse_audio_optimization(
    payload: &serde_json::Value,
) -> Result<AudioOptimization, UsbConfigError> {
    let root = payload.get("runtimeConfig").unwrap_or(payload);
    let Some(root) = root.as_object() else {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig must be an object".into(),
        ));
    };
    let Some(sound) = root.get("sound") else {
        return Ok(AudioOptimization::Latency);
    };
    let sound = sound
        .as_object()
        .ok_or_else(|| UsbConfigError::Invalid("runtimeConfig.sound must be an object".into()))?;
    let Some(value) = sound.get("optimizeFor") else {
        return Ok(AudioOptimization::Latency);
    };
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TypedSound {
        #[serde(default)]
        optimize_for: AudioOptimization,
    }
    serde_json::from_value::<TypedSound>(serde_json::json!({ "optimizeFor": value }))
        .map(|sound| sound.optimize_for)
        .map_err(|_| {
            UsbConfigError::Invalid(
                "runtimeConfig.sound.optimizeFor must be `latency` or `capacity`".into(),
            )
        })
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
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
    let root = payload
        .get("runtimeConfig")
        .ok_or_else(|| UsbConfigError::Invalid("runtimeConfig must be an object".into()))?;
    let Some(root) = root.as_object() else {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig must be an object".into(),
        ));
    };
    let audio_outputs = root
        .get("audioOutputs")
        .ok_or_else(|| UsbConfigError::Invalid("runtimeConfig.audioOutputs is required".into()))
        .and_then(|value| AudioOutputSet::decode(value).map_err(UsbConfigError::Invalid))?;
    if !audio_outputs.dac() {
        return Err(UsbConfigError::Invalid("Jack Audio is always on".into()));
    }
    let usb = root
        .get("usb")
        .ok_or_else(|| UsbConfigError::Invalid("runtimeConfig.usb is required".into()))?
        .as_object()
        .ok_or_else(|| UsbConfigError::Invalid("runtimeConfig.usb must be an object".into()))?;
    if usb
        .keys()
        .any(|key| !matches!(key.as_str(), "midiOutEnabled" | "dataRole"))
    {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig.usb contains unsupported fields".into(),
        ));
    }
    let midi_out_enabled = usb
        .get("midiOutEnabled")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            UsbConfigError::Invalid("runtimeConfig.usb.midiOutEnabled must be boolean".into())
        })?;
    let data_role = usb
        .get("dataRole")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            UsbConfigError::Invalid("runtimeConfig.usb.dataRole must be `gadget` or `host`".into())
        })?;
    let data_role = match data_role {
        "gadget" => UsbDataRole::Gadget,
        "host" => UsbDataRole::Host,
        _ => {
            return Err(UsbConfigError::Invalid(
                "runtimeConfig.usb.dataRole must be `gadget` or `host`".into(),
            ))
        }
    };
    if data_role == UsbDataRole::Host && (audio_outputs.usb() || midi_out_enabled) {
        return Err(UsbConfigError::Invalid(
            "host USB data role requires USB audio and MIDI to be disabled".into(),
        ));
    }
    Ok(UsbRuntimeConfig {
        audio_outputs,
        midi_out_enabled,
        data_role,
    })
}

#[cfg(test)]
#[path = "usb_config_audio_optimization_tests.rs"]
mod audio_optimization_tests;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
#[path = "usb_config_role_tests.rs"]
mod role_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_canonical_usb_runtime_config() {
        for payload in [
            serde_json::json!({}),
            serde_json::json!({ "runtimeConfig": {} }),
            serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false }
                }
            }),
        ] {
            assert!(parse_usb_runtime_config(&payload).is_err());
        }
    }

    #[test]
    fn parses_nested_usb_runtime_config() {
        assert_eq!(
            parse_usb_runtime_config(&serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": true, "hdmi": false },
                    "usb": { "midiOutEnabled": true, "dataRole": "gadget" }
                }
            }))
            .unwrap(),
            UsbRuntimeConfig {
                audio_outputs: AudioOutputSet::from_flags(true, true, false).unwrap(),
                midi_out_enabled: true,
                data_role: UsbDataRole::Gadget,
            }
        );
    }

    #[test]
    fn rejects_missing_jack_at_the_pi_config_boundary() {
        let error = parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": {
                "audioOutputs": { "dac": false, "usb": true, "hdmi": false },
                "usb": { "midiOutEnabled": false, "dataRole": "gadget" }
            }
        }))
        .unwrap_err();
        assert_eq!(
            error,
            UsbConfigError::Invalid("Jack Audio is always on".into())
        );
    }

    #[test]
    fn rejects_missing_jack_before_a_pi_config_write() {
        let error =
            crate::usb_config_validation::validate_pi_audio_outputs_payload(&serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": false, "usb": false, "hdmi": false }
                }
            }))
            .unwrap_err();
        assert_eq!(error, "Jack Audio is always on");
    }

    #[test]
    fn maps_canonical_audio_outputs_at_the_pi_boundary() {
        for (outputs, expected) in [
            (
                serde_json::json!({ "dac": true, "usb": false, "hdmi": false }),
                UsbAudioOut::Jack,
            ),
            (
                serde_json::json!({ "dac": true, "usb": true, "hdmi": false }),
                UsbAudioOut::Both,
            ),
        ] {
            assert_eq!(
                parse_usb_runtime_config(&serde_json::json!({
                    "runtimeConfig": {
                        "audioOutputs": outputs,
                        "usb": { "midiOutEnabled": false, "dataRole": "gadget" }
                    }
                }))
                .unwrap()
                .audio_outputs,
                expected.outputs()
            );
        }
    }

    #[test]
    fn canonical_hdmi_and_unrepresentable_outputs_are_profile_checked_later() {
        let audio_outputs = serde_json::json!({ "dac": true, "usb": false, "hdmi": true });
        assert!(parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": {
                "audioOutputs": audio_outputs,
                "usb": { "midiOutEnabled": false, "dataRole": "gadget" }
            }
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
    fn canonical_audio_outputs_preserve_usb_midi_parsing() {
        let config = parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": {
                "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                "usb": { "midiOutEnabled": true, "dataRole": "gadget" }
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
            r#"{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":false},"usb":{"midiOutEnabled":false,"dataRole":"gadget"}}}"#,
        )
        .unwrap();

        assert_eq!(
            read_usb_runtime_config(&store_dir).unwrap().audio_outputs,
            UsbAudioOut::Both.outputs()
        );
        let _ = std::fs::remove_dir_all(store_dir);
    }

    #[test]
    fn missing_sound_optimize_for_defaults_to_latency() {
        for payload in [
            serde_json::json!({}),
            serde_json::json!({
                "runtimeConfig": { "sound": {} }
            }),
        ] {
            assert_eq!(
                parse_audio_optimization(&payload).unwrap(),
                AudioOptimization::Latency
            );
        }
    }

    #[test]
    fn parses_typed_optimize_for_and_rejects_unknown_values() {
        assert_eq!(
            parse_audio_optimization(&serde_json::json!({
                "runtimeConfig": { "sound": { "optimizeFor": "capacity" } }
            }))
            .unwrap(),
            AudioOptimization::Capacity
        );
        assert_eq!(
            parse_audio_optimization(&serde_json::json!({
                "runtimeConfig": { "sound": { "optimizeFor": "balanced" } }
            }))
            .unwrap_err(),
            UsbConfigError::Invalid(
                "runtimeConfig.sound.optimizeFor must be `latency` or `capacity`".into()
            )
        );
    }

    #[test]
    fn rejects_malformed_and_wrong_shaped_usb_config() {
        assert!(parse_usb_runtime_config(&serde_json::json!({
            "runtimeConfig": { "audioOutputs": "mystery" }
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
    fn rejects_missing_or_unknown_usb_fields() {
        for payload in [
            serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "dataRole": "gadget" }
                }
            }),
            serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "midiOutEnabled": false }
                }
            }),
            serde_json::json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": {
                        "midiOutEnabled": false,
                        "dataRole": "gadget",
                        "unknown": true
                    }
                }
            }),
        ] {
            assert!(parse_usb_runtime_config(&payload).is_err());
        }
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
