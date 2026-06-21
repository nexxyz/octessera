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

    fn should_defer_menu_key(&self, key: &str) -> bool {
        key == "behaviorId"
            || key == "danceMode"
            || key == "dance.fx.type"
            || key.ends_with(".slot1.type")
            || key.ends_with(".slot2.type")
            || key.starts_with("mixer.master.slots.") && key.ends_with(".type")
            || key.starts_with("instruments.") && key.ends_with(".type")
    }

    pub(super) fn apply_menu_key_fast(&mut self, key: &str) -> bool {
        if let Some(applied) = self.apply_runtime_menu_key_fast(key) {
            return applied;
        }
        let Some(rest) = key.strip_prefix("instruments.") else {
            return false;
        };
        let Some((index, suffix)) = parse_indexed_key(rest) else {
            return false;
        };
        let number_value = self.menu.number_for_key(key);
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        let changed = match suffix {
            "mixer.volume" => number_value
                .map(|value| fast_instrument_volume(value, instrument))
                .unwrap_or(false),
            "mixer.panPos" => number_value
                .map(|value| fast_instrument_pan(value, instrument))
                .unwrap_or(false),
            "synth.amp.gainPct" => number_value
                .map(|value| fast_instrument_synth_gain(value, instrument))
                .unwrap_or(false),
            "synth.filter.cutoffHz" => number_value
                .map(|value| fast_instrument_filter_cutoff(value, instrument))
                .unwrap_or(false),
            "synth.filter.resonance" => number_value
                .map(|value| fast_instrument_filter_resonance(value, instrument))
                .unwrap_or(false),
            _ => return false,
        };
        if changed {
            self.config_dirty = true;
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
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
            if self.menu.state.stack.first() == Some(&3) {
                self.active_dance_mode = dance_mode;
            }
            self.config_dirty = true;
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
            self.config_dirty = true;
        }
        true
    }

    fn fast_master_volume_menu_key(&mut self) -> bool {
        let Some(master_volume) = self.menu.selected_master_volume() else {
            return false;
        };
        if self.ui.master_volume != master_volume {
            self.ui.master_volume = master_volume;
            self.config_dirty = true;
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        }
        true
    }
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

fn fast_instrument_synth_gain(value: i32, instrument: &mut super::NativeInstrumentSlot) -> bool {
    let value = value.clamp(0, 100) as u8;
    if instrument.synth_gain_pct == value {
        false
    } else {
        instrument.synth_gain_pct = value;
        set_json_path_number(
            &mut instrument.synth_config,
            &["amp", "gainPct"],
            f64::from(value),
        );
        true
    }
}

fn fast_instrument_filter_cutoff(value: i32, instrument: &mut super::NativeInstrumentSlot) -> bool {
    let display = value.clamp(0, 255);
    let cutoff = cutoff_display_to_hz(display) as u16;
    if super::synth_filter_cutoff(instrument) == cutoff {
        false
    } else {
        set_json_path_number(
            &mut instrument.synth_config,
            &["filter", "cutoffHz"],
            f64::from(cutoff),
        );
        true
    }
}

fn fast_instrument_filter_resonance(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> bool {
    let value = value.clamp(0, 255) as u8;
    if super::synth_filter_resonance(instrument) == value {
        false
    } else {
        set_json_path_number(
            &mut instrument.synth_config,
            &["filter", "resonance"],
            f64::from(value),
        );
        true
    }
}
