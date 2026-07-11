use crate::protocol::RuntimeAudioCommand;
use std::time::{Duration, Instant};

pub(super) use super::menu_apply_fast_values::structural_draft_key;
use super::menu_apply_fast_values::*;
use super::NativeRunner;

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
            return Ok(());
        }
        if structural_draft_key(key) {
            return self.commit_structural_draft_key(key);
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
            "autoName" => Some(self.fast_layer_auto_name_key(index, key)),
            _ => None,
        }
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
        let prefix = format!("layers.{index}.pulses");
        let layer = self.pulses_layers.get_mut(index)?;
        let changed = if matches!(
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
            super::menu_apply_pulses_fx::apply_pulses_scan_and_mapping_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("pulses.triggerProbability")
            || suffix.starts_with("pulses.pitch.")
        {
            super::menu_apply_pulses_fx::apply_pulses_probability_and_pitch_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("pulses.x.") {
            super::menu_apply_pulses_fx::apply_pulses_axis_menu_state(
                &self.menu, layer, &prefix, "x",
            )
        } else if suffix.starts_with("pulses.y.") {
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

    fn fast_full_instrument_synth_key(&mut self, index: usize, _key: &str) -> bool {
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        if super::menu_apply_instrument_synth::apply_synth_menu_fields(
            &self.menu, index, instrument,
        ) {
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_full_instrument_sample_key(&mut self, index: usize, key: &str) -> bool {
        let changed = self.apply_instrument_menu_state();
        if changed {
            if key.ends_with(".selectedSlot") || key.ends_with(".velocityLevelsEnabled") {
                self.rematerialize_menu_around_key(key);
            }
            if let Some(config) = self.instrument_audio_config(index) {
                self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                    instrument_slot: index,
                    config,
                });
            } else {
                self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_midi_instrument_key(&mut self, index: usize, _key: &str) -> bool {
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        if super::menu_apply_instrument_midi::apply_midi_menu_fields(&self.menu, index, instrument)
        {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_instrument_volume_key(&mut self, index: usize, value: Option<i32>) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        let command_value = if fast_instrument_volume(value, instrument) {
            Some(f32::from(instrument.volume))
        } else {
            None
        };
        if let Some(volume_pct) = command_value {
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: index,
                volume_pct: Some(volume_pct),
                pan_pos: None,
            });
        }
        command_value.is_some()
    }

    fn fast_instrument_pan_key(&mut self, index: usize, value: Option<i32>) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        let command_value = if fast_instrument_pan(value, instrument) {
            Some(usize::from(instrument.pan_pos))
        } else {
            None
        };
        if let Some(pan_pos) = command_value {
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: index,
                volume_pct: None,
                pan_pos: Some(pan_pos),
            });
        }
        command_value.is_some()
    }

    fn fast_instrument_synth_key(
        &mut self,
        index: usize,
        value: Option<i32>,
        path: &'static str,
        apply: fn(i32, &mut super::NativeInstrumentSlot) -> Option<f32>,
    ) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        let Some(audio_value) = apply(value, instrument) else {
            return false;
        };
        self.queue_audio_command(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: path.into(),
            value: audio_value,
        });
        true
    }

    fn fast_sample_bank_key(
        &mut self,
        index: usize,
        value: Option<i32>,
        path: &'static str,
        apply: fn(i32, &mut super::NativeInstrumentSlot) -> Option<f32>,
    ) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        let Some(audio_value) = apply(value, instrument) else {
            return false;
        };
        self.queue_audio_command(RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: index,
            path: path.into(),
            value: audio_value,
        });
        true
    }

    pub(super) fn queue_audio_command(&mut self, command: RuntimeAudioCommand) {
        self.outbox.push_audio_command(command);
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
