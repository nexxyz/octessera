use super::canonical::{
    enum_field, number_field, object_field, object_value, signed_value, unsigned_field,
};
use super::device_io;
use super::instruments;
use super::layers;
use super::mapping_bindings;
use super::mixer_fx;
use super::modulation;
use super::Value;
use serde_json::Map;

pub(super) fn validate_runtime(runtime: &Map<String, Value>) -> Result<(), String> {
    super::canonical::behavior_field(runtime, "activeBehavior", "runtimeConfig")?;
    unsigned_field(
        runtime,
        "activeLayerIndex",
        "runtimeConfig",
        0,
        (platform_core::LAYER_COUNT - 1) as u64,
    )?;
    enum_field(runtime, "velocityCurve", "runtimeConfig", VELOCITY_CURVES)?;
    enum_field(runtime, "voiceStealingMode", "runtimeConfig", VOICE_MODES)?;
    enum_field(
        runtime,
        "numericDisplayMode",
        "runtimeConfig",
        DISPLAY_MODES,
    )?;
    enum_field(runtime, "sparksMode", "runtimeConfig", SPARKS_MODES)?;
    enum_field(runtime, "xyRelease", "runtimeConfig", XY_RELEASES)?;
    unsigned_field(
        runtime,
        "audioOutputBufferFrames",
        "runtimeConfig",
        64,
        2048,
    )?;
    if let Some(value) = runtime.get("audioOutputBufferFrames") {
        if !matches!(value.as_u64(), Some(64 | 128 | 256 | 512 | 1024 | 2048)) {
            return Err("runtimeConfig.audioOutputBufferFrames is unsupported".into());
        }
    }
    instruments::validate_sound(runtime)?;
    modulation::validate_global_modulation(runtime)?;
    layers::validate_layers(runtime)?;
    validate_transport(runtime)?;
    validate_sparks(runtime)?;
    instruments::validate_instruments(runtime)?;
    mixer_fx::validate_mixer(runtime)?;
    mapping_bindings::validate_bindings(runtime)?;
    device_io::validate_midi(runtime)?;
    device_io::validate_audio_outputs(runtime)?;
    device_io::validate_usb(runtime)?;
    device_io::validate_hdmi(runtime)?;
    device_io::validate_recording(runtime)
}

pub(super) fn validate_system(root: &Map<String, Value>) -> Result<(), String> {
    if let Some(system) = object_field(root, "system", "configuration")? {
        enum_field(system, "sparksMode", "configuration.system", SPARKS_MODES)?;
    }
    Ok(())
}

fn validate_transport(runtime: &Map<String, Value>) -> Result<(), String> {
    number_field(runtime, "bpm", "runtimeConfig", 40.0, 240.0)?;
    if let Some(transport) = object_field(runtime, "transport", "runtimeConfig")? {
        number_field(transport, "bpm", "runtimeConfig.transport", 40.0, 240.0)?;
        unsigned_field(transport, "swingPct", "runtimeConfig.transport", 0, 75)?;
    }
    unsigned_field(runtime, "swingPct", "runtimeConfig", 0, 75)
}

fn validate_sparks(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(sparks) = object_field(runtime, "sparksFx", "runtimeConfig")? else {
        return Ok(());
    };
    if let Some(selected) = sparks.get("selected") {
        validate_sparks_config(selected, "runtimeConfig.sparksFx.selected")?;
    }
    if let Some(assignments) =
        super::canonical::array_field(sparks, "assignments", "runtimeConfig.sparksFx", usize::MAX)?
    {
        for (index, value) in assignments.iter().enumerate() {
            let path = format!("runtimeConfig.sparksFx.assignments[{index}]");
            let assignment = object_value(value, &path)?;
            unsigned_field(assignment, "x", &path, 0, 7)?;
            unsigned_field(assignment, "y", &path, 0, 7)?;
            let config = object_field(assignment, "config", &path)?
                .ok_or_else(|| format!("{path}.config must be an object"))?;
            validate_sparks_config(&Value::Object(config.clone()), &format!("{path}.config"))?;
        }
    }
    Ok(())
}

fn validate_sparks_config(value: &Value, path: &str) -> Result<(), String> {
    let object = object_value(value, path)?;
    enum_field(object, "fxType", path, SPARKS_FX_TYPES)?;
    enum_field(object, "targetKey", path, SPARKS_TARGETS)?;
    if let Some(params) = object_field(object, "params", path)? {
        let fx_type = object
            .get("fxType")
            .and_then(Value::as_str)
            .unwrap_or("none");
        for (key, value) in params {
            let Some((min, max)) = sparks_param_range(fx_type, key) else {
                return Err(format!("{path}.params.{key} is not valid for {fx_type}"));
            };
            signed_value(value, &format!("{path}.params.{key}"), min, max)?;
        }
    }
    Ok(())
}

fn sparks_param_range(fx_type: &str, key: &str) -> Option<(i64, i64)> {
    match (fx_type, key) {
        ("stutter", "rateHz") => Some((1, 32)),
        ("stutter", "depthPct") => Some((0, 100)),
        ("freeze", "releaseMs") => Some((10, 5000)),
        ("freeze", "mixPct") => Some((0, 100)),
        ("filter_sweep", "cutoffPct" | "resonancePct") => Some((0, 100)),
        ("filter_sweep", "sweepInMs" | "sweepOutMs") => Some((10, 3000)),
        ("pitch_shift", "semitones") => Some((-24, 24)),
        ("pitch_shift", "cents") => Some((-100, 100)),
        ("pitch_shift", "mixPct") => Some((0, 100)),
        _ => None,
    }
}

const VELOCITY_CURVES: &[&str] = &["linear", "soft", "hard"];
const VOICE_MODES: &[&str] = &[
    "none",
    "fixed12",
    "fixed16",
    "auto-soft",
    "auto-balanced",
    "auto-hard",
];
const DISPLAY_MODES: &[&str] = &["bar", "numbers", "bar+numbers"];
const SPARKS_MODES: &[&str] = &["mix", "pan", "fx", "trigger-gate", "transpose", "xy"];
const XY_RELEASES: &[&str] = &["sample-hold", "reset-center"];
const SPARKS_FX_TYPES: &[&str] = &["none", "stutter", "freeze", "filter_sweep", "pitch_shift"];
const SPARKS_TARGETS: &[&str] = &[
    "master",
    "fx_bus_1",
    "fx_bus_2",
    "instrument_1",
    "instrument_2",
    "instrument_3",
    "instrument_4",
    "instrument_5",
    "instrument_6",
    "instrument_7",
    "instrument_8",
];
