use crate::native_menu::NativeMenuValue;

use super::{NativeRunner, Value};

impl NativeRunner {
    pub(super) fn turn_generated_behavior_target(
        &mut self,
        key: &str,
        delta: i8,
    ) -> Option<String> {
        let item = self.generated_behavior_target_item(key)?;
        let value = match item.value {
            NativeMenuValue::Enum { options, selected } => {
                let next = turn_index(selected, options.len(), delta)?;
                options.get(next)?.clone()
            }
            NativeMenuValue::Number {
                value,
                min,
                max,
                step,
            } => (value + i32::from(delta) * step)
                .clamp(min, max)
                .to_string(),
            NativeMenuValue::Bool { value } => (!value).to_string(),
            _ => return None,
        };
        self.apply_generated_behavior_target_value(key, &value)?;
        Some(value)
    }

    fn apply_generated_behavior_target_value(&mut self, key: &str, value: &str) -> Option<()> {
        if let Some(index) = parse_layer_algorithm_step_key(key) {
            let pulses = super::note_unit_to_pulses(value);
            let layer_step = self.layer_algorithm_step_pulses.get_mut(index)?;
            *layer_step = pulses;
            if index == self.active_layer_index {
                self.algorithm_step_pulses = pulses;
            }
            self.config_dirty = true;
            return Some(());
        }
        let (index, field) = parse_layer_behavior_config_key(key)?;
        let config = self.layer_behavior_configs.get_mut(index)?;
        let mut object = config.as_object().cloned().unwrap_or_default();
        object.insert(field.into(), parse_generated_value(value));
        *config = Value::Object(object.clone());
        if index == self.active_layer_index {
            self.behavior_config = Value::Object(object);
        }
        self.config_dirty = true;
        Some(())
    }
}

fn parse_generated_value(value: &str) -> Value {
    value
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| match value {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::String(value.into()),
        })
}

fn turn_index(selected: usize, len: usize, delta: i8) -> Option<usize> {
    (len > 0).then(|| (selected as i32 + i32::from(delta)).rem_euclid(len as i32) as usize)
}

fn parse_layer_algorithm_step_key(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("layers.")?;
    let (index, field) = rest.split_once('.')?;
    (field == "algorithmStep")
        .then(|| index.parse().ok())
        .flatten()
}

fn parse_layer_behavior_config_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("layers.")?;
    let (index, field) = rest.split_once(".worlds.behaviorConfig.")?;
    Some((index.parse().ok()?, field))
}
