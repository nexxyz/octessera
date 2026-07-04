use super::*;

impl NativeRunner {
    pub(super) fn mapping_config_for_part(
        &self,
        part_index: usize,
    ) -> platform_core::MappingConfig {
        let Some(sense) = self.sense_parts.get(part_index) else {
            return self.base_mapping_config.clone();
        };
        let mut mapping = self.base_mapping_config.clone();
        mapping.base_midi_note = i32::from(sense.lowest_note.min(sense.highest_note));
        mapping.starting_midi_note = i32::from(sense.starting_note);
        mapping.max_midi_note = i32::from(sense.lowest_note.max(sense.highest_note));
        mapping.column_step_degrees = if sense.x_pitch_enabled {
            sense.x_pitch_steps
        } else {
            0
        };
        mapping.row_step_degrees = if sense.y_pitch_enabled {
            sense.y_pitch_steps
        } else {
            0
        };
        mapping.range_mode = if sense.out_of_range == "clamp" {
            RangeMode::Clamp
        } else {
            RangeMode::Wrap
        };
        mapping.scale = scale_steps(&sense.scale, &sense.root);
        mapping.activate = trigger_target(sense.activate_slot, &sense.activate_action, 96, 150);
        mapping.stable = trigger_target(sense.stable_slot, &sense.stable_action, 88, 130);
        mapping.deactivate =
            trigger_target(sense.deactivate_slot, &sense.deactivate_action, 68, 90);
        mapping.scanned = trigger_target(sense.scanned_slot, &sense.scanned_action, 88, 130);
        mapping.scanned_empty = trigger_target(
            sense.scanned_empty_slot,
            &sense.scanned_empty_action,
            68,
            90,
        );
        mapping
    }

    pub(super) fn interpretation_profile_for_part(
        &self,
        part_index: usize,
    ) -> InterpretationProfile {
        let Some(sense) = self.sense_parts.get(part_index) else {
            return self.interpretation_profile.clone();
        };
        let mut profile = self.interpretation_profile.clone();
        profile.event.enabled = sense.event_enabled;
        profile.state.enabled = sense.state_notes_enabled || sense.scan_mode == "scanning";
        let sections = if sense.scan_sections <= 1 {
            None
        } else {
            Some(usize::from(sense.scan_sections))
        };
        profile.state.tick = if sense.scan_mode == "scanning" {
            let reverse = sense.scan_direction == "reverse";
            if sense.scan_axis == "columns" {
                TickStrategy::ScanColumnActive { sections, reverse }
            } else {
                TickStrategy::ScanRowActive { sections, reverse }
            }
        } else {
            TickStrategy::WholeGridTransitions
        };
        profile.x = AxisStrategy::ScaleStep {
            step: if sense.x_pitch_enabled {
                sense.x_pitch_steps.max(0) as usize
            } else {
                0
            },
        };
        profile.y = AxisStrategy::ScaleStep {
            step: if sense.y_pitch_enabled {
                sense.y_pitch_steps.max(0) as usize
            } else {
                0
            },
        };
        profile
    }

    pub(super) fn activate_engine(&mut self, index: usize) -> Result<(), String> {
        let behavior_id = self
            .part_behavior_ids
            .get(index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let profile = self.interpretation_profile_for_part(index);
        let mapping = self.mapping_config_for_part(index);
        let next = if let Some(slot) = self.part_engines.get_mut(index) {
            slot.take()
        } else {
            None
        };
        self.engine = if let Some(mut engine) = next {
            engine.set_interpretation_profile(profile.clone());
            engine.set_mapping_config(mapping.clone());
            engine
        } else {
            Self::build_engine(
                behavior,
                self.part_behavior_configs
                    .get(index)
                    .filter(|config| !config.is_null())
                    .cloned()
                    .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
                    .unwrap_or(Value::Null),
                profile.clone(),
                mapping.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                index,
            )?
        };
        self.behavior = behavior;
        self.behavior_config = self
            .part_behavior_configs
            .get(index)
            .filter(|config| !config.is_null())
            .cloned()
            .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
            .unwrap_or(Value::Null);
        self.interpretation_profile = profile;
        self.mapping_config = mapping;
        Ok(())
    }

    pub(super) fn switch_active_engine(&mut self, index: usize) -> Result<(), String> {
        let next_index = index.min(GRID_HEIGHT.saturating_sub(1));
        if next_index == self.active_part_index {
            return Ok(());
        }

        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }

        let behavior_id = self
            .part_behavior_ids
            .get(next_index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let behavior_config = self
            .part_behavior_configs
            .get(next_index)
            .filter(|config| !config.is_null())
            .cloned()
            .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
            .unwrap_or(Value::Null);
        let profile = self.interpretation_profile_for_part(next_index);
        let mapping = self.mapping_config_for_part(next_index);
        let mut next_engine = match self.part_engines.get_mut(next_index).and_then(Option::take) {
            Some(engine) => engine,
            None => Self::build_engine(
                behavior,
                behavior_config.clone(),
                profile.clone(),
                mapping.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                next_index,
            )?,
        };
        next_engine.set_interpretation_profile(profile.clone());
        next_engine.set_mapping_config(mapping.clone());

        let previous_index = self.active_part_index;
        std::mem::swap(&mut self.engine, &mut next_engine);
        if let Some(slot) = self.part_engines.get_mut(previous_index) {
            *slot = Some(next_engine);
        }

        self.active_part_index = next_index;
        self.algorithm_step_pulses = self
            .part_algorithm_step_pulses
            .get(next_index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES);
        self.behavior = behavior;
        self.behavior_config = behavior_config;
        self.interpretation_profile = profile;
        self.mapping_config = mapping;
        Ok(())
    }

    pub(super) fn serialized_state_for_part(&self, index: usize) -> Result<Value, String> {
        if index == self.active_part_index {
            return self.engine.serialized_state();
        }
        if let Some(Some(engine)) = self.part_engines.get(index) {
            return engine.serialized_state();
        }
        Ok(Value::Null)
    }

    pub(super) fn l1_payload_for_part(&self, index: usize, behavior_id: &str) -> Value {
        let step_pulses = if index == self.active_part_index {
            self.algorithm_step_pulses
        } else {
            self.part_algorithm_step_pulses
                .get(index)
                .copied()
                .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES)
        };
        let save_grid_state = self.save_grid_states.get(index).copied().unwrap_or(true);
        let mut l1 = serde_json::Map::new();
        l1.insert("behaviorId".into(), json!(behavior_id));
        l1.insert("stepRate".into(), json!(note_unit_from_pulses(step_pulses)));
        l1.insert(
            "behaviorConfig".into(),
            if index == self.active_part_index {
                self.behavior_config.clone()
            } else {
                self.part_behavior_configs
                    .get(index)
                    .cloned()
                    .unwrap_or(Value::Null)
            },
        );
        l1.insert("saveGridState".into(), json!(save_grid_state));
        if save_grid_state {
            if let Ok(state) = self.serialized_state_for_part(index) {
                if !state.is_null() {
                    l1.insert("savedState".into(), state);
                }
            }
        }
        Value::Object(l1)
    }

    pub(super) fn rebuild_part_engine_from_payload(
        &self,
        index: usize,
        behavior_id: &str,
        l1: &Value,
    ) -> Result<NativePartEngine, String> {
        let behavior = platform_core::get_native_behavior(behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let config = NativePartEngineConfig {
            behavior,
            behavior_config: l1.get("behaviorConfig").cloned().unwrap_or(Value::Null),
            interpretation_profile: self.interpretation_profile_for_part(index),
            mapping_config: self.mapping_config_for_part(index),
            global_sound: self.global_sound.clone(),
            note_behaviors: self.note_behaviors.clone(),
            part_index: index,
        };
        let save_grid_state = l1
            .get("saveGridState")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let state = l1
            .get("savedState")
            .filter(|value| !value.is_null())
            .or_else(|| l1.get("behaviorState").filter(|value| !value.is_null()));
        if let Some(state) = state.filter(|_| save_grid_state) {
            NativePartEngine::from_serialized_state(config, state.clone())
        } else {
            NativePartEngine::new(config)
        }
    }

    pub(super) fn remap_bindings_for_behavior_change(
        &mut self,
        from_behavior_id: &str,
        to_behavior_id: &str,
        part_index: usize,
    ) {
        let Some(to_behavior) = platform_core::get_native_behavior(to_behavior_id) else {
            return;
        };
        if let Some(param_mods) = self.param_mods.get_mut(part_index) {
            for binding in param_mods.x.iter_mut().chain(param_mods.y.iter_mut()) {
                if let Some(current) = binding.clone() {
                    if let Some(next) =
                        remap_behavior_param_binding(current, to_behavior, part_index)
                    {
                        *binding = Some(next);
                    }
                }
            }
        }

        let from_action =
            platform_core::get_native_behavior(from_behavior_id).and_then(primary_behavior_action);
        let to_action = primary_behavior_action(to_behavior);
        for binding in &mut self.aux_bindings {
            let Some(aux) = binding else {
                continue;
            };
            if let Some(turn_key) = aux.turn_key.clone() {
                if let Some(remapped) = remap_behavior_binding_key(&turn_key, to_behavior, None) {
                    aux.turn_key = Some(remapped.key);
                }
            }
            if let (Some((from_action, _)), Some(NativeMenuAction::BehaviorAction(action))) =
                (&from_action, aux.press_action.as_ref())
            {
                if action == from_action {
                    aux.press_action = to_action
                        .as_ref()
                        .map(|(action, _)| NativeMenuAction::BehaviorAction(action.clone()));
                }
            }
            if aux.turn_key.is_none() && aux.press_action.is_none() {
                *binding = None;
            }
        }
    }
}
