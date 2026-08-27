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
        if let Some(usb) = runtime.get("usb") {
            let usb = usb
                .as_object()
                .ok_or_else(|| "runtimeConfig.usb must be an object".to_string())?;
            if usb.contains_key("audioOut") {
                return Err("runtimeConfig.usb.audioOut is unsupported".into());
            }
        }
        runtime
            .get("audioOutputs")
            .ok_or_else(|| "runtimeConfig must contain audioOutputs".to_string())
            .and_then(|value| decode_canonical(value, "runtimeConfig.audioOutputs"))
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
}

fn bool_field(object: &Map<String, Value>, key: &str, path: &str) -> Result<bool, String> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{path}.{key} must be a boolean"))
}
