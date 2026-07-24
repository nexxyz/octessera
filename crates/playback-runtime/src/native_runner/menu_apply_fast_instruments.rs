use crate::protocol::RuntimeAudioCommand;

use super::menu_apply_fast_values::*;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_instrument_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("instruments.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
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
                return Some(self.fast_full_instrument_sample_key(index, key));
            }
            suffix if suffix.starts_with("synth.") => {
                self.fast_full_instrument_synth_key(index, key)
            }
            suffix if suffix.starts_with("midi.") => self.fast_midi_instrument_key(index, key),
            _ => return None,
        };
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Some(true)
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
        self.rebase_and_recompose_modulation_key(key);
        true
    }

    pub(super) fn fast_full_instrument_synth_key(&mut self, index: usize, key: &str) -> bool {
        let changed = {
            let Some(instrument) = self.instruments.get_mut(index) else {
                return false;
            };
            super::menu_apply_instrument_synth::apply_synth_menu_fields(
                &self.menu, index, instrument,
            )
        };
        if changed {
            self.audio_config_revision = self.audio_config_revision.saturating_add(1);
            self.mark_fast_autosave_dirty();
            self.rebase_and_recompose_modulation_key(key);
        }
        true
    }

    pub(super) fn fast_full_instrument_sample_key(&mut self, index: usize, key: &str) -> bool {
        let changed = self.apply_instrument_menu_state();
        if changed {
            if key.ends_with(".selectedSlot") || key.ends_with(".velocityLevelsEnabled") {
                self.rematerialize_menu_around_key(key);
            }
            let rebased = self.rebase_and_recompose_modulation_key(key);
            if !rebased {
                if let Some(config) = self.instrument_audio_config(index) {
                    self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                        instrument_slot: index,
                        config,
                    });
                } else {
                    self.audio_config_revision = self.audio_config_revision.saturating_add(1);
                }
            } else if self.instrument_audio_config(index).is_none() {
                self.audio_config_revision = self.audio_config_revision.saturating_add(1);
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_midi_instrument_key(&mut self, index: usize, _key: &str) -> bool {
        let changed = {
            let Some(instrument) = self.instruments.get_mut(index) else {
                return false;
            };
            super::menu_apply_instrument_midi::apply_midi_menu_fields(&self.menu, index, instrument)
        };
        if changed {
            self.rebase_and_recompose_modulation_key(_key);
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_instrument_volume_key(&mut self, index: usize, value: Option<i32>) -> bool {
        let Some(value) = value else {
            return false;
        };
        let key = format!("instruments.{index}.mixer.volume");
        let command_value = {
            let Some(instrument) = self.instruments.get_mut(index) else {
                return false;
            };
            fast_instrument_volume(value, instrument).then(|| f32::from(instrument.volume))
        };
        if command_value.is_some() && !self.rebase_and_recompose_modulation_key(&key) {
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: index,
                volume_pct: command_value,
                pan_pos: None,
            });
        }
        command_value.is_some()
    }

    pub(super) fn fast_instrument_pan_key(&mut self, index: usize, value: Option<i32>) -> bool {
        let Some(value) = value else {
            return false;
        };
        let key = format!("instruments.{index}.mixer.panPos");
        let command_value = {
            let Some(instrument) = self.instruments.get_mut(index) else {
                return false;
            };
            fast_instrument_pan(value, instrument).then(|| usize::from(instrument.pan_pos))
        };
        if command_value.is_some() && !self.rebase_and_recompose_modulation_key(&key) {
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: index,
                volume_pct: None,
                pan_pos: command_value,
            });
        }
        command_value.is_some()
    }

    pub(super) fn fast_instrument_synth_key(
        &mut self,
        index: usize,
        value: Option<i32>,
        path: &'static str,
        apply: fn(i32, &mut super::NativeInstrumentSlot) -> Option<f32>,
    ) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(audio_value) = (|| {
            let instrument = self.instruments.get_mut(index)?;
            apply(value, instrument)
        })() else {
            return false;
        };
        if !self.rebase_and_recompose_modulation_key(&path_key(index, path)) {
            self.queue_audio_command(RuntimeAudioCommand::SetSynthParam {
                instrument_slot: index,
                path: path.into(),
                value: audio_value,
            });
        }
        true
    }

    pub(super) fn fast_instrument_synth_number_key(
        &mut self,
        index: usize,
        value: Option<i32>,
        command_path: &'static str,
        json_path: &[&str],
        min: i32,
        max: i32,
    ) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(audio_value) = (|| {
            let instrument = self.instruments.get_mut(index)?;
            fast_instrument_synth_number(value, instrument, json_path, min, max)
        })() else {
            return false;
        };
        if !self.rebase_and_recompose_modulation_key(&path_key(index, command_path)) {
            self.queue_audio_command(RuntimeAudioCommand::SetSynthParam {
                instrument_slot: index,
                path: command_path.into(),
                value: audio_value,
            });
        }
        true
    }

    pub(super) fn fast_sample_bank_key(
        &mut self,
        index: usize,
        value: Option<i32>,
        path: &'static str,
        apply: fn(i32, &mut super::NativeInstrumentSlot) -> Option<f32>,
    ) -> bool {
        let Some(value) = value else {
            return false;
        };
        let Some(audio_value) = (|| {
            let instrument = self.instruments.get_mut(index)?;
            apply(value, instrument)
        })() else {
            return false;
        };
        if !self.rebase_and_recompose_modulation_key(&path_key(index, path)) {
            self.queue_audio_command(RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: index,
                path: path.into(),
                value: audio_value,
            });
        }
        true
    }

    pub(super) fn queue_audio_command(&mut self, command: RuntimeAudioCommand) {
        self.outbox.push_audio_command(command);
    }
}

fn path_key(index: usize, path: &str) -> String {
    format!("instruments.{index}.{path}")
}
