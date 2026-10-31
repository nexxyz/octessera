use crate::delay_timing::{
    nearest_note_for_ms, normalized_delay_params, note_ms, strip_delay_timing_metadata,
};
use crate::native_menu::{
    fx_bus_slot_children_for_key, global_fx_slot_children_for_key, FX_BUSES_LABEL, GLOBAL_FX_LABEL,
};
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
            if rest == "volume" {
                return Some(self.fast_fx_bus_volume_key(bus_index, key));
            }
            let (slot_name, param_path) = rest.split_once(".params.")?;
            let slot_index = match slot_name {
                "slot1" => 0,
                "slot2" => 1,
                "slot3" => 2,
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
        if !self.rebase_and_recompose_modulation_key(key) {
            self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                generation: 0,
                pan_pos: Some(usize::from(pan_pos)),
                volume_pct: None,
            });
        }
        true
    }

    fn fast_fx_bus_volume_key(&mut self, bus_index: usize, key: &str) -> bool {
        let Some(value) = self.menu.number_for_key(key) else {
            return false;
        };
        let Some(bus) = self.fx_buses.get_mut(bus_index) else {
            return false;
        };
        let volume_pct = value.clamp(0, 100) as u8;
        if bus.volume_pct == volume_pct {
            return true;
        }
        bus.volume_pct = volume_pct;
        self.mark_fast_autosave_dirty();
        if !self.rebase_and_recompose_modulation_key(key) {
            self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                generation: 0,
                pan_pos: None,
                volume_pct: Some(f32::from(volume_pct)),
            });
        }
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
        self.set_fx_bus_param(bus_index, slot_index, param_path, key, value)
    }

    pub(super) fn set_fx_bus_param(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        param_path: &str,
        key: &str,
        value: Value,
    ) -> bool {
        let bpm = self.current_menu_bpm();
        let Some(bus) = self.fx_buses.get_mut(bus_index) else {
            return false;
        };
        let (fx_type, params) = match slot_index {
            0 => (&bus.slot1_type, &mut bus.slot1_params),
            1 => (&bus.slot2_type, &mut bus.slot2_params),
            2 => (&bus.slot3_type, &mut bus.slot3_params),
            _ => return false,
        };
        let before = params.clone();
        if !apply_fx_param_value(params, param_path, value, bpm) {
            return false;
        }
        if before == *params {
            return true;
        }
        let time_mode_params = (param_path == "timeMode").then(|| params.clone());
        let fx_type = fx_type.clone();
        let params = audio_params_for_fx(&fx_type, params);
        if let Some(slot_params) = time_mode_params {
            self.show_delay_time_row(bus_index, slot_index, &slot_params);
        }
        self.mark_fast_autosave_dirty();
        if !self.rebase_and_recompose_modulation_key(key) {
            if let Some(param) = super::modulation_audio::realtime_safe_fx_param_id(param_path) {
                if let Some(value) = params.get(param_path).and_then(Value::as_f64) {
                    self.queue_audio_command(RuntimeAudioCommand::SetFxBusParam {
                        bus_index,
                        slot_index,
                        generation: 0,
                        param,
                        value: value as f32,
                    });
                }
            } else {
                self.queue_audio_command(RuntimeAudioCommand::SetFxBusSlot {
                    bus_index,
                    slot_index,
                    generation: 0,
                    fx_type,
                    params,
                });
            }
        }
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
        let previous_bus_label = format!("B{}: {}", bus_index + 1, bus.name);
        let (slot_type, slot_params) = match slot_index {
            0 => (&mut bus.slot1_type, &mut bus.slot1_params),
            1 => (&mut bus.slot2_type, &mut bus.slot2_params),
            2 => (&mut bus.slot3_type, &mut bus.slot3_params),
            _ => return false,
        };
        let changed = if *slot_type == next_type {
            false
        } else {
            *slot_type = next_type;
            *slot_params = fx_default_params(slot_type);
            let next_label = crate::native_menu::slot_group_label(slot_index + 1, slot_type);
            self.menu.replace_group_label_containing_direct_key(
                &format!("mixer.buses.{bus_index}.slot{}.type", slot_index + 1),
                &next_label,
            );
            true
        };
        if !changed {
            return true;
        }
        if bus.auto_name {
            bus.name = derive_bus_name(bus);
        }
        let next_bus_label = format!("B{}: {}", bus_index + 1, bus.name);
        let (fx_type, params) = match slot_index {
            0 => (
                bus.slot1_type.clone(),
                audio_params_for_fx(&bus.slot1_type, &bus.slot1_params),
            ),
            1 => (
                bus.slot2_type.clone(),
                audio_params_for_fx(&bus.slot2_type, &bus.slot2_params),
            ),
            2 => (
                bus.slot3_type.clone(),
                audio_params_for_fx(&bus.slot3_type, &bus.slot3_params),
            ),
            _ => return false,
        };
        let slot_key = format!("mixer.buses.{bus_index}.slot{}.type", slot_index + 1);
        let slot_prefix = format!("mixer.buses.{bus_index}.slot{}", slot_index + 1);
        let children = fx_bus_slot_children_for_key(
            &slot_prefix,
            &fx_type,
            &match slot_index {
                0 => self.fx_buses[bus_index].slot1_params.clone(),
                1 => self.fx_buses[bus_index].slot2_params.clone(),
                2 => self.fx_buses[bus_index].slot3_params.clone(),
                _ => Value::Null,
            },
            bus_index,
            self.current_menu_bpm(),
        );
        self.menu
            .replace_group_children_containing_direct_key(&slot_key, &children);
        if previous_bus_label != next_bus_label {
            self.show_fx_bus_name(bus_index);
        }
        self.menu.show_shape_group_bindings(FX_BUSES_LABEL);
        self.mark_fast_autosave_dirty();
        if !self.rebase_and_recompose_modulation_key(key) {
            self.queue_audio_command(RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                generation: 0,
                fx_type,
                params,
            });
        }
        true
    }

    pub(super) fn show_fx_bus_name(&mut self, bus_index: usize) {
        let Some(bus) = self.fx_buses.get(bus_index) else {
            return;
        };
        let name_key = format!("mixer.buses.{bus_index}.name");
        self.menu.replace_group_label_containing_direct_key(
            &name_key,
            &format!("B{}: {}", bus_index + 1, bus.name),
        );
        self.menu.set_text_value_for_key(&name_key, &bus.name);
    }

    fn fast_global_fx_param_key(&mut self, slot_index: usize, param_path: &str, key: &str) -> bool {
        let value = self.menu_value_for_audio_param(key);
        let Some(fx_type) = self.global_fx_slots.get(slot_index).cloned() else {
            return false;
        };
        let Some(params) = self.global_fx_params.get_mut(slot_index) else {
            return false;
        };
        match set_json_leaf(params, param_path, value) {
            Some(true) => {}
            Some(false) => return true,
            None => return false,
        }
        let params = value_object_to_map(params);
        self.mark_fast_autosave_dirty();
        if !self.rebase_and_recompose_modulation_key(key) {
            if let Some(param) = super::modulation_audio::realtime_safe_fx_param_id(param_path) {
                if let Some(value) = params.get(param_path).and_then(Value::as_f64) {
                    self.queue_audio_command(RuntimeAudioCommand::SetGlobalFxParam {
                        slot_index,
                        generation: 0,
                        param,
                        value: value as f32,
                    });
                }
            } else {
                self.queue_audio_command(RuntimeAudioCommand::SetGlobalFxSlot {
                    slot_index,
                    generation: 0,
                    fx_type,
                    params,
                });
            }
        }
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
        let next_label = crate::native_menu::slot_group_label(slot_index + 1, slot);
        self.menu.replace_group_label_containing_direct_key(
            &format!("mixer.master.slots.{slot_index}.type"),
            &next_label,
        );
        let fx_type = slot.clone();
        let params = value_object_to_map(params);
        let slot_key = format!("mixer.master.slots.{slot_index}.type");
        let slot_prefix = format!("mixer.master.slots.{slot_index}");
        let children = global_fx_slot_children_for_key(
            &slot_prefix,
            &fx_type,
            &self.global_fx_params[slot_index],
            self.current_menu_bpm(),
        );
        self.menu
            .replace_group_children_containing_direct_key(&slot_key, &children);
        self.menu.show_shape_group_bindings(GLOBAL_FX_LABEL);
        self.mark_fast_autosave_dirty();
        if !self.rebase_and_recompose_modulation_key(key) {
            self.queue_audio_command(RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                fx_type,
                generation: 0,
                params,
            });
        }
        true
    }

    fn menu_value_for_audio_param(&self, key: &str) -> Value {
        if let Some(number) = self.menu.number_for_key(key) {
            let param = key.rsplit('.').next().unwrap_or(key);
            return super::fx_param_codec::display_to_storage(param, f64::from(number));
        }
        self.menu
            .value_for_key(key)
            .map(Value::from)
            .unwrap_or(Value::Null)
    }

    /// Time Mode swaps the delay's time row in place: Time ms in ms mode, Time Note in note mode.
    fn show_delay_time_row(&mut self, bus_index: usize, slot_index: usize, params: &Value) {
        let slot_prefix = format!("mixer.buses.{bus_index}.slot{}", slot_index + 1);
        let children = fx_bus_slot_children_for_key(
            &slot_prefix,
            "delay",
            params,
            bus_index,
            self.current_menu_bpm(),
        );
        self.menu.replace_group_children_containing_direct_key(
            &format!("{slot_prefix}.type"),
            &children,
        );
    }

    pub(super) fn current_menu_bpm(&self) -> u16 {
        crate::delay_timing::visible_bpm_u16(self.transport.bpm)
    }
}

fn apply_fx_param_value(params: &mut Value, param_path: &str, value: Value, bpm: u16) -> bool {
    if param_path == "timeNote" || param_path == "timeMs" {
        if param_path == "timeNote" && !value.is_string() {
            return false;
        }
        let mut next = normalized_delay_params(params, bpm);
        set_json_leaf(&mut next, param_path, value);
        *params = normalized_delay_params(&next, bpm);
        return true;
    }
    if param_path == "timeMode" {
        let Some(mode) = value.as_str() else {
            return false;
        };
        let mut next = normalized_delay_params(params, bpm);
        if mode == "note" {
            let current_ms = next.get("timeMs").and_then(Value::as_i64).unwrap_or(250) as i32;
            let note = next
                .get("timeNote")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| nearest_note_for_ms(current_ms, bpm).to_string());
            set_json_leaf(&mut next, "timeMode", Value::from("note"));
            set_json_leaf(&mut next, "timeNote", Value::from(note.clone()));
            set_json_leaf(&mut next, "timeMs", Value::from(note_ms(&note, bpm)));
        } else {
            set_json_leaf(&mut next, "timeMode", Value::from("ms"));
        }
        *params = next;
        return true;
    }
    set_json_leaf(params, param_path, value).is_some()
}

pub(super) fn audio_params_for_fx(fx_type: &str, params: &Value) -> BTreeMap<String, Value> {
    if fx_type == "delay" {
        strip_delay_timing_metadata(params).into_iter().collect()
    } else {
        value_object_to_map(params)
    }
}

fn set_json_leaf(target: &mut Value, path: &str, value: Value) -> Option<bool> {
    let object = target.as_object_mut()?;
    let changed = object.get(path) != Some(&value);
    if changed {
        object.insert(path.to_string(), value);
    }
    Some(changed)
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
