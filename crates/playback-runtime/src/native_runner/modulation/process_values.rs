use super::super::{NativeParamBinding, NativeRunner, Value};

pub(super) fn canonical_binding_range(binding: &NativeParamBinding) -> Option<(f64, f64)> {
    let min = binding.min?;
    let max = binding.max?;
    Some((min.min(max), min.max(max)))
}

pub(super) fn user_binding_range(binding: &NativeParamBinding) -> Option<(f64, f64)> {
    let min = binding.user_min.or(binding.min)?;
    let max = binding.user_max.or(binding.max)?;
    Some((min.min(max), min.max(max)))
}

pub(super) fn canonical_clamp(min: Option<f64>, max: Option<f64>, value: f64) -> f64 {
    match (min, max) {
        (Some(min), Some(max)) => value.clamp(min.min(max), min.max(max)),
        _ => value,
    }
}

pub(in crate::native_runner) fn persistent_base_value(
    runner: &NativeRunner,
    binding: &NativeParamBinding,
) -> f64 {
    super::process_audio::audio_base_value(runner, &binding.key)
        .or_else(|| {
            super::keys::parse_layer_behavior_config_binding_key(&binding.key).and_then(
                |(index, field)| {
                    runner
                        .layer_behavior_config(index)
                        .get(field)
                        .and_then(Value::as_f64)
                },
            )
        })
        .or_else(|| runner.menu.number_for_key(&binding.key).map(f64::from))
        .or_else(|| canonical_binding_range(binding).map(|(min, _)| min))
        .unwrap_or(0.0)
}

pub(super) fn current_discrete_value(runner: &NativeRunner, binding: &NativeParamBinding) -> Value {
    if binding.kind == "number" {
        return runner
            .menu
            .number_for_key(&binding.key)
            .map(Value::from)
            .unwrap_or(Value::Null);
    }
    runner
        .menu
        .value_for_key(&binding.key)
        .map(|value| {
            if binding.kind == "bool" {
                Value::Bool(value == "true")
            } else {
                Value::String(value)
            }
        })
        .unwrap_or(Value::Null)
}

pub(super) fn numeric_sample(
    runner: &NativeRunner,
    binding: &NativeParamBinding,
    normalized: f64,
) -> f64 {
    let Some((min, max)) = user_binding_range(binding) else {
        return super::process_audio::audio_base_value(runner, &binding.key).unwrap_or(0.0);
    };
    let curve = runner.menu.curve_for_key(&binding.key);
    super::value::curved_binding_value(normalized, (min, max), curve).clamp(min, max)
}

/// The whole menu step a modulated value is saved as; the engine gets the value itself.
pub(super) fn stepped_binding_value(binding: &NativeParamBinding, value: f64) -> f64 {
    match binding.step.filter(|step| *step > 0.0) {
        Some(step) => canonical_clamp(binding.min, binding.max, (value / step).round() * step),
        None => value,
    }
}
