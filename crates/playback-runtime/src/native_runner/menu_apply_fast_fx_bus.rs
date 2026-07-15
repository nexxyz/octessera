use crate::delay_timing::{
    nearest_note_for_ms, normalized_delay_params, note_ms, strip_delay_timing_metadata,
};
use crate::native_menu::{fx_bus_slot_children_for_key, global_fx_slot_children_for_key};
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
        self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            pan_pos: Some(usize::from(pan_pos)),
            volume_pct: None,
        });
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
        self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            pan_pos: None,
            volume_pct: Some(f32::from(volume_pct)),
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
        let sync_time_ms = matches!(param_path, "timeMode" | "timeNote").then(|| params.clone());
        let fx_type = fx_type.clone();
        let params = audio_params_for_fx(&fx_type, params);
        if let Some(sync_time_ms) = sync_time_ms {
            self.sync_delay_time_ms_menu_value(key, &sync_time_ms);
        }
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
            let next_label = fx_slot_group_label(slot_index + 1, slot_type);
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
        let next_bus_name = bus.name.clone();
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
            self.menu.replace_group_label_containing_direct_key(
                &format!("mixer.buses.{bus_index}.name"),
                &next_bus_label,
            );
            self.menu
                .set_text_value_for_key(&format!("mixer.buses.{bus_index}.name"), &next_bus_name);
        }
        self.mark_fast_autosave_dirty();
        self.queue_audio_command(RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
        });
        self.warn_if_bus_fx_over_budget();
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
        match set_json_leaf(params, param_path, value) {
            Some(true) => {}
            Some(false) => return true,
            None => return false,
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
        let next_label = fx_slot_group_label(slot_index + 1, slot);
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
        self.mark_fast_autosave_dirty();
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

    fn sync_delay_time_ms_menu_value(&mut self, key: &str, value: &Value) {
        let Some(time_ms) = value
            .get("timeMs")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
        else {
            return;
        };
        let time_ms_key = key
            .strip_suffix("timeNote")
            .or_else(|| key.strip_suffix("timeMode"))
            .map(|prefix| format!("{prefix}timeMs"));
        if let Some(time_ms_key) = time_ms_key {
            self.menu.set_number_value_for_key(&time_ms_key, time_ms);
        }
    }

    pub(super) fn current_menu_bpm(&self) -> u16 {
        crate::delay_timing::visible_bpm_u16(self.bpm)
    }
}

fn apply_fx_param_value(params: &mut Value, param_path: &str, value: Value, bpm: u16) -> bool {
    if param_path == "timeNote" {
        let Some(note) = value.as_str() else {
            return false;
        };
        let mut next = normalized_delay_params(params, bpm);
        set_json_leaf(&mut next, "timeMode", Value::from("note"));
        set_json_leaf(&mut next, "timeNote", Value::from(note));
        set_json_leaf(&mut next, "timeMs", Value::from(note_ms(note, bpm)));
        *params = next;
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
    if param_path == "timeMs" {
        let mut next = normalized_delay_params(params, bpm);
        set_json_leaf(&mut next, "timeMode", Value::from("ms"));
        set_json_leaf(&mut next, param_path, value);
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

fn fx_slot_group_label(slot_number: usize, slot_type: &str) -> String {
    format!("Slot {slot_number}: {}", fx_type_label(slot_type))
}

fn fx_type_label(slot_type: &str) -> String {
    match slot_type {
        "none" => "None".into(),
        "delay" => "Delay".into(),
        "duck" => "Duck".into(),
        "reverb" => "Reverb".into(),
        "tremolo" => "Tremolo".into(),
        "saturator" => "Saturator".into(),
        "distortion" => "Distortion".into(),
        "bitcrusher" => "Bitcrusher".into(),
        "vibrato" => "Vibrato".into(),
        "chorus" => "Chorus".into(),
        "flanger" => "Flanger".into(),
        "filter_lfo" => "Filter LFO".into(),
        "wah" => "Wah".into(),
        "auto_pan" => "Auto Pan".into(),
        "glitch" => "Glitch".into(),
        "compressor" => "Compressor".into(),
        "eq" => "EQ".into(),
        "vinyl" => "Vinyl".into(),
        _ => slot_type.into(),
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
