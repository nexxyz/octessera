use std::fmt::{Display, Formatter};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AudioOutputState {
    dac: bool,
    usb: bool,
}

impl AudioOutputState {
    fn legacy(audio_out: UsbAudioOut) -> Self {
        match audio_out {
            UsbAudioOut::Jack => Self {
                dac: true,
                usb: false,
            },
            UsbAudioOut::Usb => Self {
                dac: false,
                usb: true,
            },
            UsbAudioOut::Both => Self {
                dac: true,
                usb: true,
            },
        }
    }

    fn into_audio_out(self) -> Result<UsbAudioOut, UsbConfigError> {
        match (self.dac, self.usb) {
            (true, false) => Ok(UsbAudioOut::Jack),
            (false, true) => Ok(UsbAudioOut::Usb),
            (true, true) => Ok(UsbAudioOut::Both),
            (false, false) => Err(UsbConfigError::Invalid(
                "runtimeConfig.audioOutputs must enable DAC or USB audio".into(),
            )),
        }
    }
}

impl Default for UsbRuntimeConfig {
    fn default() -> Self {
        Self {
            audio_out: UsbAudioOut::Jack,
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
    let canonical_audio = parse_canonical_audio_outputs(root)?;
    let legacy_audio = usb
        .and_then(|usb| usb.get("audioOut"))
        .map(parse_legacy_audio_out)
        .transpose()?
        .map(AudioOutputState::legacy);
    let audio_state = match (canonical_audio, legacy_audio) {
        (Some(canonical), Some(legacy)) if canonical != legacy => {
            return Err(UsbConfigError::Invalid(
                "runtimeConfig.audioOutputs and runtimeConfig.usb.audioOut disagree".into(),
            ));
        }
        (Some(canonical), _) => canonical,
        (None, Some(legacy)) => legacy,
        (None, None) => AudioOutputState::legacy(UsbAudioOut::Jack),
    };
    let audio_out = audio_state.into_audio_out()?;
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
        audio_out,
        midi_out_enabled,
    })
}

fn parse_canonical_audio_outputs(
    root: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<AudioOutputState>, UsbConfigError> {
    let Some(value) = root.get("audioOutputs") else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig.audioOutputs must be an object".into(),
        ));
    };
    if object.len() != 3
        || ["dac", "usb", "hdmi"]
            .iter()
            .any(|key| !object.contains_key(*key))
    {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig.audioOutputs must contain exactly boolean dac, usb, and hdmi fields"
                .into(),
        ));
    }
    let read_bool = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| {
                UsbConfigError::Invalid(format!("runtimeConfig.audioOutputs.{key} must be boolean"))
            })
    };
    let dac = read_bool("dac")?;
    let usb = read_bool("usb")?;
    if read_bool("hdmi")? {
        return Err(UsbConfigError::Invalid(
            "runtimeConfig.audioOutputs.hdmi=true is unsupported by this device".into(),
        ));
    }
    Ok(Some(AudioOutputState { dac, usb }))
}

fn parse_legacy_audio_out(value: &serde_json::Value) -> Result<UsbAudioOut, UsbConfigError> {
    match value {
        serde_json::Value::String(value) => match value.as_str() {
            "jack" => Ok(UsbAudioOut::Jack),
            "usb" => Ok(UsbAudioOut::Usb),
            "both" => Ok(UsbAudioOut::Both),
            value => Err(UsbConfigError::Invalid(format!(
                "runtimeConfig.usb.audioOut has unsupported value {value:?}"
            ))),
        },
        _ => Err(UsbConfigError::Invalid(
            "runtimeConfig.usb.audioOut must be a string".into(),
        )),
    }
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
                audio_out: UsbAudioOut::Both,
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
            .audio_out,
            UsbAudioOut::Usb
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
                .audio_out,
                expected
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
                audio_out: UsbAudioOut::Both,
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
    fn canonical_hdmi_and_unrepresentable_outputs_fail_closed() {
        for audio_outputs in [
            serde_json::json!({ "dac": true, "usb": false, "hdmi": true }),
            serde_json::json!({ "dac": false, "usb": false, "hdmi": false }),
            serde_json::json!({ "dac": true, "usb": false, "hdmi": false, "extra": false }),
        ] {
            assert!(matches!(
                parse_usb_runtime_config(&serde_json::json!({
                    "runtimeConfig": { "audioOutputs": audio_outputs }
                })),
                Err(UsbConfigError::Invalid(_))
            ));
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

        assert_eq!(config.audio_out, UsbAudioOut::Jack);
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
            read_usb_runtime_config(&store_dir).unwrap().audio_out,
            UsbAudioOut::Both
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
