use crate::native_menu::NativeMenuValue;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
            let key = format!("layers.{layer_index}.build.behaviorConfig.{field}");
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
            let key = format!("layers.{layer_index}.build.behaviorConfig.{field}");
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
        // A sequencer's pattern is drawn by hand, so its settings (pages, view, advance) keep it;
        // other behaviors re-initialise their world from the new settings.
        let saved_state = (behavior_id == "sequencer")
            .then(|| self.serialized_state_for_layer(layer_index).ok())
            .flatten()
            .filter(|state| !state.is_null());
        self.replace_layer_engine_with_config(
            layer_index,
            behavior,
            engine_config.clone(),
            saved_state,
        )?;
        self.set_layer_behavior_config(layer_index, behavior_id.as_str(), engine_config);
        if mark_autosave {
            for field in fields {
                let key = format!("layers.{layer_index}.build.behaviorConfig.{field}");
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
    let (index, field) = rest.split_once(".build.behaviorConfig.")?;
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
