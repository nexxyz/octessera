use super::{
    default_dance_fx_selected, sanitize_dance_fx_config, NativeDanceFxAssignment, NativeRunner,
    Value, DEFAULT_ALGORITHM_STEP_PULSES, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn apply_config_payload(&mut self, payload: Value) -> Result<(), String> {
        let before_payload = self.config_payload();
        let runtime = payload.get("runtimeConfig").unwrap_or(&payload);
        let desired_active_part_index = runtime
            .get("activePartIndex")
            .and_then(Value::as_u64)
            .map(|value| (value as usize).min(GRID_HEIGHT.saturating_sub(1)))
            .unwrap_or(self.active_part_index);
        if let Some(active_part_index) = runtime.get("activePartIndex").and_then(Value::as_u64) {
            self.active_part_index =
                (active_part_index as usize).min(GRID_HEIGHT.saturating_sub(1));
        }
        self.apply_parts_payload(runtime)?;
        self.apply_touch_and_xy_payload(runtime, desired_active_part_index);
        self.swap_active_engine_from_part(desired_active_part_index)?;
        self.apply_instruments_payload(runtime);
        self.apply_runtime_ui_and_sound_payload(runtime, &payload);
        self.apply_sample_browser_favourites_payload(runtime);
        let active_behavior_id = self
            .part_behavior_ids
            .get(self.active_part_index)
            .cloned()
            .or_else(|| {
                payload
                    .get("activeBehavior")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&active_behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{active_behavior_id}`"))?;
        self.behavior = behavior;
        if let Some(active_l1) = runtime
            .get("parts")
            .and_then(Value::as_array)
            .and_then(|parts| parts.get(self.active_part_index))
            .and_then(|part| part.get("l1"))
        {
            self.behavior_config = active_l1
                .get("behaviorConfig")
                .cloned()
                .unwrap_or(Value::Null);
        }
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();
        self.engine
            .set_interpretation_profile(self.interpretation_profile.clone());
        self.sync_engine_runtime_config();
        self.menu.state = Default::default();
        self.menu.rebuild(self.menu_config());
        let after_payload = self.config_payload();
        if audio_config_changed(&before_payload, &after_payload) {
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        }
        Ok(())
    }

    pub(super) fn apply_touch_fx_payload(&mut self, touch_fx: &Value) {
        self.dance_fx_selected = sanitize_dance_fx_config(
            &touch_fx
                .get("selected")
                .cloned()
                .unwrap_or_else(default_dance_fx_selected),
        );
        self.dance_fx_assignments = touch_fx
            .get("assignments")
            .and_then(Value::as_array)
            .map(|assignments| {
                assignments
                    .iter()
                    .filter_map(|assignment| {
                        let x = assignment.get("x")?.as_u64()? as usize;
                        let y = assignment.get("y")?.as_u64()? as usize;
                        if x >= super::GRID_WIDTH || y >= super::GRID_HEIGHT {
                            return None;
                        }
                        Some(NativeDanceFxAssignment {
                            x,
                            y,
                            config: sanitize_dance_fx_config(
                                &assignment
                                    .get("config")
                                    .cloned()
                                    .unwrap_or_else(default_dance_fx_selected),
                            ),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.dance_fx_assign = None;
        self.active_part_index = self.active_part_index.min(GRID_HEIGHT.saturating_sub(1));
        self.algorithm_step_pulses = self
            .part_algorithm_step_pulses
            .get(self.active_part_index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES);
    }

    pub(super) fn apply_sample_browser_favourites_payload(&mut self, runtime: &Value) {
        self.sample_favourite_dirs = runtime
            .get("sampleFavouriteDirs")
            .and_then(Value::as_array)
            .map(|dirs| {
                dirs.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
    }
}

fn audio_config_changed(before: &Value, after: &Value) -> bool {
    let before = before.get("runtimeConfig").unwrap_or(before);
    let after = after.get("runtimeConfig").unwrap_or(after);
    ["instruments", "mixer", "masterVolume"]
        .into_iter()
        .any(|key| before.get(key) != after.get(key))
}
