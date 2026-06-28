use super::NativeRunner;
use serde_json::{json, Value};

impl NativeRunner {
    pub(super) fn rebuild_looper_engine_with_config_state(
        &mut self,
        state: Value,
    ) -> Result<(), String> {
        self.engine = platform_core::NativePartEngine::from_serialized_state(
            platform_core::NativePartEngineConfig {
                behavior: self.behavior,
                behavior_config: self.behavior_config.clone(),
                interpretation_profile: self.interpretation_profile.clone(),
                mapping_config: self.mapping_config.clone(),
                global_sound: self.global_sound.clone(),
                note_behaviors: self.note_behaviors.clone(),
                part_index: self.active_part_index,
            },
            looper_state_with_config(state, &self.behavior_config),
        )?;
        self.menu.rebuild(self.menu_config());
        Ok(())
    }
}

fn looper_state_with_config(mut state: Value, config: &Value) -> Value {
    let mode = config
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| *mode == "overdub")
        .unwrap_or("play");
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
