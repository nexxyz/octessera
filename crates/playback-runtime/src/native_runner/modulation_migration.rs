use super::modulation_target::{classify_key, TargetMode};
use super::{json, Value, GLOBAL_LFO_COUNT};
use serde_json::Map;
use std::collections::BTreeSet;

pub(super) fn migrate_legacy_modulation(
    payload: &mut Value,
    current: &Value,
) -> Result<Option<String>, String> {
    let Some(runtime) = runtime_object_mut(payload) else {
        return Ok(None);
    };
    let current_runtime = current.get("runtimeConfig").unwrap_or(current);
    let mut reports = Vec::new();
    migrate_lfo_bank(runtime, current_runtime, &mut reports);
    migrate_xy(runtime, current_runtime, &mut reports);
    migrate_aux_bank(runtime, "auxBindings", &mut reports);
    migrate_aux_bank(runtime, "shiftAuxBindings", &mut reports);
    for layer in runtime
        .get_mut("layers")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(layer) = layer.as_object_mut() {
            layer.remove("linkLfo");
            layer.remove("xy");
        }
    }
    let disabled = normalize_exclusive_claims(runtime, true)?;
    if disabled > 0 {
        reports.push(format!("disabled {disabled} duplicate exclusive bindings"));
    }
    if reports.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "Migrated legacy modulation: {}",
            reports.join(", ")
        )))
    }
}

pub(super) fn validate_canonical_modulation(runtime: &Map<String, Value>) -> Result<(), String> {
    normalize_exclusive_claims_value(runtime, false)
}

pub(super) fn validate_canonical_lfo_bank_shape(payload: &Value) -> Result<(), String> {
    let runtime = payload.get("runtimeConfig").unwrap_or(payload);
    let Some(lfos) = runtime.get("linkLfos") else {
        return Ok(());
    };
    let Some(lfos) = lfos.as_array() else {
        return Err("runtimeConfig.linkLfos must be an array".into());
    };
    if lfos.len() != GLOBAL_LFO_COUNT {
        return Err("runtimeConfig.linkLfos must contain exactly eight slots".into());
    }
    Ok(())
}

fn runtime_object_mut(payload: &mut Value) -> Option<&mut Map<String, Value>> {
    if payload.get("runtimeConfig").is_some() {
        payload
            .get_mut("runtimeConfig")
            .and_then(Value::as_object_mut)
    } else {
        payload.as_object_mut()
    }
}

fn migrate_lfo_bank(runtime: &mut Map<String, Value>, current: &Value, reports: &mut Vec<String>) {
    let legacy_layers = runtime.get("layers").and_then(Value::as_array).cloned();
    if runtime.contains_key("linkLfos") {
        if legacy_layers
            .as_ref()
            .is_some_and(|layers| layers.iter().any(|layer| layer.get("linkLfo").is_some()))
        {
            reports.push("global LFO bank won per-layer conflicts".into());
        }
        return;
    }
    let mut bank = current
        .get("linkLfos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| vec![default_lfo(); GLOBAL_LFO_COUNT]);
    bank.resize_with(GLOBAL_LFO_COUNT, default_lfo);
    let mut migrated = 0;
    if let Some(layers) = legacy_layers {
        for (index, layer) in layers.into_iter().take(GLOBAL_LFO_COUNT).enumerate() {
            let Some(lfo) = layer.get("linkLfo").cloned() else {
                continue;
            };
            bank[index] = sanitized_lfo(lfo);
            migrated += 1;
        }
    }
    runtime.insert("linkLfos".into(), Value::Array(bank));
    if migrated > 0 {
        reports.push(format!("migrated {migrated} LFO slots by layer index"));
    }
}

fn migrate_xy(runtime: &mut Map<String, Value>, current: &Value, reports: &mut Vec<String>) {
    if runtime.contains_key("xy") {
        return;
    }
    let Some(layers) = runtime.get("layers").and_then(Value::as_array) else {
        if let Some(xy) = current.get("xy") {
            runtime.insert("xy".into(), xy.clone());
        }
        return;
    };
    let active = runtime
        .get("activeLayerIndex")
        .and_then(Value::as_u64)
        .or_else(|| current.get("activeLayerIndex").and_then(Value::as_u64))
        .unwrap_or(0) as usize;
    let selected = layers
        .get(active)
        .and_then(|layer| layer.get("xy"))
        .or_else(|| layers.iter().find_map(|layer| layer.get("xy")));
    if let Some(xy) = selected {
        runtime.insert("xy".into(), xy.clone());
        reports.push("migrated Play XY from the active or first present layer".into());
    } else if let Some(xy) = current.get("xy") {
        runtime.insert("xy".into(), xy.clone());
    }
}

fn migrate_aux_bank(runtime: &mut Map<String, Value>, key: &str, reports: &mut Vec<String>) {
    let Some(bindings) = runtime.get_mut(key).and_then(Value::as_object_mut) else {
        return;
    };
    let mut migrated = 0;
    for binding in bindings.values_mut() {
        let Some(turn_key) = binding
            .get("turnKey")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some((index, field)) = legacy_lfo_key(&turn_key) else {
            continue;
        };
        if let Some(object) = binding.as_object_mut() {
            object.insert("turnKey".into(), format!("linkLfos.{index}.{field}").into());
            migrated += 1;
        }
    }
    if migrated > 0 {
        reports.push(format!("migrated {migrated} {key} LFO keys"));
    }
}

fn legacy_lfo_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("layers.")?;
    let (index, field) = rest.split_once(".linkLfo.")?;
    let index = index.parse::<usize>().ok()?;
    (index < GLOBAL_LFO_COUNT).then_some((index, field))
}

fn default_lfo() -> Value {
    json!({"enabled": false, "target": null, "period": "1/1", "depthPct": 100})
}

fn sanitized_lfo(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("phasePulses");
    }
    value
}

fn normalize_exclusive_claims(
    runtime: &mut Map<String, Value>,
    normalize: bool,
) -> Result<usize, String> {
    let mut claimed = BTreeSet::new();
    let mut disabled = 0;
    let Some(layers) = runtime.get_mut("layers").and_then(Value::as_array_mut) else {
        return Ok(0);
    };
    for (layer_index, layer) in layers.iter_mut().enumerate() {
        let Some(param_mods) = layer.get_mut("paramMods").and_then(Value::as_object_mut) else {
            continue;
        };
        for axis in ["x", "y"] {
            let Some(bindings) = param_mods.get_mut(axis).and_then(Value::as_array_mut) else {
                continue;
            };
            for (slot, binding) in bindings.iter_mut().enumerate() {
                if claim_is_exclusive(binding, &mut claimed) {
                    if normalize {
                        *binding = Value::Null;
                        disabled += 1;
                    } else {
                        return Err(format!(
                            "runtimeConfig.layers[{layer_index}].paramMods.{axis}[{slot}] conflicts with an earlier exclusive binding"
                        ));
                    }
                }
            }
        }
    }
    if let Some(xy) = runtime.get_mut("xy").and_then(Value::as_object_mut) {
        for axis in ["x", "y"] {
            let Some(binding) = xy.get_mut(axis) else {
                continue;
            };
            if claim_is_exclusive(binding, &mut claimed) {
                if normalize {
                    *binding = Value::Null;
                    disabled += 1;
                } else {
                    return Err(format!(
                        "runtimeConfig.xy.{axis} conflicts with an earlier exclusive binding"
                    ));
                }
            }
        }
    }
    Ok(disabled)
}

fn normalize_exclusive_claims_value(
    runtime: &Map<String, Value>,
    normalize: bool,
) -> Result<(), String> {
    let mut value = Value::Object(runtime.clone());
    normalize_exclusive_claims(value.as_object_mut().expect("object"), normalize).map(|_| ())
}

fn claim_is_exclusive(value: &Value, claimed: &mut BTreeSet<String>) -> bool {
    let Some(key) = value.get("key").and_then(Value::as_str) else {
        return false;
    };
    if classify_key(key).is_some_and(|(_, mode, _)| mode == TargetMode::Discrete) {
        !claimed.insert(key.into())
    } else {
        false
    }
}
