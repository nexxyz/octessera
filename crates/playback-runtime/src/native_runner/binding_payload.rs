use super::modulation_keys::{
    parse_fx_bus_binding_key, parse_global_fx_binding_key, parse_pulses_binding_key,
};
use super::*;

pub(super) fn param_binding_payload(binding: Option<&NativeParamBinding>) -> Value {
    let Some(binding) = binding else {
        return Value::Null;
    };
    let mut value = serde_json::Map::new();
    value.insert("key".into(), json!(binding.key));
    if let Some(label) = &binding.label {
        value.insert("label".into(), json!(label));
    }
    value.insert("kind".into(), json!(binding.kind));
    if let Some(min) = binding.min {
        value.insert("min".into(), json!(min));
    }
    if let Some(max) = binding.max {
        value.insert("max".into(), json!(max));
    }
    if let Some(step) = binding.step {
        value.insert("step".into(), json!(step));
    }
    if let Some(user_min) = binding.user_min {
        value.insert("userMin".into(), json!(user_min));
    }
    if let Some(user_max) = binding.user_max {
        value.insert("userMax".into(), json!(user_max));
    }
    if !binding.options.is_empty() {
        value.insert("options".into(), json!(binding.options));
    }
    value.insert("invert".into(), json!(binding.invert));
    Value::Object(value)
}

pub(super) fn param_mods_from_payload(payload: &Value) -> NativeParamMods {
    NativeParamMods {
        x: param_axis_bindings_from_payload(payload.get("x")),
        y: param_axis_bindings_from_payload(payload.get("y")),
    }
}

pub(super) fn param_axis_bindings_from_payload(
    payload: Option<&Value>,
) -> Vec<Option<NativeParamBinding>> {
    let mut out = vec![None, None];
    if let Some(values) = payload.and_then(Value::as_array) {
        for (index, value) in values.iter().take(2).enumerate() {
            out[index] = param_binding_from_payload(value);
        }
    }
    out
}

pub(super) fn param_binding_from_payload(payload: &Value) -> Option<NativeParamBinding> {
    let key = payload.get("key")?.as_str()?.to_string();
    if !supported_param_binding_key(&key) {
        return None;
    }
    let kind = match payload.get("kind").and_then(Value::as_str) {
        Some("enum") => "enum",
        Some("bool") => "bool",
        _ => "number",
    }
    .to_string();
    let mut binding = NativeParamBinding {
        key,
        label: payload
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string),
        kind,
        min: payload.get("min").and_then(Value::as_f64),
        max: payload.get("max").and_then(Value::as_f64),
        step: payload.get("step").and_then(Value::as_f64),
        user_min: payload.get("userMin").and_then(Value::as_f64),
        user_max: payload.get("userMax").and_then(Value::as_f64),
        options: payload
            .get("options")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        invert: payload
            .get("invert")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    sanitize_binding_user_range(&mut binding);
    Some(binding)
}

pub(super) fn sanitize_binding_user_range(binding: &mut NativeParamBinding) {
    if binding.kind != "number" {
        binding.user_min = None;
        binding.user_max = None;
        return;
    }
    let Some(target_min) = binding.min else {
        return;
    };
    let Some(target_max) = binding.max else {
        return;
    };
    let low = target_min.min(target_max);
    let high = target_min.max(target_max);
    binding.user_min = binding.user_min.map(|value| value.clamp(low, high));
    binding.user_max = binding.user_max.map(|value| value.clamp(low, high));
    if let (Some(user_min), Some(user_max)) = (binding.user_min, binding.user_max) {
        if user_min > user_max {
            binding.user_min = Some(user_max);
            binding.user_max = Some(user_min);
        }
    }
}

pub(super) fn supported_param_binding_key(key: &str) -> bool {
    if key.contains(".linkLfo.") {
        return false;
    }
    if matches!(
        key,
        "sound.noteLengthMs" | "sound.velocityScalePct" | "sound.voiceStealingMode"
    ) || key.starts_with("layers.")
        && (key.ends_with(".algorithmStep") || key.contains(".worlds.behaviorConfig."))
    {
        return true;
    }
    if parse_pulses_binding_key(key).is_some()
        || parse_fx_bus_binding_key(key).is_some()
        || parse_global_fx_binding_key(key).is_some()
        || key.starts_with("sparks.fx.")
    {
        return true;
    }
    let Some((_, field)) = parse_instrument_binding_key(key) else {
        return false;
    };
    supported_instrument_binding_field(field)
}

fn supported_instrument_binding_field(field: &str) -> bool {
    matches!(
        field,
        "type"
            | "noteBehavior"
            | "mixer.route"
            | "mixer.volume"
            | "mixer.panPos"
            | "synth.osc1.waveform"
            | "synth.osc1.octave"
            | "synth.osc1.levelPct"
            | "synth.osc1.detuneCents"
            | "synth.osc1.pulseWidthPct"
            | "synth.osc2.waveform"
            | "synth.osc2.octave"
            | "synth.osc2.levelPct"
            | "synth.osc2.detuneCents"
            | "synth.osc2.pulseWidthPct"
            | "synth.amp.gainPct"
            | "synth.amp.velocitySensitivityPct"
            | "synth.ampEnv.attackMs"
            | "synth.ampEnv.decayMs"
            | "synth.ampEnv.sustainPct"
            | "synth.ampEnv.releaseMs"
            | "synth.filter.type"
            | "synth.filter.cutoffHz"
            | "synth.filter.resonance"
            | "synth.filter.envAmountPct"
            | "synth.filter.keyTrackingPct"
            | "synth.filterEnv.attackMs"
            | "synth.filterEnv.decayMs"
            | "synth.filterEnv.sustainPct"
            | "synth.filterEnv.releaseMs"
            | "sample.tuneSemis"
            | "sample.selectedSlot"
            | "sample.amp.gainPct"
            | "sample.amp.velocitySensitivityPct"
            | "sample.ampEnv.attackMs"
            | "sample.ampEnv.decayMs"
            | "sample.ampEnv.sustainPct"
            | "sample.ampEnv.releaseMs"
            | "sample.baseVelocity"
            | "sample.velocityLevelsEnabled"
            | "sample.velocityLevels.high"
            | "sample.velocityLevels.medium"
            | "sample.velocityLevels.low"
            | "sample.filter.type"
            | "sample.filter.cutoffHz"
            | "sample.filter.resonance"
            | "sample.filter.envAmountPct"
            | "sample.filter.keyTrackingPct"
            | "sample.filterEnv.attackMs"
            | "sample.filterEnv.decayMs"
            | "sample.filterEnv.sustainPct"
            | "sample.filterEnv.releaseMs"
            | "midi.enabled"
            | "midi.channel"
            | "midi.velocity"
            | "midi.durationMs"
    )
}

pub(super) fn supported_aux_turn_key(key: &str) -> bool {
    !key.is_empty()
        && !key.contains("..")
        && (supported_param_binding_key(key)
            || key.starts_with("layers.")
            || key.starts_with("linkLfos.")
            || key.starts_with("mixer.")
            || key.starts_with("transport.")
            || key.starts_with("sparks.")
            || key.starts_with("midi")
            || key.starts_with("hdmi.")
            || key.starts_with("usb.")
            || key.starts_with("recording.")
            || key.starts_with("screen")
            || key.ends_with("Brightness")
            || matches!(
                key,
                "masterVolume"
                    | "sound.audioOutputBufferFrames"
                    | "autoSaveDefault"
                    | "rollingBackups"
                    | "ghostCells"
                    | "inputEventsWhilePaused"
                    | "numericDisplayMode"
                    | "dimTimerSeconds"
                    | "screenSleepSeconds"
            ))
}
