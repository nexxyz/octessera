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
        key == "danceMode"
            || key == "dance.fx.type"
            || key == "system.draftName"
            || key.starts_with("parts.") && key.ends_with(".name")
            || key.starts_with("instruments.") && key.ends_with(".name")
            || key.starts_with("mixer.buses.") && key.ends_with(".name")
            || structural_draft_key(key)
    }

    pub(super) fn apply_menu_key_fast(&mut self, key: &str) -> bool {
        if let Some(applied) = self.apply_runtime_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_fx_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_part_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_behavior_config_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_sense_menu_key_fast(key) {
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

    fn apply_part_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("parts.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        match suffix {
            "autoName" => Some(self.fast_part_auto_name_key(index, key)),
            _ => None,
        }
    }

    fn apply_behavior_config_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if !key.contains(".l1.behaviorConfig.") {
            return None;
        }
        Some(self.fast_behavior_config_key().unwrap_or(false))
    }

    fn apply_sense_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("parts.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        let prefix = format!("parts.{index}.l2");
        let part = self.sense_parts.get_mut(index)?;
        let changed = if matches!(
            suffix,
            "l2.scanMode"
                | "l2.scanAxis"
                | "l2.scanUnit"
                | "l2.scanDirection"
                | "l2.scanSections"
                | "l2.eventEnabled"
                | "l2.stateNotesEnabled"
        ) || suffix.starts_with("l2.mapping.")
        {
            super::menu_apply_sense_fx::apply_sense_scan_and_mapping_menu_state(
                &self.menu, part, &prefix,
            )
        } else if suffix.starts_with("l2.triggerProbability") || suffix.starts_with("l2.pitch.") {
            super::menu_apply_sense_fx::apply_sense_probability_and_pitch_menu_state(
                &self.menu, part, &prefix,
            )
        } else if suffix.starts_with("l2.x.") {
            super::menu_apply_sense_fx::apply_sense_axis_menu_state(&self.menu, part, &prefix, "x")
        } else if suffix.starts_with("l2.y.") {
            super::menu_apply_sense_fx::apply_sense_axis_menu_state(&self.menu, part, &prefix, "y")
        } else {
            return None;
        };
        if changed {
            if index == self.active_part_index {
                self.refresh_active_mapping_config();
                self.refresh_active_interpretation_profile();
                self.engine
                    .set_interpretation_profile(self.interpretation_profile.clone());
            }
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }

    fn fast_part_auto_name_key(&mut self, index: usize, key: &str) -> bool {
        let Some(auto_name) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let Some(target) = self.part_auto_names.get_mut(index) else {
            return false;
        };
        let mut changed = false;
        if *target != auto_name {
            *target = auto_name;
            changed = true;
        }
        if auto_name {
            let behavior_id = self
                .part_behavior_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| self.behavior.id().into());
            if let Some(name) = self.part_names.get_mut(index) {
                changed |= value_changed(name, behavior_id);
            }
        }
        if changed {
            self.menu.rebuild(self.menu_config());
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
    std::env::var("CELLSYMPHONY_PI_UI_PROFILE")
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
