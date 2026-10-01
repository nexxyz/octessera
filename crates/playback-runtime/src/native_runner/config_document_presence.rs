use serde_json::Value;

const REQUIRED_RUNTIME_KEYS: &[&str] = &[
    "activeBehavior",
    "activeLayerIndex",
    "linkLfos",
    "xy",
    "layers",
    "playFx",
    "transport",
    "xyRelease",
    "sampleFavouriteDirs",
    "hdmi",
    "instruments",
    "mixer",
    "masterVolume",
    "sound",
    "dsp",
    "noteLengthMs",
    "velocityScalePct",
    "velocityCurve",
    "voiceStealingMode",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
    "bpm",
    "playMode",
    "auxBindings",
    "shiftAuxBindings",
    "midi",
    "usb",
    "audioOutputs",
    "recording",
];

const SOUND_KEYS: &[&str] = &[
    "noteLengthMs",
    "velocityScalePct",
    "velocityCurve",
    "voiceStealingMode",
    "audioOutputBufferFrames",
    "optimizeFor",
];
const MIDI_KEYS: &[&str] = &[
    "enabled",
    "outId",
    "inId",
    "syncMode",
    "clockOutEnabled",
    "clockInEnabled",
    "respondToStartStop",
];
const USB_KEYS: &[&str] = &["dataRole", "midiOutEnabled"];
const AUDIO_OUTPUT_KEYS: &[&str] = &["dac", "usb", "hdmi"];
const RECORDING_KEYS: &[&str] = &["maxMinutes"];
const TRANSPORT_KEYS: &[&str] = &["bpm", "swingPct"];
const XY_KEYS: &[&str] = &["x", "y", "smoothingMs", "xInvert", "yInvert"];
const PLAY_FX_KEYS: &[&str] = &["selected", "assignments"];
const PLAY_FX_SELECTED_KEYS: &[&str] = &["fxType", "targetKey", "params"];
const PLAY_FX_ASSIGNMENT_KEYS: &[&str] = &["x", "y", "config"];
const HDMI_KEYS: &[&str] = &["mode", "showGridlines", "cycleMeasures"];
const DSP_KEYS: &[&str] = &["busIdleThreshold", "workerWarningThreshold"];

pub(super) fn require_full_runtime_presence(runtime: &Value, context: &str) -> Result<(), String> {
    require_fields(runtime, REQUIRED_RUNTIME_KEYS, context)?;
    require_aux_banks(runtime, context)?;
    require_nested(runtime, "sound", SOUND_KEYS, context)?;
    require_nested(runtime, "midi", MIDI_KEYS, context)?;
    require_nested(runtime, "usb", USB_KEYS, context)?;
    require_nested(runtime, "audioOutputs", AUDIO_OUTPUT_KEYS, context)?;
    require_nested(runtime, "recording", RECORDING_KEYS, context)?;
    require_nested(runtime, "transport", TRANSPORT_KEYS, context)?;
    require_nested(runtime, "xy", XY_KEYS, context)?;
    require_nested(runtime, "hdmi", HDMI_KEYS, context)?;
    require_nested(runtime, "dsp", DSP_KEYS, context)?;
    validate_play_fx(runtime, context)
}

pub(super) fn require_aux_banks(runtime: &Value, context: &str) -> Result<(), String> {
    for bank in ["auxBindings", "shiftAuxBindings"] {
        if !runtime.get(bank).is_some_and(Value::is_object) {
            return Err(format!("{context}.{bank} must be present as an object"));
        }
    }
    Ok(())
}

fn require_nested(
    runtime: &Value,
    key: &str,
    required: &[&str],
    context: &str,
) -> Result<(), String> {
    let path = format!("{context}.{key}");
    let object = runtime
        .get(key)
        .filter(|value| value.is_object())
        .ok_or_else(|| format!("{path} must be present as an object"))?;
    require_fields(object, required, &path)
}

fn require_fields(value: &Value, required: &[&str], context: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    for key in required {
        if !object.contains_key(*key) {
            return Err(format!("{context}.{key} is required"));
        }
    }
    Ok(())
}

fn validate_play_fx(runtime: &Value, context: &str) -> Result<(), String> {
    let path = format!("{context}.playFx");
    require_nested(runtime, "playFx", PLAY_FX_KEYS, context)?;
    let play_fx = runtime.get("playFx").unwrap();
    if let Some(selected) = play_fx.get("selected").filter(|value| value.is_object()) {
        require_fields(selected, PLAY_FX_SELECTED_KEYS, &format!("{path}.selected"))?;
    }
    let assignments = play_fx
        .get("assignments")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{path}.assignments must be an array"))?;
    for (index, assignment) in assignments.iter().enumerate() {
        require_fields(
            assignment,
            PLAY_FX_ASSIGNMENT_KEYS,
            &format!("{path}.assignments[{index}]"),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NativeRunner, NativeRunnerConfig};

    #[test]
    fn config_documents_presence_contract_matches_native_snapshot_emitter() {
        let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let snapshot = runner.capture_config_snapshot().into_payload();
        require_full_runtime_presence(&snapshot["runtimeConfig"], "snapshot.runtimeConfig")
            .unwrap();
    }
}
