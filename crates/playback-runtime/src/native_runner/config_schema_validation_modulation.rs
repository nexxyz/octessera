use super::canonical::{
    array_field, bool_field, enum_field, object_field, object_value, unsigned_field,
};
use super::mapping_bindings::{validate_binding_field, validate_binding_value};
use super::Value;
use serde_json::Map;

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
    bool_field(xy, "xInvert", "runtimeConfig.xy")?;
    bool_field(xy, "yInvert", "runtimeConfig.xy")?;
    super::super::modulation_migration::validate_canonical_modulation(runtime)
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
