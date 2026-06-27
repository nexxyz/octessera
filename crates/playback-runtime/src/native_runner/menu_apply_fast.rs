use crate::protocol::RuntimeAudioCommand;

use super::{cutoff_display_to_hz, set_json_path_number, NativeRunner, PAN_POSITION_COUNT};

impl NativeRunner {
    pub(super) fn apply_or_schedule_menu_key(&mut self, key: &str) -> Result<(), String> {
        if self.apply_menu_key_fast(key) {
            return Ok(());
        }
        if self.should_defer_menu_key(key) {
            self.schedule_deferred_menu_apply(key);
            return Ok(());
        }
        self.apply_menu_state()
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
                    _ => self.apply_menu_state(),
                };
            }
        }
        if self.apply_deferred_menu_key_fast(key) {
            return Ok(());
        }
        self.apply_menu_state()
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
            _ => return false,
        };
        if changed {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn apply_runtime_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        match key {
            "danceMode" => Some(self.fast_dance_mode_menu_key()),
            "algorithmStep" => Some(self.fast_algorithm_step_menu_key()),
            "masterVolume" => Some(self.fast_master_volume_menu_key()),
            _ => None,
        }
    }

    fn fast_dance_mode_menu_key(&mut self) -> bool {
        let Some(dance_mode) = self.menu.selected_dance_mode() else {
            return false;
        };
        let changed = self.dance_mode != dance_mode;
        if changed {
            self.dance_mode = dance_mode.clone();
            if self.menu.is_in_dance_root_group() {
                self.active_dance_mode = dance_mode;
            }
            self.mark_fast_autosave_dirty();
            self.menu.rebuild(self.menu_config());
        }
        true
    }

    fn fast_algorithm_step_menu_key(&mut self) -> bool {
        let Some(step_pulses) = self.menu.selected_algorithm_step_pulses() else {
            return false;
        };
        let changed = self.algorithm_step_pulses != step_pulses
            || self
                .part_algorithm_step_pulses
                .get(self.active_part_index)
                .copied()
                .unwrap_or(self.algorithm_step_pulses)
                != step_pulses;
        if changed {
            self.algorithm_step_pulses = step_pulses;
            if let Some(part_step) = self
                .part_algorithm_step_pulses
                .get_mut(self.active_part_index)
            {
                *part_step = step_pulses;
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_master_volume_menu_key(&mut self) -> bool {
        let Some(master_volume) = self.menu.selected_master_volume() else {
            return false;
        };
        if self.ui.master_volume != master_volume {
            self.ui.master_volume = master_volume;
            self.mark_fast_autosave_dirty();
            self.queue_audio_command(RuntimeAudioCommand::SetMasterVolume {
                volume_pct: f32::from(master_volume),
            });
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

pub(super) fn structural_draft_key(key: &str) -> bool {
    if key == "behaviorId" {
        return true;
    }
    if let Some(rest) = key.strip_prefix("instruments.") {
        return parse_indexed_key(rest)
            .is_some_and(|(_, suffix)| matches!(suffix, "type" | "mixer.route"));
    }
    if let Some(rest) = key.strip_prefix("mixer.buses.") {
        return parse_indexed_key(rest)
            .is_some_and(|(_, suffix)| matches!(suffix, "slot1.type" | "slot2.type"));
    }
    if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
        return parse_indexed_key(rest).is_some_and(|(_, suffix)| suffix == "type");
    }
    false
}

fn parse_indexed_key(value: &str) -> Option<(usize, &str)> {
    let (index, suffix) = value.split_once('.')?;
    Some((index.parse().ok()?, suffix))
}

fn fast_instrument_volume(value: i32, instrument: &mut super::NativeInstrumentSlot) -> bool {
    let value = value.clamp(0, 100) as u8;
    if instrument.volume == value {
        false
    } else {
        instrument.volume = value;
        true
    }
}

fn fast_instrument_pan(value: i32, instrument: &mut super::NativeInstrumentSlot) -> bool {
    let value = value.clamp(0, i32::from(PAN_POSITION_COUNT - 1)) as u8;
    if instrument.pan_pos == value {
        false
    } else {
        instrument.pan_pos = value;
        true
    }
}

fn fast_instrument_synth_gain(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 100) as u8;
    if instrument.synth_gain_pct == value {
        None
    } else {
        instrument.synth_gain_pct = value;
        set_json_path_number(
            &mut instrument.synth_config,
            &["amp", "gainPct"],
            f64::from(value),
        );
        Some(f32::from(value))
    }
}

fn fast_instrument_filter_cutoff(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let display = value.clamp(0, 255);
    let cutoff = cutoff_display_to_hz(display) as u16;
    if super::synth_filter_cutoff(instrument) == cutoff {
        None
    } else {
        set_json_path_number(
            &mut instrument.synth_config,
            &["filter", "cutoffHz"],
            f64::from(cutoff),
        );
        Some(f32::from(cutoff))
    }
}

fn fast_instrument_filter_resonance(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 255) as u8;
    if super::synth_filter_resonance(instrument) == value {
        None
    } else {
        set_json_path_number(
            &mut instrument.synth_config,
            &["filter", "resonance"],
            f64::from(value),
        );
        Some(f32::from(value))
    }
}

fn fast_sample_tune(value: i32, instrument: &mut super::NativeInstrumentSlot) -> Option<f32> {
    let value = value.clamp(-24, 24) as i8;
    if instrument.sample_tune_semis == value {
        None
    } else {
        instrument.sample_tune_semis = value;
        Some(f32::from(value))
    }
}

fn fast_sample_gain(value: i32, instrument: &mut super::NativeInstrumentSlot) -> Option<f32> {
    let value = value.clamp(0, 100) as u8;
    if instrument.sample_gain_pct == value {
        None
    } else {
        instrument.sample_gain_pct = value;
        Some(f32::from(value))
    }
}

fn fast_sample_velocity_sensitivity(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 100) as u8;
    if instrument.sample_amp_velocity_sensitivity_pct == value {
        None
    } else {
        instrument.sample_amp_velocity_sensitivity_pct = value;
        Some(f32::from(value))
    }
}
