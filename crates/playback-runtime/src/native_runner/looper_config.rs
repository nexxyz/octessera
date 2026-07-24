use serde_json::{json, Value};

pub(super) fn effective_looper_mode(
    config: &Value,
    state: &platform_core::NativeBehaviorState,
) -> Option<String> {
    matches!(state, platform_core::NativeBehaviorState::Looper(_))
        .then(|| effective_mode(config, looper_state_mode(state)))
}

fn looper_state_mode(state: &platform_core::NativeBehaviorState) -> Option<&str> {
    match state {
        platform_core::NativeBehaviorState::Looper(state) => Some(state.mode.as_str()),
        _ => None,
    }
}

fn effective_mode(config: &Value, state_mode: Option<&str>) -> String {
    match config.get("mode").and_then(Value::as_str).or(state_mode) {
        Some("play") => "play".into(),
        _ => "overdub".into(),
    }
}

pub(super) fn looper_state_with_config(mut state: Value, config: &Value) -> Value {
    let state_mode = state.get("mode").and_then(Value::as_str);
    let mode = config
        .get("mode")
        .and_then(Value::as_str)
        .or(state_mode)
        .filter(|mode| matches!(*mode, "overdub" | "play"))
        .unwrap_or("overdub")
        .to_string();
    let length_steps = config
        .get("lengthSteps")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 64) as usize)
        .unwrap_or(16);
    let Some(object) = state.as_object_mut() else {
        return json!({ "mode": mode, "lengthSteps": length_steps });
    };
    object.insert("mode".into(), json!(mode));
    object.insert("lengthSteps".into(), json!(length_steps));
    let steps = object.entry("steps").or_insert_with(|| json!([]));
    if let Some(step_array) = steps.as_array_mut() {
        step_array.truncate(length_steps);
        while step_array.len() < length_steps {
            step_array.push(json!([]));
        }
    }
    if let Some(step_index) = object.get("stepIndex").and_then(Value::as_u64) {
        object.insert(
            "stepIndex".into(),
            json!(step_index as usize % length_steps),
        );
    }
    state
}
