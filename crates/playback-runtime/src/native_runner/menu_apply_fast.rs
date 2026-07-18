use std::time::{Duration, Instant};

pub(super) use super::menu_apply_fast_values::structural_draft_key;
use super::menu_apply_fast_values::*;
use super::{note_unit_to_pulses, NativeRunner};

impl NativeRunner {
    pub(super) fn apply_or_schedule_menu_key(&mut self, key: &str) -> Result<(), String> {
        let started = menu_key_profile_enabled().then(Instant::now);
        let result = self.apply_or_schedule_menu_key_inner(key);
        if let Some(started) = started {
            log_menu_key_duration("apply", key, started.elapsed());
        }
        result
    }

    fn apply_or_schedule_menu_key_inner(&mut self, key: &str) -> Result<(), String> {
        if self.apply_menu_key_fast(key) {
            self.clear_all_link_arp_state();
            return Ok(());
        }
        if self.apply_binding_range_key_fast(key) {
            self.clear_all_link_arp_state();
            return Ok(());
        }
        if structural_draft_key(key) {
            let result = self.commit_structural_draft_key(key);
            if result.is_ok() {
                self.clear_all_link_arp_state();
            }
            return result;
        }
        if self.should_defer_menu_key(key) {
            self.schedule_deferred_menu_apply(key);
            return Ok(());
        }
        Err(format!(
            "unhandled menu edit key `{key}`; add an explicit keyed apply handler"
        ))
    }

    pub(super) fn commit_structural_draft_key(&mut self, key: &str) -> Result<(), String> {
        self.clear_deferred_menu_apply();
        if key == "behaviorId" {
            return self.commit_behavior_structural_draft();
        }
        if let Some(rest) = key.strip_prefix("instruments.") {
            if let Some((index, suffix)) = parse_indexed_key(rest) {
                return match suffix {
                    "type" => {
                        self.commit_instrument_type_structural_draft(index);
                        Ok(())
                    }
                    "mixer.route" => {
                        self.commit_instrument_route_structural_draft(index);
                        Ok(())
                    }
                    _ => Err(format!(
                        "unhandled structural instrument edit key `instruments.{index}.{suffix}`"
                    )),
                };
            }
        }
        if self.apply_deferred_menu_key_fast(key) {
            return Ok(());
        }
        Err(format!(
            "unhandled structural menu edit key `{key}`; add an explicit commit handler"
        ))
    }

    fn should_defer_menu_key(&self, key: &str) -> bool {
        key == "sparksMode"
            || key == "sparks.fx.type"
            || key == "system.draftName"
            || key.starts_with("layers.") && key.ends_with(".name")
            || key.starts_with("instruments.") && key.ends_with(".name")
            || key.starts_with("mixer.buses.") && key.ends_with(".name")
            || structural_draft_key(key)
    }

    pub(super) fn apply_menu_key_fast(&mut self, key: &str) -> bool {
        if key == "sparks.fx.type" {
            return self.fast_sparks_fx_type_key(key);
        }
        if key == "sparks.fx.target" || key.starts_with("sparks.fx.params.") {
            return self.fast_sparks_fx_value_key();
        }
        if let Some(applied) = self.apply_runtime_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_fx_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_layer_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_behavior_config_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_pulses_menu_key_fast(key) {
            return applied;
        }
        let Some(rest) = key.strip_prefix("instruments.") else {
            return false;
        };
        let Some((index, suffix)) = parse_indexed_key(rest) else {
            return false;
        };
        let number_value = self.menu.number_for_key(key);
        let changed = match suffix {
            "noteBehavior" => self.fast_instrument_note_behavior_key(index, key),
            "mixer.volume" => self.fast_instrument_volume_key(index, number_value),
            "mixer.panPos" => self.fast_instrument_pan_key(index, number_value),
            "synth.amp.gainPct" => self.fast_instrument_synth_key(
                index,
                number_value,
                "synth.amp.gainPct",
                fast_instrument_synth_gain,
            ),
            "synth.filter.cutoffHz" => self.fast_instrument_synth_key(
                index,
                number_value,
                "synth.filter.cutoffHz",
                fast_instrument_filter_cutoff,
            ),
            "synth.filter.resonance" => self.fast_instrument_synth_key(
                index,
                number_value,
                "synth.filter.resonance",
                fast_instrument_filter_resonance,
            ),
            "synth.filter.envAmountPct" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.filter.envAmountPct",
                &["filter", "envAmountPct"],
                -100,
                100,
            ),
            "synth.filter.keyTrackingPct" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.filter.keyTrackingPct",
                &["filter", "keyTrackingPct"],
                0,
                100,
            ),
            "synth.amp.velocitySensitivityPct" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.amp.velocitySensitivityPct",
                &["amp", "velocitySensitivityPct"],
                0,
                100,
            ),
            "synth.ampEnv.attackMs" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.ampEnv.attackMs",
                &["ampEnv", "attackMs"],
                0,
                5000,
            ),
            "synth.ampEnv.decayMs" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.ampEnv.decayMs",
                &["ampEnv", "decayMs"],
                0,
                5000,
            ),
            "synth.ampEnv.sustainPct" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.ampEnv.sustainPct",
                &["ampEnv", "sustainPct"],
                0,
                100,
            ),
            "synth.ampEnv.releaseMs" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.ampEnv.releaseMs",
                &["ampEnv", "releaseMs"],
                0,
                10000,
            ),
            "synth.filterEnv.attackMs" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.filterEnv.attackMs",
                &["filterEnv", "attackMs"],
                0,
                5000,
            ),
            "synth.filterEnv.decayMs" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.filterEnv.decayMs",
                &["filterEnv", "decayMs"],
                0,
                5000,
            ),
            "synth.filterEnv.sustainPct" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.filterEnv.sustainPct",
                &["filterEnv", "sustainPct"],
                0,
                100,
            ),
            "synth.filterEnv.releaseMs" => self.fast_instrument_synth_number_key(
                index,
                number_value,
                "synth.filterEnv.releaseMs",
                &["filterEnv", "releaseMs"],
                0,
                10000,
            ),
            "sample.tuneSemis" => {
                self.fast_sample_bank_key(index, number_value, "sample.tuneSemis", fast_sample_tune)
            }
            "sample.amp.gainPct" => self.fast_sample_bank_key(
                index,
                number_value,
                "sample.amp.gainPct",
                fast_sample_gain,
            ),
            "sample.amp.velocitySensitivityPct" => self.fast_sample_bank_key(
                index,
                number_value,
                "sample.amp.velocitySensitivityPct",
                fast_sample_velocity_sensitivity,
            ),
            "sample.filter.cutoffHz" => self.fast_sample_bank_key(
                index,
                number_value,
                "sample.filter.cutoffHz",
                fast_sample_filter_cutoff,
            ),
            "sample.filter.resonance" => self.fast_sample_bank_key(
                index,
                number_value,
                "sample.filter.resonance",
                fast_sample_filter_resonance,
            ),
            suffix if suffix.starts_with("sample.") => {
                return self.fast_full_instrument_sample_key(index, key);
            }
            suffix if suffix.starts_with("synth.") => {
                self.fast_full_instrument_synth_key(index, key)
            }
            suffix if suffix.starts_with("midi.") => self.fast_midi_instrument_key(index, key),
            _ => return false,
        };
        if changed {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn apply_binding_range_key_fast(&mut self, key: &str) -> bool {
        let Some((target, suffix)) = key.rsplit_once('.') else {
            return false;
        };
        let is_min = match suffix {
            "rangeMin" => true,
            "rangeMax" => false,
            _ => return false,
        };
        let Some(value) = self.menu.number_for_key(key) else {
            return false;
        };
        self.set_param_binding_range_value(target, is_min, value);
        self.menu.rebuild(self.menu_config());
        let _ = self.menu.focus_item_key(key);
        true
    }

    fn fast_instrument_note_behavior_key(&mut self, index: usize, key: &str) -> bool {
        let Some(value) = self.menu.value_for_key(key) else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        if instrument.note_behavior == value {
            return false;
        }
        instrument.note_behavior = value;
        self.sync_engine_runtime_config();
        true
    }

    fn apply_layer_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("layers.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        match suffix {
            "algorithmStep" => Some(self.fast_layer_algorithm_step_key(index, key)),
            "autoName" => Some(self.fast_layer_auto_name_key(index, key)),
            _ => None,
        }
    }

    fn fast_layer_algorithm_step_key(&mut self, index: usize, key: &str) -> bool {
        let Some(value) = self.menu.value_for_key(key) else {
            return false;
        };
        let Some(layer_step) = self.layer_algorithm_step_pulses.get_mut(index) else {
            return false;
        };
        let pulses = note_unit_to_pulses(&value);
        if *layer_step == pulses {
            return false;
        }
        *layer_step = pulses;
        if index == self.active_layer_index {
            self.algorithm_step_pulses = pulses;
        }
        true
    }

    fn apply_behavior_config_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if !key.contains(".worlds.behaviorConfig.") {
            return None;
        }
        Some(self.fast_behavior_config_key().unwrap_or(false))
    }

    fn apply_pulses_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("layers.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        let layer = self.pulses_layers.get_mut(index)?;
        let changed = if suffix.starts_with("linkLfo.target.range") {
            return None;
        } else if suffix.starts_with("linkLfo.") {
            self.restore_link_lfo_base_audio();
            let layer = self.pulses_layers.get_mut(index)?;
            super::menu_apply_pulses_fx::apply_link_lfo_menu_state(
                &self.menu,
                layer,
                &format!("layers.{index}.linkLfo"),
            )
        } else if suffix.starts_with("pulses.arp.") {
            let prefix = format!("layers.{index}.pulses.arp");
            super::menu_apply_pulses_fx::apply_link_arp_menu_state(&self.menu, layer, &prefix)
        } else if matches!(
            suffix,
            "pulses.scanMode"
                | "pulses.scanAxis"
                | "pulses.scanUnit"
                | "pulses.scanDirection"
                | "pulses.scanSections"
                | "pulses.eventEnabled"
                | "pulses.stateNotesEnabled"
        ) || suffix.starts_with("pulses.mapping.")
        {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_scan_and_mapping_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("pulses.triggerProbability")
            || suffix.starts_with("pulses.pitch.")
        {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_probability_and_pitch_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("pulses.x.") {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_axis_menu_state(
                &self.menu, layer, &prefix, "x",
            )
        } else if suffix.starts_with("pulses.y.") {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_axis_menu_state(
                &self.menu, layer, &prefix, "y",
            )
        } else {
            return None;
        };
        if changed {
            if suffix == "pulses.scanMode" {
                self.rematerialize_menu_around_key(key);
            }
            if suffix.starts_with("pulses.arp.") {
                self.clear_link_arp_state_for_layer(index);
            }
            if index == self.active_layer_index {
                self.refresh_active_mapping_config();
                self.refresh_active_interpretation_profile();
                self.engine
                    .set_interpretation_profile(self.interpretation_profile.clone());
            }
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }

    pub(super) fn rematerialize_menu_around_key(&mut self, key: &str) {
        let was_editing = self.menu.state.editing;
        self.menu.rebuild(self.menu_config());
        let _ = self.menu.focus_item_key(key);
        self.menu.state.editing = was_editing;
    }

    fn fast_layer_auto_name_key(&mut self, index: usize, key: &str) -> bool {
        let Some(auto_name) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let Some(target) = self.layer_auto_names.get_mut(index) else {
            return false;
        };
        let mut changed = false;
        if *target != auto_name {
            *target = auto_name;
            changed = true;
        }
        if auto_name {
            let behavior_id = self
                .layer_behavior_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| self.behavior.id().into());
            if let Some(name) = self.layer_names.get_mut(index) {
                changed |= value_changed(name, behavior_id);
            }
        }
        if changed {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_behavior_config_key(&mut self) -> Result<bool, String> {
        if self.apply_behavior_config_menu_state()? {
            self.mark_fast_autosave_dirty();
        }
        Ok(true)
    }
}

fn menu_key_profile_enabled() -> bool {
    std::env::var("OCTESSERA_PI_UI_PROFILE")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "profile" | "ui" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn log_menu_key_duration(stage: &str, key: &str, duration: Duration) {
    if duration >= Duration::from_millis(5) {
        eprintln!(
            "menu-key-profile stage={stage} key={key} duration_us={}",
            duration.as_micros()
        );
    }
}

pub(super) fn value_changed<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}
