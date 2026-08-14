use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioOutputSet {
    dac: bool,
    usb: bool,
    hdmi: bool,
}

impl Default for AudioOutputSet {
    fn default() -> Self {
        Self::jack()
    }
}

impl AudioOutputSet {
    pub const fn jack() -> Self {
        Self {
            dac: true,
            usb: false,
            hdmi: false,
        }
    }

    pub const fn from_flags(dac: bool, usb: bool, hdmi: bool) -> Result<Self, &'static str> {
        if !dac && !usb && !hdmi {
            return Err("at least one audio output must be enabled");
        }
        Ok(Self { dac, usb, hdmi })
    }

    pub const fn dac(self) -> bool {
        self.dac
    }

    pub const fn usb(self) -> bool {
        self.usb
    }

    pub const fn hdmi(self) -> bool {
        self.hdmi
    }

    pub fn decode(value: &Value) -> Result<Self, String> {
        decode_canonical(value, "runtimeConfig.audioOutputs")
    }

    pub fn decode_runtime_config(value: &Value) -> Result<Self, String> {
        let runtime = value
            .get("runtimeConfig")
            .unwrap_or(value)
            .as_object()
            .ok_or_else(|| "runtimeConfig must be an object".to_string())?;
        Self::decode_runtime_fields(runtime)?
            .ok_or_else(|| "runtimeConfig must contain audioOutputs or usb.audioOut".to_string())
    }

    pub(crate) fn decode_runtime_fields(
        runtime: &Map<String, Value>,
    ) -> Result<Option<Self>, String> {
        let canonical = runtime
            .get("audioOutputs")
            .map(|value| decode_canonical(value, "runtimeConfig.audioOutputs"))
            .transpose()?;
        let legacy = match runtime.get("usb") {
            None => None,
            Some(value) => {
                let usb = value
                    .as_object()
                    .ok_or_else(|| "runtimeConfig.usb must be an object".to_string())?;
                usb.get("audioOut")
                    .map(|value| decode_legacy(value, "runtimeConfig.usb.audioOut"))
                    .transpose()?
            }
        };
        merge_canonical_and_legacy(canonical, legacy)
    }

    pub(crate) fn as_value(self) -> Value {
        json!({
            "dac": self.dac,
            "usb": self.usb,
            "hdmi": self.hdmi,
        })
    }
}

fn decode_canonical(value: &Value, path: &str) -> Result<AudioOutputSet, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    if object.len() != 3
        || ["dac", "usb", "hdmi"]
            .iter()
            .any(|key| !object.contains_key(*key))
    {
        return Err(format!(
            "{path} must contain exactly boolean dac, usb, and hdmi fields"
        ));
    }
    let dac = bool_field(object, "dac", path)?;
    let usb = bool_field(object, "usb", path)?;
    let hdmi = bool_field(object, "hdmi", path)?;
    AudioOutputSet::from_flags(dac, usb, hdmi).map_err(|error| format!("{path} {error}"))
}

fn decode_legacy(value: &Value, path: &str) -> Result<AudioOutputSet, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("{path} must be a string"))?;
    match value {
        "jack" => Ok(AudioOutputSet::jack()),
        "usb" => AudioOutputSet::from_flags(false, true, false).map_err(str::to_string),
        "both" => AudioOutputSet::from_flags(true, true, false).map_err(str::to_string),
        _ => Err(format!("{path} has unsupported value `{value}`")),
    }
}

fn merge_canonical_and_legacy(
    canonical: Option<AudioOutputSet>,
    legacy: Option<AudioOutputSet>,
) -> Result<Option<AudioOutputSet>, String> {
    match (canonical, legacy) {
        (Some(canonical), Some(legacy))
            if canonical.dac() != legacy.dac() || canonical.usb() != legacy.usb() =>
        {
            Err("runtimeConfig.audioOutputs and runtimeConfig.usb.audioOut disagree".into())
        }
        (Some(canonical), _) => Ok(Some(canonical)),
        (None, Some(legacy)) => Ok(Some(legacy)),
        (None, None) => Ok(None),
    }
}

pub(crate) fn normalize_audio_outputs(payload: &mut Value) -> Result<(), String> {
    let runtime_value = if payload.get("runtimeConfig").is_some() {
        payload.get_mut("runtimeConfig").expect("runtimeConfig")
    } else {
        payload
    };
    let runtime = runtime_value
        .as_object_mut()
        .ok_or_else(|| "runtimeConfig must be an object".to_string())?;
    let Some(outputs) = AudioOutputSet::decode_runtime_fields(runtime)? else {
        return Ok(());
    };
    runtime.insert("audioOutputs".into(), outputs.as_value());
    if let Some(usb) = runtime.get_mut("usb").and_then(Value::as_object_mut) {
        usb.remove("audioOut");
    }
    Ok(())
}

pub(crate) fn strip_device_audio_fields(payload: &mut Value) {
    let runtime_value = if payload.get("runtimeConfig").is_some() {
        payload.get_mut("runtimeConfig").expect("runtimeConfig")
    } else {
        payload
    };
    let Some(runtime) = runtime_value.as_object_mut() else {
        return;
    };
    runtime.remove("audioOutputs");
    if let Some(usb) = runtime.get_mut("usb").and_then(Value::as_object_mut) {
        usb.remove("audioOut");
    }
}

fn bool_field(object: &Map<String, Value>, key: &str, path: &str) -> Result<bool, String> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{path}.{key} must be a boolean"))
}
