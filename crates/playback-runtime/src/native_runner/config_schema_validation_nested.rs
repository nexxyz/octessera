use super::{
    array_field, bool_field, enum_field, number_value, object_field, object_value, signed_field,
    signed_value, string_field, unsigned_field, Value,
};
use platform_core::{INSTRUMENT_COUNT, SAMPLE_SLOT_COUNT};
use serde_json::Map;

#[path = "config_schema_validation_audio.rs"]
mod audio;
#[path = "config_schema_validation_layers.rs"]
mod layers;
use super::super::modulation_migration::validate_canonical_modulation;
use audio::*;
use layers::*;

pub(super) fn validate_payload(root: &Map<String, Value>) -> Result<(), String> {
    let runtime = object_field(root, "runtimeConfig", "configuration")?
        .ok_or_else(|| "configuration runtimeConfig must be an object".to_string())?;
    walk_scalars(&Value::Object(root.clone()), "configuration")?;
    validate_runtime(runtime)?;
    if let Some(mapping) = root.get("mappingConfig") {
        validate_mapping_config(mapping)?;
    }
    if let Some(system) = object_field(root, "system", "configuration")? {
        enum_field(system, "sparksMode", "configuration.system", SPARKS_MODES)?;
    }
    Ok(())
}

fn validate_runtime(runtime: &Map<String, Value>) -> Result<(), String> {
    behavior_field(runtime, "activeBehavior", "runtimeConfig")?;
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
    validate_sound(runtime)?;
    validate_global_modulation(runtime)?;
    validate_layers(runtime)?;
    validate_transport(runtime)?;
    validate_sparks(runtime)?;
    validate_instruments(runtime)?;
    validate_mixer(runtime)?;
    validate_bindings(runtime)?;
    validate_midi(runtime)?;
    validate_usb(runtime)?;
    validate_hdmi(runtime)?;
    validate_recording(runtime)
}

fn validate_global_modulation(runtime: &Map<String, Value>) -> Result<(), String> {
    let lfos = array_field(runtime, "linkLfos", "runtimeConfig", 8)?
        .ok_or_else(|| "runtimeConfig.linkLfos must be present".to_string())?;
    if lfos.len() != 8 {
        return Err("runtimeConfig.linkLfos must contain exactly eight slots".into());
    }
    for (index, value) in lfos.iter().enumerate() {
        let path = format!("runtimeConfig.linkLfos[{index}]");
        let lfo = object_value(value, &path)?;
        bool_field(lfo, "enabled", &path)?;
        validate_binding_field(lfo, "target", &path)?;
        if let Some(target) = lfo.get("target").filter(|value| !value.is_null()) {
            let target = object_value(target, &format!("{path}.target"))?;
            if target.get("kind").and_then(Value::as_str) != Some("number") {
                return Err(format!("{path}.target must be numeric"));
            }
            let key = target
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{path}.target.key must be a string"))?;
            if !super::super::modulation_audio::is_live_link_lfo_target(key) {
                return Err(format!("{path}.target is not additive and live-safe"));
            }
        }
        enum_field(lfo, "period", &path, crate::timing_units::NOTE_UNIT_OPTIONS)?;
        unsigned_field(lfo, "depthPct", &path, 0, 100)?;
        if lfo.contains_key("phasePulses") {
            return Err(format!(
                "{path}.phasePulses is transient and cannot be serialized"
            ));
        }
    }
    let xy = object_field(runtime, "xy", "runtimeConfig")?
        .ok_or_else(|| "runtimeConfig.xy must be present".to_string())?;
    validate_binding_field(xy, "x", "runtimeConfig.xy")?;
    validate_binding_field(xy, "y", "runtimeConfig.xy")?;
    bool_field(xy, "xInvert", "runtimeConfig.xy")?;
    bool_field(xy, "yInvert", "runtimeConfig.xy")?;
    validate_canonical_modulation(runtime)
}

fn validate_sound(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(sound) = object_field(runtime, "sound", "runtimeConfig")? else {
        return Ok(());
    };
    unsigned_field(sound, "noteLengthMs", "runtimeConfig.sound", 30, 2000)?;
    enum_field(
        sound,
        "velocityCurve",
        "runtimeConfig.sound",
        VELOCITY_CURVES,
    )?;
    enum_field(
        sound,
        "voiceStealingMode",
        "runtimeConfig.sound",
        VOICE_MODES,
    )?;
    unsigned_field(
        sound,
        "audioOutputBufferFrames",
        "runtimeConfig.sound",
        64,
        2048,
    )?;
    if let Some(value) = sound.get("audioOutputBufferFrames") {
        if !matches!(value.as_u64(), Some(64 | 128 | 256 | 512 | 1024 | 2048)) {
            return Err("runtimeConfig.sound.audioOutputBufferFrames is unsupported".into());
        }
    }
    Ok(())
}

fn validate_bindings(runtime: &Map<String, Value>) -> Result<(), String> {
    for key in ["auxBindings", "shiftAuxBindings"] {
        let Some(bindings) = object_field(runtime, key, "runtimeConfig")? else {
            continue;
        };
        for (slot, value) in bindings {
            validate_aux_binding(value, &format!("runtimeConfig.{key}.{slot}"))?;
        }
    }
    Ok(())
}

fn validate_aux_binding(value: &Value, path: &str) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let binding = object_value(value, path)?;
    if let Some(turn_key) = binding.get("turnKey") {
        if let Some(turn_key) = turn_key.as_str() {
            if turn_key.starts_with("layers.") && turn_key.contains(".linkLfo.") {
                return Err(format!("{path}.turnKey uses a legacy per-layer LFO key"));
            }
            if !super::super::supported_aux_turn_key(turn_key) {
                return Err(format!("{path}.turnKey is unsupported"));
            }
        } else if !turn_key.is_null() {
            return Err(format!("{path}.turnKey must be a string or null"));
        }
    }
    if let Some(action) = binding.get("pressAction") {
        if action.is_null() {
            return Ok(());
        }
        let action_path = format!("{path}.pressAction");
        let action = object_value(action, &action_path)?;
        let kind = action
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{action_path}.kind must be a string"))?;
        match kind {
            "behavior_action" => string_field(action, "actionType", &action_path)?,
            "platform_effect" => string_field(action, "action", &action_path)?,
            "instrument_clone" | "instrument_reset" => unsigned_field(
                action,
                "slot",
                &action_path,
                0,
                (INSTRUMENT_COUNT - 1) as u64,
            )?,
            "reset_behavior" => {}
            _ => return Err(format!("{action_path}.kind has unknown value `{kind}`")),
        }
    }
    Ok(())
}

fn validate_binding_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        validate_binding_value(value, &format!("{path}.{key}"))?;
    }
    Ok(())
}

fn validate_binding_value(value: &Value, path: &str) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let binding = object_value(value, path)?;
    let key = binding
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{path}.key must be a string"))?;
    if !super::super::supported_param_binding_key(key) {
        return Err(format!("{path}.key is unsupported"));
    }
    enum_field(binding, "kind", path, &["number", "enum", "bool"])?;
    if let Some(label) = binding.get("label") {
        if !label.is_null() && !label.is_string() {
            return Err(format!("{path}.label must be a string or null"));
        }
    }
    for key in ["min", "max", "step", "userMin", "userMax"] {
        if let Some(value) = binding.get(key) {
            number_value(value, &format!("{path}.{key}"), f64::MIN, f64::MAX)?;
        }
    }
    if let Some(options) = array_field(binding, "options", path, usize::MAX)? {
        for (index, value) in options.iter().enumerate() {
            if !value.is_string() {
                return Err(format!("{path}.options[{index}] must be a string"));
            }
        }
    }
    bool_field(binding, "invert", path)
}

pub(super) fn validate_mapping_config(value: &Value) -> Result<(), String> {
    let object = object_value(value, "mappingConfig")?;
    for key in ["baseMidiNote", "startingMidiNote", "maxMidiNote"] {
        unsigned_field(object, key, "mappingConfig", 0, 127)?;
    }
    let base = object
        .get("baseMidiNote")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let starting = object
        .get("startingMidiNote")
        .and_then(Value::as_u64)
        .unwrap_or(base);
    let max = object
        .get("maxMidiNote")
        .and_then(Value::as_u64)
        .unwrap_or(127);
    if starting < base || starting > max || max < base {
        return Err("mappingConfig MIDI note range is invalid".into());
    }
    enum_field(object, "rangeMode", "mappingConfig", &["clamp", "wrap"])?;
    let scale = object
        .get("scale")
        .and_then(Value::as_array)
        .ok_or_else(|| "mappingConfig.scale must be a non-empty array".to_string())?;
    if scale.is_empty() {
        return Err("mappingConfig.scale must not be empty".into());
    }
    for (index, value) in scale.iter().enumerate() {
        unsigned_value_range(value, &format!("mappingConfig.scale[{index}]"), 0, 11)?;
    }
    for key in ["rowStepDegrees", "columnStepDegrees"] {
        signed_field(object, key, "mappingConfig", -16, 16)?;
    }
    for key in [
        "activate",
        "deactivate",
        "stable",
        "scanned",
        "scanned_empty",
    ] {
        let target = object_field(object, key, "mappingConfig")?
            .ok_or_else(|| format!("mappingConfig.{key} must be an object"))?;
        let path = format!("mappingConfig.{key}");
        enum_field(target, "action", &path, &["none", "note_on", "note_off"])?;
        unsigned_field(target, "channel", &path, 0, 15)?;
        unsigned_field(target, "velocity", &path, 1, 127)?;
        unsigned_field(target, "durationMs", &path, 1, 8000)?;
    }
    Ok(())
}

fn behavior_field(object: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        let behavior = value
            .as_str()
            .ok_or_else(|| format!("{path}.{key} must be a string"))?;
        if platform_core::get_native_behavior(behavior).is_none() {
            return Err(format!("{path}.{key} has unknown behavior `{behavior}`"));
        }
    }
    Ok(())
}

fn walk_scalars(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let next_path = format!("{path}.{key}");
                validate_scalar(key, value, &next_path)?;
                walk_scalars(value, &next_path)?;
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                walk_scalars(value, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_scalar(key: &str, value: &Value, path: &str) -> Result<(), String> {
    if matches!(key, "path" | "turnKey") && !matches!(value, Value::Null | Value::String(_)) {
        return Err(format!("{path} must be a string or null"));
    }
    match key {
        "revision" | "schemaVersion" => unsigned_value(value, path)?,
        "masterVolume" | "displayBrightness" | "gridBrightness" | "buttonBrightness" => {
            unsigned_range(value, path, 0, 100)?
        }
        "velocityScalePct" => unsigned_range(value, path, 0, 200)?,
        "screenSleepSeconds" | "dimTimerSeconds" => unsigned_range(value, path, 0, 600)?,
        "swingPct" => unsigned_range(value, path, 0, 75)?,
        "bpm" => number_value(value, path, 40.0, 240.0)?,
        "noteLengthMs" => signed_value(value, path, 10, 2000)?,
        "scanSections" => unsigned_range(value, path, 1, 8)?,
        "delaySteps" => unsigned_range(value, path, 0, 16)?,
        "retriggerCount" => unsigned_range(value, path, 0, 8)?,
        "depthPct" | "gainPct" | "velocitySensitivityPct" => unsigned_range(value, path, 0, 100)?,
        "channel" => unsigned_range(value, path, 0, 16)?,
        "durationMs" => unsigned_range(value, path, 10, 5000)?,
        "selectedSlot" => unsigned_range(value, path, 0, (SAMPLE_SLOT_COUNT - 1) as u64)?,
        "audioOutputBufferFrames" => unsigned_value(value, path)?,
        "enabled" | "autoName" | "eventEnabled" | "stateNotesEnabled" | "saveGridState"
        | "xInvert" | "yInvert" | "invert" | "showGridlines" => bool_value(value, path)?,
        "path" | "turnKey" => {}
        _ => {}
    }
    Ok(())
}

fn unsigned_value(value: &Value, path: &str) -> Result<(), String> {
    if value.as_u64().is_none() {
        return Err(format!("{path} must be an unsigned integer"));
    }
    Ok(())
}

fn unsigned_value_range(value: &Value, path: &str, min: u64, max: u64) -> Result<(), String> {
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{path} must be an unsigned integer"))?;
    if value < min || value > max {
        return Err(format!("{path} is outside the supported range"));
    }
    Ok(())
}

fn unsigned_range(value: &Value, path: &str, min: u64, max: u64) -> Result<(), String> {
    unsigned_value_range(value, path, min, max)
}

fn bool_value(value: &Value, path: &str) -> Result<(), String> {
    if !value.is_boolean() {
        return Err(format!("{path} must be a boolean"));
    }
    Ok(())
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
