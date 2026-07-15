use super::menu_apply_fx_state::{apply_fx_bus_menu_state, apply_global_fx_slot_menu_state};
use super::{
    apply_value_lane_menu_state, json, set_bool_from_menu, set_i32_from_menu, set_string_from_menu,
    set_target_slot_from_menu, set_u8_enum_from_menu, set_u8_from_menu, sparks_fx_param_default,
    sparks_fx_param_keys, sparks_fx_target_key, sparks_fx_type, NativePulsesLayer, NativeRunner,
    Value,
};
use platform_core::BUS_FX_WARNING_SLOT_COUNT;

impl NativeRunner {
    pub(super) fn apply_pulses_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.pulses_layers.len() {
            let prefix = format!("layers.{index}.pulses");
            let Some(layer) = self.pulses_layers.get_mut(index) else {
                continue;
            };
            changed |= apply_pulses_scan_and_mapping_menu_state(&self.menu, layer, &prefix);
            changed |= apply_pulses_probability_and_pitch_menu_state(&self.menu, layer, &prefix);
            changed |= apply_pulses_axis_menu_state(&self.menu, layer, &prefix, "x");
            changed |= apply_pulses_axis_menu_state(&self.menu, layer, &prefix, "y");
            changed |=
                apply_link_lfo_menu_state(&self.menu, layer, &format!("layers.{index}.linkLfo"));
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
            changed |= apply_fx_bus_menu_state(&self.menu, bus, &prefix);
        }
        for index in 0..self.global_fx_slots.len() {
            changed |= apply_global_fx_slot_menu_state(
                &self.menu,
                &mut self.global_fx_slots,
                &mut self.global_fx_params,
                index,
            );
        }
        let active_fx_slots = self.active_bus_fx_slot_count();
        if active_fx_slots > BUS_FX_WARNING_SLOT_COUNT {
            self.show_toast(format!(
                "FX budget warning ({}/{})",
                active_fx_slots, BUS_FX_WARNING_SLOT_COUNT
            ));
        }
        changed
    }

    pub(super) fn apply_sparks_fx_menu_state(&mut self) -> bool {
        let before = self.sparks_fx_selected.clone();
        let fx_type = self
            .menu
            .value_for_key("sparks.fx.type")
            .unwrap_or_else(|| sparks_fx_type(&before).into());
        let target = self
            .menu
            .value_for_key("sparks.fx.target")
            .unwrap_or_else(|| sparks_fx_target_key(&before).into());
        let mut params = serde_json::Map::new();
        for key in sparks_fx_param_keys(&fx_type) {
            let default = sparks_fx_param_default(&fx_type, key);
            let value = self
                .menu
                .number_for_key(&format!("sparks.fx.params.{key}"))
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
        self.sparks_fx_selected =
            json!({ "fxType": fx_type, "targetKey": target, "params": params });
        self.sparks_fx_selected != before
    }
}

pub(super) fn apply_pulses_scan_and_mapping_menu_state(
    menu: &crate::native_menu::NativeMenuModel,
    layer: &mut super::NativePulsesLayer,
    prefix: &str,
) -> bool {
    let mut changed = false;
    changed |= set_string_from_menu(menu, &mut layer.scan_mode, &format!("{prefix}.scanMode"));
    changed |= set_string_from_menu(menu, &mut layer.scan_axis, &format!("{prefix}.scanAxis"));
    changed |= set_string_from_menu(menu, &mut layer.scan_unit, &format!("{prefix}.scanUnit"));
    changed |= set_string_from_menu(
        menu,
        &mut layer.scan_direction,
        &format!("{prefix}.scanDirection"),
    );
    changed |= set_u8_enum_from_menu(
        menu,
        &mut layer.scan_sections,
        &format!("{prefix}.scanSections"),
        8,
    );
    changed |= set_target_slot_from_menu(
        menu,
        &mut layer.scanned_slot,
        &format!("{prefix}.mapping.scanned.slot"),
    );
    changed |= set_string_from_menu(
        menu,
        &mut layer.scanned_action,
        &format!("{prefix}.mapping.scanned.action"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.scanned_timing.delay_steps,
        &format!("{prefix}.mapping.scanned.delaySteps"),
        16,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.scanned_timing.retrigger_count,
        &format!("{prefix}.mapping.scanned.retriggerCount"),
        8,
    );
    changed |= set_target_slot_from_menu(
        menu,
        &mut layer.scanned_empty_slot,
        &format!("{prefix}.mapping.scanned_empty.slot"),
    );
    changed |= set_string_from_menu(
        menu,
        &mut layer.scanned_empty_action,
        &format!("{prefix}.mapping.scanned_empty.action"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.scanned_empty_timing.delay_steps,
        &format!("{prefix}.mapping.scanned_empty.delaySteps"),
        16,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.scanned_empty_timing.retrigger_count,
        &format!("{prefix}.mapping.scanned_empty.retriggerCount"),
        8,
    );
    changed |= set_bool_from_menu(
        menu,
        &mut layer.event_enabled,
        &format!("{prefix}.eventEnabled"),
    );
    changed |= set_bool_from_menu(
        menu,
        &mut layer.state_notes_enabled,
        &format!("{prefix}.stateNotesEnabled"),
    );
    changed |= set_target_slot_from_menu(
        menu,
        &mut layer.activate_slot,
        &format!("{prefix}.mapping.activate.slot"),
    );
    changed |= set_string_from_menu(
        menu,
        &mut layer.activate_action,
        &format!("{prefix}.mapping.activate.action"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.activate_timing.delay_steps,
        &format!("{prefix}.mapping.activate.delaySteps"),
        16,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.activate_timing.retrigger_count,
        &format!("{prefix}.mapping.activate.retriggerCount"),
        8,
    );
    changed |= set_target_slot_from_menu(
        menu,
        &mut layer.stable_slot,
        &format!("{prefix}.mapping.stable.slot"),
    );
    changed |= set_string_from_menu(
        menu,
        &mut layer.stable_action,
        &format!("{prefix}.mapping.stable.action"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.stable_timing.delay_steps,
        &format!("{prefix}.mapping.stable.delaySteps"),
        16,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.stable_timing.retrigger_count,
        &format!("{prefix}.mapping.stable.retriggerCount"),
        8,
    );
    changed |= set_target_slot_from_menu(
        menu,
        &mut layer.deactivate_slot,
        &format!("{prefix}.mapping.deactivate.slot"),
    );
    changed |= set_string_from_menu(
        menu,
        &mut layer.deactivate_action,
        &format!("{prefix}.mapping.deactivate.action"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.deactivate_timing.delay_steps,
        &format!("{prefix}.mapping.deactivate.delaySteps"),
        16,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.deactivate_timing.retrigger_count,
        &format!("{prefix}.mapping.deactivate.retriggerCount"),
        8,
    );
    changed
}

pub(super) fn apply_pulses_probability_and_pitch_menu_state(
    menu: &crate::native_menu::NativeMenuModel,
    layer: &mut super::NativePulsesLayer,
    prefix: &str,
) -> bool {
    let mut changed = false;
    changed |= set_string_from_menu(
        menu,
        &mut layer.trigger_probability_mode,
        &format!("{prefix}.triggerProbabilityMode"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.trigger_probability_low_pct,
        &format!("{prefix}.triggerProbabilityLowPct"),
        100,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.trigger_probability_high_pct,
        &format!("{prefix}.triggerProbabilityHighPct"),
        100,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.lowest_note,
        &format!("{prefix}.pitch.lowestNote"),
        127,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.highest_note,
        &format!("{prefix}.pitch.highestNote"),
        127,
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.starting_note,
        &format!("{prefix}.pitch.startingNote"),
        127,
    );
    changed |= set_string_from_menu(menu, &mut layer.scale, &format!("{prefix}.pitch.scale"));
    changed |= set_string_from_menu(menu, &mut layer.root, &format!("{prefix}.pitch.root"));
    changed |= set_string_from_menu(
        menu,
        &mut layer.out_of_range,
        &format!("{prefix}.pitch.outOfRange"),
    );
    changed
}

pub(super) fn apply_pulses_axis_menu_state(
    menu: &crate::native_menu::NativeMenuModel,
    layer: &mut super::NativePulsesLayer,
    prefix: &str,
    axis: &str,
) -> bool {
    let mut changed = false;
    if axis == "x" {
        changed |= set_bool_from_menu(
            menu,
            &mut layer.x_pitch_enabled,
            &format!("{prefix}.x.pitch.enabled"),
        );
        changed |= set_i32_from_menu(
            menu,
            &mut layer.x_pitch_steps,
            &format!("{prefix}.x.pitch.steps"),
            -16,
            16,
        );
        changed |= set_bool_from_menu(
            menu,
            &mut layer.x_pitch_restart_each_section,
            &format!("{prefix}.x.pitch.restartEachSection"),
        );
        changed |= apply_value_lane_menu_state(
            menu,
            &mut layer.x_velocity,
            &format!("{prefix}.x.velocity"),
        );
        changed |= apply_value_lane_menu_state(
            menu,
            &mut layer.x_filter_cutoff,
            &format!("{prefix}.x.filterCutoff"),
        );
        changed |= apply_value_lane_menu_state(
            menu,
            &mut layer.x_filter_resonance,
            &format!("{prefix}.x.filterResonance"),
        );
    } else {
        changed |= set_bool_from_menu(
            menu,
            &mut layer.y_pitch_enabled,
            &format!("{prefix}.y.pitch.enabled"),
        );
        changed |= set_i32_from_menu(
            menu,
            &mut layer.y_pitch_steps,
            &format!("{prefix}.y.pitch.steps"),
            -16,
            16,
        );
        changed |= set_bool_from_menu(
            menu,
            &mut layer.y_pitch_restart_each_section,
            &format!("{prefix}.y.pitch.restartEachSection"),
        );
        changed |= apply_value_lane_menu_state(
            menu,
            &mut layer.y_velocity,
            &format!("{prefix}.y.velocity"),
        );
        changed |= apply_value_lane_menu_state(
            menu,
            &mut layer.y_filter_cutoff,
            &format!("{prefix}.y.filterCutoff"),
        );
        changed |= apply_value_lane_menu_state(
            menu,
            &mut layer.y_filter_resonance,
            &format!("{prefix}.y.filterResonance"),
        );
    }
    changed
}

pub(super) fn apply_link_lfo_menu_state(
    menu: &super::NativeMenuModel,
    layer: &mut NativePulsesLayer,
    prefix: &str,
) -> bool {
    let mut changed = false;
    changed |= set_bool_from_menu(
        menu,
        &mut layer.link_lfo.enabled,
        &format!("{prefix}.enabled"),
    );
    if layer.link_lfo.target.is_none() && layer.link_lfo.enabled {
        layer.link_lfo.enabled = false;
        changed = true;
    }
    changed |= set_string_from_menu(
        menu,
        &mut layer.link_lfo.period,
        &format!("{prefix}.period"),
    );
    changed |= set_u8_from_menu(
        menu,
        &mut layer.link_lfo.depth_pct,
        &format!("{prefix}.depthPct"),
        100,
    );
    changed
}
