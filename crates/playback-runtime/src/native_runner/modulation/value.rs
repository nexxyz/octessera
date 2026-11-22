use super::super::{fx_default_params, json, NativeParamBinding, Value};

pub(super) fn apply_string_value(target: &mut String, value: Value, allowed: &[&str]) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    if !allowed.is_empty() && !allowed.contains(&value) {
        return false;
    }
    if target != value {
        *target = value.into();
        return true;
    }
    false
}

pub(super) fn apply_bool_value(target: &mut bool, value: Value) -> bool {
    let Some(value) = value.as_bool() else {
        return false;
    };
    if *target != value {
        *target = value;
        return true;
    }
    false
}

pub(super) fn apply_u8_value(target: &mut u8, value: Value, max: u8) -> bool {
    let Some(value) = value.as_f64() else {
        return false;
    };
    let value = value.round().clamp(0.0, f64::from(max)) as u8;
    if *target != value {
        *target = value;
        return true;
    }
    false
}

pub(super) fn apply_u8_enum_value(target: &mut u8, value: Value, max: u8) -> bool {
    let Some(value) = value.as_str().and_then(|value| value.parse::<u8>().ok()) else {
        return false;
    };
    let value = value.clamp(1, max);
    if *target != value {
        *target = value;
        return true;
    }
    false
}

pub(super) fn apply_i32_value(target: &mut i32, value: Value, min: i32, max: i32) -> bool {
    let Some(value) = value.as_f64() else {
        return false;
    };
    let value = (value.round() as i32).clamp(min, max);
    if *target != value {
        *target = value;
        return true;
    }
    false
}

pub(super) fn apply_fx_slot_type_value(
    slot_type: &mut String,
    params: &mut Value,
    value: Value,
    is_valid: fn(&str) -> bool,
) -> bool {
    let Some(value) = value.as_str().filter(|value| is_valid(value)) else {
        return false;
    };
    if slot_type != value {
        *slot_type = value.into();
        *params = fx_default_params(value);
        return true;
    }
    false
}

pub(super) fn apply_fx_param_binding_value(
    fx_type: &str,
    params: &mut Value,
    key: &str,
    value: Value,
) -> bool {
    let mut map = params.as_object().cloned().unwrap_or_default();
    let next = if matches!(key, "source" | "sourceTap") {
        value
            .as_str()
            .filter(|value| {
                super::super::config_schema_validation::is_valid_fx_string_param(
                    fx_type, key, value,
                )
            })
            .map(|value| json!(value))
    } else if let Some(spec) = super::super::fx_param_specs::fx_param_spec(fx_type, key) {
        value.as_f64().map(|value| {
            fx_param_storage_value(key, value.clamp(f64::from(spec.min), f64::from(spec.max)))
        })
    } else {
        None
    };
    let Some(next) = next else {
        return false;
    };
    if map.get(key) != Some(&next) {
        map.insert(key.into(), next);
        *params = Value::Object(map);
        return true;
    }
    false
}

pub(super) fn fx_param_storage_value(key: &str, value: f64) -> Value {
    super::super::fx_param_codec::display_to_storage(key, value)
}

pub(super) fn axis_norm(index: usize, size: usize, invert: bool) -> f32 {
    let norm = index.min(size.saturating_sub(1)) as f32 / size.saturating_sub(1).max(1) as f32;
    if invert {
        1.0 - norm
    } else {
        norm
    }
}

pub(in crate::native_runner) fn quantize_binding_value(
    norm: f32,
    binding: &NativeParamBinding,
) -> Value {
    let norm = norm.clamp(0.0, 1.0);
    if binding.kind == "enum" && !binding.options.is_empty() {
        let index = (norm * (binding.options.len().saturating_sub(1)) as f32).round() as usize;
        return json!(binding.options[index.min(binding.options.len() - 1)]);
    }
    if binding.kind == "bool" {
        return json!(norm >= 0.5);
    }
    let min = binding.min.unwrap_or(0.0);
    let max = binding.max.unwrap_or(127.0);
    let (min, max) = effective_numeric_range(binding, min, max);
    let step = binding.step.unwrap_or(1.0);
    let raw = min + f64::from(norm) * (max - min);
    let stepped = if step > 0.0 {
        (raw / step).round() * step
    } else {
        raw
    };
    json!(stepped.clamp(min, max))
}

/// Where `value` sits in `binding`'s range, `0.0` at Range Min and `1.0` at Range Max.
pub(in crate::native_runner) fn binding_normalized(
    value: f64,
    binding: &NativeParamBinding,
) -> f32 {
    let (min, max) = effective_numeric_range(
        binding,
        binding.min.unwrap_or(0.0),
        binding.max.unwrap_or(127.0),
    );
    if max <= min {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
}

fn effective_numeric_range(
    binding: &NativeParamBinding,
    target_min: f64,
    target_max: f64,
) -> (f64, f64) {
    let low = target_min.min(target_max);
    let high = target_min.max(target_max);
    let min = binding.user_min.unwrap_or(target_min).clamp(low, high);
    let max = binding.user_max.unwrap_or(target_max).clamp(low, high);
    if min <= max {
        (min, max)
    } else {
        (max, min)
    }
}

#[cfg(test)]
mod tests {
    use super::apply_fx_param_binding_value;
    use serde_json::json;

    #[test]
    fn a_binding_driven_past_clip_s_top_stores_the_top() {
        let mut params = json!({ "drive": 2.5, "clip": 0.6, "mixPct": 100 });
        assert!(apply_fx_param_binding_value(
            "distortion",
            &mut params,
            "clip",
            json!(200.0)
        ));
        assert_eq!(params["clip"], json!(1.0));
    }
}
