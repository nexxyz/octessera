use super::canonical::{
    array_field, bool_field, enum_field, object_field, object_value, unsigned_field,
};
use super::mapping_bindings::{validate_binding_field, validate_binding_value};
use super::Value;
use serde_json::Map;
use std::collections::BTreeSet;

pub(super) fn validate_global_modulation(runtime: &Map<String, Value>) -> Result<(), String> {
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
    validate_smoothing_ms(xy, "runtimeConfig.xy")?;
    bool_field(xy, "xInvert", "runtimeConfig.xy")?;
    bool_field(xy, "yInvert", "runtimeConfig.xy")?;
    validate_canonical_modulation(runtime)
}

pub(crate) fn validate_canonical_lfo_bank_shape(payload: &Value) -> Result<(), String> {
    let runtime = payload.get("runtimeConfig").unwrap_or(payload);
    let Some(lfos) = runtime.get("linkLfos") else {
        return Ok(());
    };
    let Some(lfos) = lfos.as_array() else {
        return Err("runtimeConfig.linkLfos must be an array".into());
    };
    if lfos.len() != super::super::GLOBAL_LFO_COUNT {
        return Err("runtimeConfig.linkLfos must contain exactly eight slots".into());
    }
    Ok(())
}

fn validate_canonical_modulation(runtime: &Map<String, Value>) -> Result<(), String> {
    let mut claimed = BTreeSet::new();
    let Some(layers) = runtime.get("layers").and_then(Value::as_array) else {
        return Ok(());
    };
    for (layer_index, layer) in layers.iter().enumerate() {
        let Some(param_mods) = layer.get("paramMods").and_then(Value::as_object) else {
            continue;
        };
        for axis in ["x", "y"] {
            let Some(bindings) = param_mods.get(axis).and_then(Value::as_array) else {
                continue;
            };
            for (slot, binding) in bindings.iter().enumerate() {
                if claim_is_exclusive(binding, &mut claimed) {
                    return Err(format!(
                        "runtimeConfig.layers[{layer_index}].paramMods.{axis}[{slot}] conflicts with an earlier exclusive binding"
                    ));
                }
            }
        }
    }
    if let Some(xy) = runtime.get("xy").and_then(Value::as_object) {
        for axis in ["x", "y"] {
            let Some(binding) = xy.get(axis) else {
                continue;
            };
            if claim_is_exclusive(binding, &mut claimed) {
                return Err(format!(
                    "runtimeConfig.xy.{axis} conflicts with an earlier exclusive binding"
                ));
            }
        }
    }
    Ok(())
}

fn claim_is_exclusive(value: &Value, claimed: &mut BTreeSet<String>) -> bool {
    let Some(key) = value.get("key").and_then(Value::as_str) else {
        return false;
    };
    if super::super::modulation_target::classify_key(key)
        .is_some_and(|(_, mode, _)| mode == super::super::modulation_target::TargetMode::Discrete)
    {
        !claimed.insert(key.into())
    } else {
        false
    }
}

pub(super) fn validate_layer_modulation(
    layer: &Map<String, Value>,
    path: &str,
) -> Result<(), String> {
    if let Some(mods) = object_field(layer, "paramMods", path)? {
        validate_param_mods(mods, &format!("{path}.paramMods"))?;
    }
    if let Some(xy) = object_field(layer, "xy", path)? {
        validate_binding_field(xy, "x", &format!("{path}.xy"))?;
        validate_binding_field(xy, "y", &format!("{path}.xy"))?;
        bool_field(xy, "xInvert", &format!("{path}.xy"))?;
        bool_field(xy, "yInvert", &format!("{path}.xy"))?;
    }
    Ok(())
}

fn validate_param_mods(mods: &Map<String, Value>, path: &str) -> Result<(), String> {
    for axis in ["x", "y"] {
        if let Some(values) = array_field(mods, axis, path, 2)? {
            for (index, value) in values.iter().enumerate() {
                validate_binding_value(value, &format!("{path}.{axis}[{index}]"))?;
            }
        }
    }
    Ok(())
}

fn validate_smoothing_ms(xy: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(value) = xy.get("smoothingMs") else {
        return Ok(());
    };
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{path}.smoothingMs must be an unsigned integer"))?;
    if value != 0 && (!(10..=500).contains(&value) || !value.is_multiple_of(10)) {
        return Err(format!(
            "{path}.smoothingMs must be 0 or a multiple of 10 from 10 to 500"
        ));
    }
    Ok(())
}
