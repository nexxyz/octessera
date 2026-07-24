use crate::native_menu::NativeMenuValue;

use super::{NativeRunner, Value};

impl NativeRunner {
    pub(super) fn turn_generated_behavior_target(
        &mut self,
        key: &str,
        delta: i8,
    ) -> Result<Option<String>, String> {
        let Some(item) = self.generated_behavior_target_item(key) else {
            return Ok(None);
        };
        let value = match item.value {
            NativeMenuValue::Enum { options, selected } => {
                let Some(next) = turn_index(selected, options.len(), delta) else {
                    return Ok(None);
                };
                let Some(value) = options.get(next) else {
                    return Ok(None);
                };
                value.clone()
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
            _ => return Ok(None),
        };
        self.apply_generated_behavior_target_value(key, &value)?;
        Ok(Some(value))
    }

    fn apply_generated_behavior_target_value(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        if let Some(index) = parse_layer_algorithm_step_key(key) {
            let pulses = super::note_unit_to_pulses(value);
            let layer_step = self
                .transport
                .layer_algorithm_step_pulses
                .get_mut(index)
                .ok_or_else(|| {
                    format!("algorithm step layer is outside the supported range: {index}")
                })?;
            *layer_step = pulses;
            if index == self.active_layer_index {
                self.transport.algorithm_step_pulses = pulses;
            }
            self.menu.set_enum_value_for_key(key, value);
            self.clear_link_arp_state_for_layer(index);
            self.rebase_and_recompose_modulation_key(key);
            self.mark_fast_autosave_dirty();
            return Ok(());
        }
        let (index, field) = parse_layer_behavior_config_key(key)
            .ok_or_else(|| format!("invalid generated behavior target key `{key}`"))?;
        self.apply_layer_behavior_config_deltas(
            index,
            &[(field.into(), parse_generated_value(value))],
        )?;
        Ok(())
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
