use super::NativeRunner;
use serde_json::{json, Value};

impl NativeRunner {
    pub(super) fn apply_looper_mode_config_fast(
        &mut self,
        next_config: &Value,
    ) -> Result<bool, String> {
        if self.behavior.id() != "looper" || !looper_mode_only_change(self, next_config) {
            return Ok(false);
        }
        self.behavior_config = next_config.clone();
        if let Some(config) = self.layer_behavior_configs.get_mut(self.active_layer_index) {
            *config = self.behavior_config.clone();
        }
        self.behavior_configs
            .insert(self.behavior.id().to_string(), self.behavior_config.clone());
        let current_state = self.engine_state();
        let mode = effective_mode(&self.behavior_config, current_state.mode().as_deref());
        let _ = self.engine.on_input(
            platform_core::DeviceInput::BehaviorAction(platform_core::BehaviorActionInput {
                action_type: format!("setMode:{mode}"),
            }),
            self.bpm as f32,
        )?;
        self.mark_fast_autosave_dirty();
        Ok(true)
    }

    pub(super) fn rebuild_looper_engine_with_config_state(
        &mut self,
        mut state: Value,
    ) -> Result<(), String> {
        if let (platform_core::NativeBehaviorState::Looper(current), Some(object)) =
            (self.engine_state(), state.as_object_mut())
        {
            object.insert("stepIndex".into(), json!(current.step_index));
        }
        self.engine = platform_core::NativeLayerEngine::from_serialized_state(
            platform_core::NativeLayerEngineConfig {
                behavior: self.behavior,
                behavior_config: self.behavior_config.clone(),
                interpretation_profile: self.interpretation_profile.clone(),
                mapping_config: self.mapping_config.clone(),
                global_sound: self.global_sound.clone(),
                note_behaviors: self.note_behaviors.clone(),
                layer_index: self.active_layer_index,
            },
            looper_state_with_config(state, &self.behavior_config),
        )?;
        Ok(())
    }
}

trait LooperEngineStateMode {
    fn mode(&self) -> Option<String>;
}

impl LooperEngineStateMode for platform_core::NativeBehaviorState {
    fn mode(&self) -> Option<String> {
        match self {
            platform_core::NativeBehaviorState::Looper(state) => Some(state.mode.clone()),
            _ => None,
        }
    }
}

fn looper_mode_only_change(runner: &NativeRunner, next_config: &Value) -> bool {
    let current_state = runner.engine_state();
    let current_mode = effective_mode(&runner.behavior_config, current_state.mode().as_deref());
    let next_mode = effective_mode(next_config, Some(&current_mode));
    if current_mode == next_mode {
        return false;
    }
    effective_length(&runner.behavior_config, &current_state)
        == effective_length(next_config, &current_state)
}

pub(super) fn effective_looper_mode(
    config: &Value,
    state: &platform_core::NativeBehaviorState,
) -> Option<String> {
    matches!(state, platform_core::NativeBehaviorState::Looper(_))
        .then(|| effective_mode(config, state.mode().as_deref()))
}

fn effective_mode(config: &Value, state_mode: Option<&str>) -> String {
    match config.get("mode").and_then(Value::as_str).or(state_mode) {
        Some("play") => "play".into(),
        _ => "overdub".into(),
    }
}

fn effective_length(config: &Value, state: &platform_core::NativeBehaviorState) -> usize {
    config
        .get("lengthSteps")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 64) as usize)
        .or(match state {
            platform_core::NativeBehaviorState::Looper(state) => Some(state.length_steps),
            _ => None,
        })
        .unwrap_or(16)
}

fn looper_state_with_config(mut state: Value, config: &Value) -> Value {
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
