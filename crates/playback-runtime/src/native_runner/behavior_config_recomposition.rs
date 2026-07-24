use super::{NativeRunner, Value};

impl NativeRunner {
    pub(super) fn prepare_behavior_config_recomposition(
        &mut self,
        layer_index: usize,
        base_config: &Value,
        fields: &[String],
    ) -> Value {
        let mut state = std::mem::take(&mut self.modulation_process);
        let mut final_config = base_config.as_object().cloned().unwrap_or_default();
        for field in fields {
            let key = format!("layers.{layer_index}.worlds.behaviorConfig.{field}");
            let Some(base_value) = final_config.get(field).cloned() else {
                continue;
            };
            let held_value = state.composed_discrete_value(&key);
            state.set_discrete_base(&key, base_value);
            if let Some(held_value) = held_value {
                final_config.insert(field.clone(), held_value);
            }
        }
        self.modulation_process = state;
        Value::Object(final_config)
    }
}
