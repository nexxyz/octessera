use crate::protocol::RuntimeAudioCommand;
use serde_json::Value;
use std::collections::BTreeMap;

use super::{derive_bus_name, fx_default_params, NativeRunner, PAN_POSITION_COUNT};

impl NativeRunner {
    pub(super) fn apply_fx_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if let Some(rest) = key.strip_prefix("mixer.buses.") {
            let (bus_index, rest) = parse_indexed_key(rest)?;
            if rest == "panPos" {
                return Some(self.fast_fx_bus_pan_key(bus_index, key));
            }
            let (slot_name, param_path) = rest.split_once(".params.")?;
            let slot_index = match slot_name {
                "slot1" => 0,
                "slot2" => 1,
                _ => return Some(false),
            };
            return Some(self.fast_fx_bus_param_key(bus_index, slot_index, param_path, key));
        }
        if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
            let (slot_index, rest) = parse_indexed_key(rest)?;
            let param_path = rest.strip_prefix("params.")?;
            return Some(self.fast_global_fx_param_key(slot_index, param_path, key));
        }
        None
    }

    fn fast_fx_bus_pan_key(&mut self, bus_index: usize, key: &str) -> bool {
        let Some(value) = self.menu.number_for_key(key) else {
            return false;
        };
        let Some(bus) = self.fx_buses.get_mut(bus_index) else {
            return false;
        };
        let pan_pos = value.clamp(0, i32::from(PAN_POSITION_COUNT - 1)) as u8;
        if bus.pan_pos == pan_pos {
            return true;
        }
        bus.pan_pos = pan_pos;
        self.mark_fast_autosave_dirty();
        self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            pan_pos: Some(usize::from(pan_pos)),
        });
        true
    }

    fn fast_fx_bus_param_key(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        param_path: &str,
        key: &str,
    ) -> bool {
        let value = self.menu_value_for_audio_param(key);
        let Some(bus) = self.fx_buses.get_mut(bus_index) else {
            return false;
        };
        let (fx_type, params) = if slot_index == 0 {
            (&bus.slot1_type, &mut bus.slot1_params)
        } else {
            (&bus.slot2_type, &mut bus.slot2_params)
        };
        if !set_json_leaf(params, param_path, value) {
            return false;
        }
        let fx_type = fx_type.clone();
        let params = value_object_to_map(params);
        self.mark_fast_autosave_dirty();
        self.queue_audio_command(RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
        });
        true
    }

    pub(super) fn fast_fx_bus_type_key(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        key: &str,
    ) -> bool {
        let Some(next_type) = self.menu.value_for_key(key) else {
            return false;
        };
        let Some(bus) = self.fx_buses.get_mut(bus_index) else {
            return false;
        };
        let changed = if slot_index == 0 {
            if bus.slot1_type == next_type {
                false
            } else {
                bus.slot1_type = next_type;
                bus.slot1_params = fx_default_params(&bus.slot1_type);
                true
            }
        } else if bus.slot2_type == next_type {
            false
        } else {
            bus.slot2_type = next_type;
            bus.slot2_params = fx_default_params(&bus.slot2_type);
            true
        };
        if !changed {
            return true;
        }
        if bus.auto_name {
            bus.name = derive_bus_name(bus);
        }
        let (fx_type, params) = if slot_index == 0 {
            (
                bus.slot1_type.clone(),
                value_object_to_map(&bus.slot1_params),
            )
        } else {
            (
                bus.slot2_type.clone(),
                value_object_to_map(&bus.slot2_params),
            )
        };
        self.mark_fast_autosave_dirty();
        self.menu.rebuild(self.menu_config());
        self.queue_audio_command(RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
        });
        true
    }

    fn fast_global_fx_param_key(&mut self, slot_index: usize, param_path: &str, key: &str) -> bool {
        let value = self.menu_value_for_audio_param(key);
        let Some(fx_type) = self.global_fx_slots.get(slot_index).cloned() else {
            return false;
        };
        let Some(params) = self.global_fx_params.get_mut(slot_index) else {
            return false;
        };
        if !set_json_leaf(params, param_path, value) {
            return false;
        }
        let params = value_object_to_map(params);
        self.mark_fast_autosave_dirty();
        self.queue_audio_command(RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
        });
        true
    }

    pub(super) fn fast_global_fx_type_key(&mut self, slot_index: usize, key: &str) -> bool {
        let Some(next_type) = self.menu.value_for_key(key) else {
            return false;
        };
        let Some(slot) = self.global_fx_slots.get_mut(slot_index) else {
            return false;
        };
        if *slot == next_type {
            return true;
        }
        *slot = next_type;
        let Some(params) = self.global_fx_params.get_mut(slot_index) else {
            return false;
        };
        *params = fx_default_params(slot);
        let fx_type = slot.clone();
        let params = value_object_to_map(params);
        self.mark_fast_autosave_dirty();
        self.menu.rebuild(self.menu_config());
        self.queue_audio_command(RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
        });
        true
    }

    fn menu_value_for_audio_param(&self, key: &str) -> Value {
        if let Some(number) = self.menu.number_for_key(key) {
            let param = key.rsplit('.').next().unwrap_or(key);
            let scale = fx_param_scale(param);
            return if (scale - 1.0).abs() < f64::EPSILON {
                Value::from(number)
            } else {
                Value::from(f64::from(number) / scale)
            };
        }
        self.menu
            .value_for_key(key)
            .map(Value::from)
            .unwrap_or(Value::Null)
    }
}

fn fx_param_scale(param: &str) -> f64 {
    match param {
        "threshold" | "feedback" | "rateHz" | "clip" | "q" | "damp" | "midQ" => 100.0,
        "drive" | "depthMs" | "baseMs" => 10.0,
        "decay" => 1000.0,
        "thresholdDb" | "ratio" | "makeupDb" | "lowGainDb" | "midGainDb" | "highGainDb" => 2.0,
        _ => 1.0,
    }
}

fn set_json_leaf(target: &mut Value, path: &str, value: Value) -> bool {
    let Some(object) = target.as_object_mut() else {
        return false;
    };
    let changed = object.get(path) != Some(&value);
    if changed {
        object.insert(path.to_string(), value);
    }
    changed
}

fn value_object_to_map(value: &Value) -> BTreeMap<String, Value> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_indexed_key(value: &str) -> Option<(usize, &str)> {
    let (index, suffix) = value.split_once('.')?;
    Some((index.parse().ok()?, suffix))
}
