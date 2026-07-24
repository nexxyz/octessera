use crate::native_menu::NativeMenuValue;
use platform_core::{NativeBehavior, NativeLayerEngine, NativeLayerEngineConfig};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

use super::modulation::apply_sampler_assignments_for_instruments_routed;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn remember_layer_behavior_config(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
        config: Value,
    ) {
        if behavior_id == "none" || config.is_null() {
            return;
        }
        if let Some(history) = self.layer_behavior_config_history.get_mut(layer_index) {
            history.insert(behavior_id.into(), config);
        }
    }

    pub(super) fn remembered_layer_behavior_config(
        &self,
        layer_index: usize,
        behavior_id: &str,
    ) -> Value {
        if behavior_id == "none" {
            return Value::Null;
        }
        self.layer_behavior_config_history
            .get(layer_index)
            .and_then(|history| history.get(behavior_id))
            .cloned()
            .or_else(|| {
                (self.layer_behavior_ids.get(layer_index).map(String::as_str) == Some(behavior_id))
                    .then(|| self.layer_behavior_configs.get(layer_index))
                    .flatten()
                    .filter(|config| !config.is_null())
                    .cloned()
            })
            .unwrap_or(Value::Null)
    }

    pub(super) fn apply_layer_behavior_config_deltas(
        &mut self,
        layer_index: usize,
        deltas: &[(String, Value)],
    ) -> Result<bool, String> {
        self.apply_layer_behavior_config_deltas_inner(layer_index, deltas, true)
    }

    pub(super) fn apply_layer_behavior_config_deltas_from_modulation(
        &mut self,
        layer_index: usize,
        deltas: &[(String, Value)],
    ) -> Result<bool, String> {
        self.apply_layer_behavior_config_deltas_inner(layer_index, deltas, false)
    }

    fn apply_layer_behavior_config_deltas_inner(
        &mut self,
        layer_index: usize,
        deltas: &[(String, Value)],
        mark_autosave: bool,
    ) -> Result<bool, String> {
        let behavior_id = self
            .layer_behavior_ids
            .get(layer_index)
            .cloned()
            .unwrap_or_else(|| "none".into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let current = self.layer_behavior_config_ref(layer_index).clone();
        let mut normalized = BTreeMap::new();
        for (field, value) in deltas {
            let key = format!("layers.{layer_index}.worlds.behaviorConfig.{field}");
            let Some(item) = self.generated_behavior_target_item(&key) else {
                return Err(format!("behavior config key is not active: `{key}`"));
            };
            let next_value = normalize_menu_value(&item.value, value)
                .ok_or_else(|| format!("invalid value for behavior config key `{key}`"))?;
            normalized.insert(field.clone(), (next_value, item));
        }
        let editing_key = self
            .menu
            .state
            .editing
            .then(|| self.menu.current_key())
            .flatten();
        let changed = normalized.iter().any(|(field, (next_value, item))| {
            let key = format!("layers.{layer_index}.worlds.behaviorConfig.{field}");
            let baseline_item = if editing_key == Some(key.as_str()) {
                self.behavior_target_item_for_layer(layer_index, &key)
                    .unwrap_or_else(|| item.clone())
            } else {
                item.clone()
            };
            let current_value = current
                .get(field)
                .and_then(|current| normalize_menu_value(&item.value, current))
                .or_else(|| menu_item_value(&baseline_item.value));
            current_value.as_ref() != Some(next_value)
        });
        if !changed {
            return Ok(false);
        }

        let fields = normalized.keys().cloned().collect::<Vec<_>>();
        let mut next = current.as_object().cloned().unwrap_or_default();
        for (field, (value, _)) in normalized {
            next.insert(field, value);
        }
        let next_config = Value::Object(next);
        let engine_config = if mark_autosave {
            self.prepare_behavior_config_recomposition(layer_index, &next_config, &fields)
        } else {
            next_config.clone()
        };
        self.replace_layer_engine_with_config(layer_index, behavior, engine_config.clone(), None)?;
        self.set_layer_behavior_config(layer_index, behavior_id.as_str(), engine_config);
        if mark_autosave {
            for field in fields {
                let key = format!("layers.{layer_index}.worlds.behaviorConfig.{field}");
                if let Some(value) = next_config.get(field).cloned() {
                    self.set_behavior_config_menu_value(&key, &value);
                }
            }
        }
        if mark_autosave {
            self.mark_fast_autosave_dirty();
        }
        Ok(true)
    }

    pub(super) fn apply_behavior_config_menu_key(&mut self, key: &str) -> Result<bool, String> {
        let Some((layer_index, field)) = parse_behavior_config_key(key) else {
            return Err(format!("invalid behavior config key: `{key}`"));
        };
        let Some(item) = self.generated_behavior_target_item(key) else {
            return Err(format!("missing behavior config menu item: `{key}`"));
        };
        let value = menu_value_for_item(&self.menu, key, &item.value)
            .ok_or_else(|| format!("behavior config key has no menu value: `{key}`"))?;
        self.apply_layer_behavior_config_deltas(layer_index, &[(field.into(), value)])
    }

    pub(super) fn replace_layer_engine_with_config(
        &mut self,
        layer_index: usize,
        behavior: NativeBehavior,
        behavior_config: Value,
        saved_state: Option<Value>,
    ) -> Result<(), String> {
        let profile = self.interpretation_profile_for_layer(layer_index);
        let mapping = self.mapping_config_for_layer(layer_index);
        let looper_step_index = (behavior.id() == "looper")
            .then(|| self.looper_step_index_for_layer(layer_index))
            .flatten();
        let state = saved_state.map(|mut state| {
            if let Some(step_index) = looper_step_index {
                if let Some(object) = state.as_object_mut() {
                    object.insert("stepIndex".into(), json!(step_index));
                }
            }
            let state = state_with_behavior_config(state, &behavior_config);
            if behavior.id() == "looper" {
                super::looper_config::looper_state_with_config(state, &behavior_config)
            } else {
                state
            }
        });
        let config = NativeLayerEngineConfig {
            behavior,
            behavior_config: behavior_config.clone(),
            interpretation_profile: profile.clone(),
            mapping_config: mapping.clone(),
            global_sound: self.global_sound.clone(),
            note_behaviors: self.note_behaviors.clone(),
            layer_index,
        };
        let next_engine = match state {
            Some(state) => NativeLayerEngine::from_serialized_state(config, state)?,
            None => NativeLayerEngine::new(config)?,
        };
        #[cfg(test)]
        {
            self.layer_behavior_rebuilds = self.layer_behavior_rebuilds.saturating_add(1);
        }

        self.drain_layer_engine_notes(layer_index)?;
        self.clear_layer_replacement_state(layer_index);
        if layer_index == self.active_layer_index {
            self.engine = next_engine;
            self.behavior = behavior;
            self.behavior_config = behavior_config;
            self.interpretation_profile = profile;
            self.mapping_config = mapping;
        } else if let Some(slot) = self.layer_engines.get_mut(layer_index) {
            *slot = Some(next_engine);
        }
        Ok(())
    }

    pub(super) fn replace_layer_engine_from_payload(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
        worlds: &Value,
    ) -> Result<(), String> {
        let behavior = platform_core::get_native_behavior(behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let behavior_config = worlds
            .get("behaviorConfig")
            .cloned()
            .unwrap_or_else(|| self.layer_behavior_config(layer_index));
        let save_grid_state = worlds
            .get("saveGridState")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let state = save_grid_state
            .then(|| {
                worlds
                    .get("savedState")
                    .filter(|value| !value.is_null())
                    .or_else(|| worlds.get("behaviorState").filter(|value| !value.is_null()))
                    .cloned()
            })
            .flatten();
        self.replace_layer_engine_with_config(
            layer_index,
            behavior,
            behavior_config.clone(),
            state,
        )?;
        self.set_layer_behavior_config(layer_index, behavior_id, behavior_config);
        Ok(())
    }

    pub(super) fn set_layer_behavior_config(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
        config: Value,
    ) {
        if let Some(target) = self.layer_behavior_configs.get_mut(layer_index) {
            *target = config.clone();
        }
        if layer_index == self.active_layer_index {
            self.behavior_config = config.clone();
        }
        self.remember_layer_behavior_config(layer_index, behavior_id, config);
    }

    pub(super) fn layer_behavior_config(&self, layer_index: usize) -> Value {
        if layer_index == self.active_layer_index {
            self.layer_behavior_configs
                .get(layer_index)
                .cloned()
                .unwrap_or_else(|| self.behavior_config.clone())
        } else {
            self.layer_behavior_configs
                .get(layer_index)
                .cloned()
                .unwrap_or(Value::Null)
        }
    }

    fn layer_behavior_config_ref(&self, layer_index: usize) -> &Value {
        self.layer_behavior_configs
            .get(layer_index)
            .unwrap_or(&self.behavior_config)
    }

    pub(super) fn reset_layer_behavior(&mut self, layer_index: usize) -> Result<(), String> {
        let behavior_id = self
            .layer_behavior_ids
            .get(layer_index)
            .cloned()
            .unwrap_or_else(|| "none".into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let config = self.layer_behavior_config(layer_index);
        self.replace_layer_engine_with_config(layer_index, behavior, config.clone(), None)?;
        self.set_layer_behavior_config(layer_index, &behavior_id, config);
        self.mark_fast_autosave_dirty();
        Ok(())
    }

    pub(super) fn drain_all_layer_engine_notes(&mut self) -> Result<(), String> {
        for layer_index in 0..self.layer_engines.len() {
            self.drain_layer_engine_notes(layer_index)?;
        }
        Ok(())
    }

    fn drain_layer_engine_notes(&mut self, layer_index: usize) -> Result<(), String> {
        let notes = if layer_index == self.active_layer_index {
            self.engine.drain_held_notes(usize::MAX)
        } else {
            self.layer_engines
                .get_mut(layer_index)
                .and_then(Option::as_mut)
                .map(|engine| engine.drain_held_notes(usize::MAX))
                .unwrap_or_default()
        };
        if notes.is_empty() {
            return Ok(());
        }
        let instruments = self.instruments.clone();
        let sense = self.pulses_layers.get(layer_index).cloned();
        let transpose_offset = self
            .sparks_transpose_offsets
            .get(layer_index)
            .copied()
            .unwrap_or(0);
        let routed = apply_sampler_assignments_for_instruments_routed(
            notes,
            &[],
            0,
            &instruments,
            sense.as_ref(),
            transpose_offset,
            self.sparks_transpose_active_notes.get_mut(layer_index),
        );
        self.pending_transpose_note_offs.extend(routed);
        Ok(())
    }

    fn clear_layer_replacement_state(&mut self, layer_index: usize) {
        self.clear_delayed_link_events_for_layer(layer_index);
        self.clear_link_arp_state_for_layer(layer_index);
        if let Some(active_notes) = self.sparks_transpose_active_notes.get_mut(layer_index) {
            active_notes.clear();
        }
    }

    fn looper_step_index_for_layer(&self, layer_index: usize) -> Option<usize> {
        let state = if layer_index == self.active_layer_index {
            self.engine.state()
        } else {
            self.layer_engines.get(layer_index)?.as_ref()?.state()
        };
        match state {
            platform_core::NativeBehaviorState::Looper(state) => Some(state.step_index),
            _ => None,
        }
    }

    pub(super) fn restore_behavior_config_menu_value(&mut self, key: &str) {
        let Some((layer_index, field)) = parse_behavior_config_key(key) else {
            return;
        };
        let Some(value) = self
            .layer_behavior_config(layer_index)
            .get(field)
            .cloned()
            .or_else(|| {
                self.serialized_state_for_layer(layer_index)
                    .ok()
                    .and_then(|state| state.get(field).cloned())
            })
        else {
            return;
        };
        self.set_behavior_config_menu_value(key, &value);
    }

    fn set_behavior_config_menu_value(&mut self, key: &str, value: &Value) {
        let Some(item) = self.menu.item_for_key(key) else {
            return;
        };
        match item.value {
            NativeMenuValue::Number { .. } => {
                if let Some(value) = value.as_i64().and_then(|value| i32::try_from(value).ok()) {
                    self.menu.set_number_value_for_key(key, value);
                }
            }
            NativeMenuValue::Bool { .. } => {
                if let Some(value) = value.as_bool() {
                    self.menu.set_bool_value_for_key(key, value);
                }
            }
            NativeMenuValue::Enum { .. } => {
                if let Some(value) = value.as_str() {
                    self.menu.set_enum_value_for_key(key, value);
                }
            }
            _ => {}
        }
    }
}

fn parse_behavior_config_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("layers.")?;
    let (index, field) = rest.split_once(".worlds.behaviorConfig.")?;
    Some((index.parse().ok()?, field))
}

fn menu_value_for_item(
    menu: &super::NativeMenuModel,
    key: &str,
    item: &NativeMenuValue,
) -> Option<Value> {
    match item {
        NativeMenuValue::Number { .. } => menu.number_for_key(key).map(Value::from),
        NativeMenuValue::Bool { .. } => menu.value_for_key(key).map(|value| json!(value == "true")),
        NativeMenuValue::Enum { .. } => menu.value_for_key(key).map(Value::String),
        _ => None,
    }
}

fn menu_item_value(item: &NativeMenuValue) -> Option<Value> {
    match item {
        NativeMenuValue::Number { value, .. } => Some(Value::from(*value)),
        NativeMenuValue::Bool { value } => Some(Value::Bool(*value)),
        NativeMenuValue::Enum {
            options, selected, ..
        } => options.get(*selected).cloned().map(Value::String),
        _ => None,
    }
}

fn normalize_menu_value(item: &NativeMenuValue, value: &Value) -> Option<Value> {
    match item {
        NativeMenuValue::Number { min, max, step, .. } => {
            let value = value.as_f64()?;
            let step = f64::from(*step).max(1.0);
            let value = ((value / step).round() * step).clamp(f64::from(*min), f64::from(*max));
            Some(if value.fract() == 0.0 {
                json!(value as i64)
            } else {
                json!(value)
            })
        }
        NativeMenuValue::Bool { .. } => value.as_bool().map(Value::Bool),
        NativeMenuValue::Enum { options, .. } => {
            let value = value.as_str()?;
            options
                .iter()
                .any(|option| option == value)
                .then(|| json!(value))
        }
        _ => None,
    }
}

fn state_with_behavior_config(state: Value, behavior_config: &Value) -> Value {
    let Value::Object(config) = behavior_config else {
        return state;
    };
    let Value::Object(mut state) = state else {
        return Value::Object(config.clone());
    };
    merge_object(&mut state, config);
    Value::Object(state)
}

fn merge_object(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        match (target.get_mut(key), value) {
            (Some(Value::Object(target)), Value::Object(overlay)) => merge_object(target, overlay),
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::state_with_behavior_config;
    use serde_json::json;

    #[test]
    fn behavior_config_overlays_saved_state_without_discarding_dynamic_fields() {
        assert_eq!(
            state_with_behavior_config(
                json!({"generation": 4, "population": 12, "rate": 1}),
                &json!({"rate": 3}),
            ),
            json!({"generation": 4, "population": 12, "rate": 3}),
        );
    }
}
