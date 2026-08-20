use super::canonical::{bool_field, enum_field, object_field, unsigned_field};
use super::Value;
use crate::AudioOutputSet;
use serde_json::Map;

pub(super) fn validate_midi(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(midi) = object_field(runtime, "midi", "runtimeConfig")? else {
        return Ok(());
    };
    let path = "runtimeConfig.midi";
    bool_field(midi, "enabled", path)?;
    for key in ["outId", "inId"] {
        if let Some(value) = midi.get(key) {
            if !value.is_null() && !value.is_string() {
                return Err(format!("{path}.{key} must be a string or null"));
            }
        }
    }
    enum_field(midi, "syncMode", path, &["internal", "external"])?;
    for key in ["clockOutEnabled", "clockInEnabled", "respondToStartStop"] {
        bool_field(midi, key, path)?;
    }
    Ok(())
}

pub(super) fn validate_audio_outputs(runtime: &Map<String, Value>) -> Result<(), String> {
    AudioOutputSet::decode_runtime_fields(runtime)?;
    Ok(())
}

pub(super) fn validate_usb(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(usb) = object_field(runtime, "usb", "runtimeConfig")? else {
        return Ok(());
    };
    if usb.contains_key("midiOutEnabled") {
        bool_field(usb, "midiOutEnabled", "runtimeConfig.usb")?;
    }
    Ok(())
}

pub(super) fn validate_hdmi(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(hdmi) = object_field(runtime, "hdmi", "runtimeConfig")? else {
        return Ok(());
    };
    enum_field(
        hdmi,
        "mode",
        "runtimeConfig.hdmi",
        &[
            "none",
            "live-grid",
            "plain-grid",
            "active-behavior",
            "cycle-behaviors",
        ],
    )?;
    bool_field(hdmi, "showGridlines", "runtimeConfig.hdmi")?;
    unsigned_field(hdmi, "cycleMeasures", "runtimeConfig.hdmi", 1, 64)
}

pub(super) fn validate_recording(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(recording) = object_field(runtime, "recording", "runtimeConfig")? else {
        return Ok(());
    };
    unsigned_field(recording, "maxMinutes", "runtimeConfig.recording", 1, 120)
}
