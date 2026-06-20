use super::sense_payload_apply::apply_sense_payload;
use super::{
    apply_legacy_trigger_gates_payload, apply_trigger_probability_map_payload, note_unit_to_pulses,
    param_binding_from_payload, param_mods_from_payload, NativeRunner, Value,
    DEFAULT_ALGORITHM_STEP_PULSES, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn apply_parts_payload(&mut self, runtime: &Value) -> Result<(), String> {
        if let Some(parts) = runtime.get("parts").and_then(Value::as_array) {
            for (index, part) in parts.iter().take(GRID_HEIGHT).enumerate() {
                let l1 = part.get("l1");
                if let Some(behavior_id) = part
                    .get("l1")
                    .and_then(|l1| l1.get("behaviorId"))
                    .and_then(Value::as_str)
                {
                    if platform_core::get_native_behavior(behavior_id).is_some() {
                        self.part_behavior_ids[index] = behavior_id.into();
                    }
                }
                if let Some(auto_name) = part.get("autoName").and_then(Value::as_bool) {
                    if let Some(target) = self.part_auto_names.get_mut(index) {
                        *target = auto_name;
                    }
                }
                if let Some(name) = part.get("name").and_then(Value::as_str) {
                    if let Some(target) = self.part_names.get_mut(index) {
                        *target = name.into();
                    }
                } else if self.part_auto_names.get(index).copied().unwrap_or(true) {
                    if let Some(target) = self.part_names.get_mut(index) {
                        *target = self
                            .part_behavior_ids
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| self.behavior.id().into());
                    }
                }
                if let Some(l2) = part.get("l2") {
                    if let Some(sense_part) = self.sense_parts.get_mut(index) {
                        apply_sense_payload(sense_part, l2);
                    }
                    if let Some(target) = self.trigger_probability_maps.get_mut(index) {
                        if let Some(map) = l2.get("triggerProbabilityMap").and_then(Value::as_array)
                        {
                            apply_trigger_probability_map_payload(target, map);
                        } else if let Some(gates) = l1
                            .and_then(|l1| l1.get("triggerGates"))
                            .and_then(Value::as_array)
                        {
                            apply_legacy_trigger_gates_payload(target, gates);
                        }
                    }
                }
                if let (Some(l1), Some(behavior_id)) = (l1, self.part_behavior_ids.get(index)) {
                    if let Some(save_grid_state) = l1.get("saveGridState").and_then(Value::as_bool)
                    {
                        if let Some(target) = self.save_grid_states.get_mut(index) {
                            *target = save_grid_state;
                        }
                    }
                    if let Some(step_rate) = l1.get("stepRate").and_then(Value::as_str) {
                        if let Some(part_step) = self.part_algorithm_step_pulses.get_mut(index) {
                            *part_step = note_unit_to_pulses(step_rate);
                        }
                    }
                    if let Some(config) = l1.get("behaviorConfig") {
                        if let Some(target) = self.part_behavior_configs.get_mut(index) {
                            *target = config.clone();
                        }
                    }
                    let engine = self.rebuild_part_engine_from_payload(index, behavior_id, l1)?;
                    if let Some(slot) = self.part_engines.get_mut(index) {
                        *slot = Some(engine);
                    }
                }
                if let Some(param_mods) = part.get("paramMods") {
                    if let Some(target) = self.param_mods.get_mut(index) {
                        *target = param_mods_from_payload(param_mods);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn apply_touch_and_xy_payload(
        &mut self,
        runtime: &Value,
        desired_active_part_index: usize,
    ) {
        if let Some(touch_fx) = runtime.get("touchFx") {
            self.apply_touch_fx_payload(touch_fx);
        }
        if let Some(xy_touch) = runtime.get("xyTouch") {
            if let Some(x) = xy_touch.get("x").and_then(Value::as_f64) {
                self.xy_touch.x = (x as f32).clamp(0.0, 1.0);
            }
            if let Some(y) = xy_touch.get("y").and_then(Value::as_f64) {
                self.xy_touch.y = (y as f32).clamp(0.0, 1.0);
            }
            if let Some(active) = xy_touch.get("active").and_then(Value::as_bool) {
                self.xy_touch.active = active;
            }
        }
        if let Some(xy_release) = runtime.get("xyRelease").and_then(Value::as_str) {
            if matches!(xy_release, "sample-hold" | "reset-center") {
                self.xy_release = xy_release.into();
            }
        }
        if let Some(active_part) = runtime
            .get("parts")
            .and_then(Value::as_array)
            .and_then(|parts| parts.get(desired_active_part_index))
            .and_then(|part| part.get("xy"))
        {
            self.xy_x_binding = active_part.get("x").and_then(param_binding_from_payload);
            self.xy_y_binding = active_part.get("y").and_then(param_binding_from_payload);
            if let Some(invert) = active_part.get("xInvert").and_then(Value::as_bool) {
                self.xy_invert_x = invert;
            }
            if let Some(invert) = active_part.get("yInvert").and_then(Value::as_bool) {
                self.xy_invert_y = invert;
            }
        }
    }

    pub(super) fn swap_active_engine_from_part(
        &mut self,
        desired_active_part_index: usize,
    ) -> Result<(), String> {
        self.active_part_index = desired_active_part_index;
        self.algorithm_step_pulses = self
            .part_algorithm_step_pulses
            .get(self.active_part_index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES);
        if let Some(Some(engine)) = self.part_engines.get_mut(desired_active_part_index) {
            let placeholder = Self::build_engine(
                self.behavior,
                self.behavior_config.clone(),
                self.interpretation_profile.clone(),
                self.mapping_config.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                desired_active_part_index,
            )?;
            self.engine = std::mem::replace(engine, placeholder);
        }
        Ok(())
    }
}
