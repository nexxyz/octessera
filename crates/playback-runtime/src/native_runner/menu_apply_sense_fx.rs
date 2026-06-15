use super::{
    apply_fx_param_menu_state, apply_value_lane_menu_state, dance_fx_param_default,
    dance_fx_param_keys, dance_fx_target_key, dance_fx_type, derive_bus_name, fx_default_params,
    json, set_bool_from_menu, set_i32_from_menu, set_string_from_menu, set_target_slot_from_menu,
    set_u8_enum_from_menu, set_u8_from_menu, NativeRunner, Value,
};

impl NativeRunner {
    pub(super) fn apply_sense_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.sense_parts.len() {
            let prefix = format!("parts.{index}.l2");
            let Some(part) = self.sense_parts.get_mut(index) else {
                continue;
            };
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_mode,
                &format!("{prefix}.scanMode"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_axis,
                &format!("{prefix}.scanAxis"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_unit,
                &format!("{prefix}.scanUnit"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_direction,
                &format!("{prefix}.scanDirection"),
            );
            changed |= set_u8_enum_from_menu(
                &self.menu,
                &mut part.scan_sections,
                &format!("{prefix}.scanSections"),
                8,
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.scanned_slot,
                &format!("{prefix}.mapping.scanned.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scanned_action,
                &format!("{prefix}.mapping.scanned.action"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.scanned_empty_slot,
                &format!("{prefix}.mapping.scanned_empty.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scanned_empty_action,
                &format!("{prefix}.mapping.scanned_empty.action"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.event_enabled,
                &format!("{prefix}.eventEnabled"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.state_notes_enabled,
                &format!("{prefix}.stateNotesEnabled"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.activate_slot,
                &format!("{prefix}.mapping.activate.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.activate_action,
                &format!("{prefix}.mapping.activate.action"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.stable_slot,
                &format!("{prefix}.mapping.stable.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.stable_action,
                &format!("{prefix}.mapping.stable.action"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.deactivate_slot,
                &format!("{prefix}.mapping.deactivate.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.deactivate_action,
                &format!("{prefix}.mapping.deactivate.action"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.trigger_probability_mode,
                &format!("{prefix}.triggerProbabilityMode"),
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.trigger_probability_low_pct,
                &format!("{prefix}.triggerProbabilityLowPct"),
                100,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.trigger_probability_high_pct,
                &format!("{prefix}.triggerProbabilityHighPct"),
                100,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.lowest_note,
                &format!("{prefix}.pitch.lowestNote"),
                127,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.highest_note,
                &format!("{prefix}.pitch.highestNote"),
                127,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.starting_note,
                &format!("{prefix}.pitch.startingNote"),
                127,
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scale,
                &format!("{prefix}.pitch.scale"),
            );
            changed |=
                set_string_from_menu(&self.menu, &mut part.root, &format!("{prefix}.pitch.root"));
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.out_of_range,
                &format!("{prefix}.pitch.outOfRange"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.x_pitch_enabled,
                &format!("{prefix}.x.pitch.enabled"),
            );
            changed |= set_i32_from_menu(
                &self.menu,
                &mut part.x_pitch_steps,
                &format!("{prefix}.x.pitch.steps"),
                -16,
                16,
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.x_pitch_restart_each_section,
                &format!("{prefix}.x.pitch.restartEachSection"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.y_pitch_enabled,
                &format!("{prefix}.y.pitch.enabled"),
            );
            changed |= set_i32_from_menu(
                &self.menu,
                &mut part.y_pitch_steps,
                &format!("{prefix}.y.pitch.steps"),
                -16,
                16,
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.y_pitch_restart_each_section,
                &format!("{prefix}.y.pitch.restartEachSection"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.x_velocity,
                &format!("{prefix}.x.velocity"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.x_filter_cutoff,
                &format!("{prefix}.x.filterCutoff"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.x_filter_resonance,
                &format!("{prefix}.x.filterResonance"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.y_velocity,
                &format!("{prefix}.y.velocity"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.y_filter_cutoff,
                &format!("{prefix}.y.filterCutoff"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.y_filter_resonance,
                &format!("{prefix}.y.filterResonance"),
            );
        }
        changed
    }

    pub(super) fn apply_fx_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.fx_buses.len() {
            let prefix = format!("mixer.buses.{index}");
            let Some(bus) = self.fx_buses.get_mut(index) else {
                continue;
            };
            let before = (
                bus.slot1_type.clone(),
                bus.slot2_type.clone(),
                bus.auto_name,
                bus.name.clone(),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut bus.slot1_type,
                &format!("{prefix}.slot1.type"),
            );
            if bus.slot1_type != before.0 {
                bus.slot1_params = fx_default_params(&bus.slot1_type);
            }
            changed |= set_string_from_menu(
                &self.menu,
                &mut bus.slot2_type,
                &format!("{prefix}.slot2.type"),
            );
            if bus.slot2_type != before.1 {
                bus.slot2_params = fx_default_params(&bus.slot2_type);
            }
            changed |= apply_fx_param_menu_state(
                &self.menu,
                &mut bus.slot1_params,
                &format!("{prefix}.slot1.params"),
            );
            changed |= apply_fx_param_menu_state(
                &self.menu,
                &mut bus.slot2_params,
                &format!("{prefix}.slot2.params"),
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut bus.pan_pos,
                &format!("{prefix}.panPos"),
                32,
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut bus.auto_name,
                &format!("{prefix}.autoName"),
            );
            if bus.auto_name
                && (bus.slot1_type != before.0 || bus.slot2_type != before.1 || !before.2)
            {
                bus.name = derive_bus_name(bus);
                changed = true;
            }
            if let Some(name) = self.menu.value_for_key(&format!("{prefix}.name")) {
                if name != before.3 {
                    bus.name = name;
                    bus.auto_name = false;
                    changed = true;
                }
            }
        }
        for index in 0..self.global_fx_slots.len() {
            let Some(slot) = self.global_fx_slots.get_mut(index) else {
                continue;
            };
            let before = slot.clone();
            changed |= set_string_from_menu(
                &self.menu,
                slot,
                &format!("mixer.master.slots.{index}.type"),
            );
            if *slot != before {
                if let Some(params) = self.global_fx_params.get_mut(index) {
                    *params = fx_default_params(slot);
                }
            }
            if let Some(params) = self.global_fx_params.get_mut(index) {
                changed |= apply_fx_param_menu_state(
                    &self.menu,
                    params,
                    &format!("mixer.master.slots.{index}.params"),
                );
            }
        }
        changed
    }

    pub(super) fn apply_dance_fx_menu_state(&mut self) -> bool {
        let before = self.dance_fx_selected.clone();
        let fx_type = self
            .menu
            .value_for_key("dance.fx.type")
            .unwrap_or_else(|| dance_fx_type(&before).into());
        let target = self
            .menu
            .value_for_key("dance.fx.target")
            .unwrap_or_else(|| dance_fx_target_key(&before).into());
        let mut params = serde_json::Map::new();
        for key in dance_fx_param_keys(&fx_type) {
            let default = dance_fx_param_default(&fx_type, key);
            let value = self
                .menu
                .number_for_key(&format!("dance.fx.params.{key}"))
                .unwrap_or_else(|| {
                    before
                        .get("params")
                        .and_then(|params| params.get(key))
                        .and_then(Value::as_i64)
                        .map(|value| value as i32)
                        .unwrap_or(default)
                });
            params.insert((*key).into(), Value::from(value));
        }
        self.dance_fx_selected =
            json!({ "fxType": fx_type, "targetKey": target, "params": params });
        self.dance_fx_selected != before
    }
}
