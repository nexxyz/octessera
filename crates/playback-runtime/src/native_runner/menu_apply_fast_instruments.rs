use crate::protocol::RuntimeAudioCommand;

use super::menu_apply_fast_values::*;
use super::NativeRunner;

mod drum;
pub(super) mod fm;
pub(super) mod pluck;

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
            suffix if suffix.starts_with("fm.") => {
                return fm::apply_fm_menu_key(self, index, key, suffix)
            }
            suffix if suffix.starts_with("pluck.") => {
                return pluck::apply_pluck_menu_key(self, index, key, suffix)
            }
            suffix if suffix.starts_with("drum.") => {
                return drum::apply_drum_menu_key(self, index, key, suffix)
            }
            suffix if suffix.starts_with("midiDrum.") => {
                return Some(self.fast_midi_drum_key(index, key, suffix))
            }
            suffix if suffix.starts_with("tile.") => {
                return Some(self.fast_tile_stamp_key(index, key, suffix))
            }
            "sample.followTranspose" | "sample.followPitch" => {
                return Some(self.fast_sample_follow_key(index, key, suffix))
            }
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
            suffix if super::synth_scalar_field(suffix).is_some() => {
                self.fast_instrument_synth_number_key(index, number_value, suffix)
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
        let sampler = instrument.kind == "sampler";
        self.sync_engine_runtime_config();
        if !self.rebase_and_recompose_modulation_key(key) && sampler {
            self.queue_instrument_slot_config(index);
        }
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
            self.commit_full_configuration_runtime_plan();
            self.mark_fast_autosave_dirty();
            self.rebase_and_recompose_modulation_key(key);
        }
        true
    }

    pub(super) fn fast_full_instrument_sample_key(&mut self, index: usize, key: &str) -> bool {
        let changed = self.apply_sampler_menu_key(index, key);
        if changed {
            if key.ends_with(".selectedSlot")
                || key.ends_with(".velocityLevelsEnabled")
                || key.ends_with(".loop")
            {
                self.show_instrument_rows_around_key(index, key);
            }
            if !self.rebase_and_recompose_modulation_key(key) {
                if !self.queue_instrument_slot_config(index) {
                    self.commit_full_configuration_runtime_plan();
                }
            } else if self.instrument_audio_config(index).is_none() {
                self.commit_full_configuration_runtime_plan();
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_midi_instrument_key(&mut self, index: usize, _key: &str) -> bool {
        let (changed, route_changed) = {
            let Some(instrument) = self.instruments.get_mut(index) else {
                return false;
            };
            let route = (instrument.midi_enabled, instrument.midi_channel);
            let changed = super::menu_apply_instrument_midi::apply_midi_menu_fields(
                &self.menu, index, instrument,
            );
            (
                changed,
                route != (instrument.midi_enabled, instrument.midi_channel),
            )
        };
        if route_changed {
            self.drain_play_transpose_instrument_notes(index);
            self.menu.show_instrument_label(&self.menu_config(), index);
        }
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
                generation: 0,
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
                generation: 0,
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
                generation: 0,
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
        command_path: &str,
    ) -> bool {
        let (Some(value), Some((json_path, min, max))) =
            (value, super::synth_scalar_field(command_path))
        else {
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
                generation: 0,
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
                generation: 0,
                path: path.into(),
                value: audio_value,
            });
        }
        true
    }

    /// Sends slot `index` its whole audio config; false when the slot has none.
    pub(super) fn queue_instrument_slot_config(&mut self, index: usize) -> bool {
        let Some(config) = self.instrument_audio_config(index) else {
            return false;
        };
        self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: index,
            generation: 0,
            config,
        });
        true
    }

    pub(super) fn queue_audio_command(&mut self, command: RuntimeAudioCommand) {
        self.enqueue_configuration_runtime_plan(
            super::ConfigurationRuntimePlan::dynamic_audio_command(command),
        );
    }
}

fn path_key(index: usize, path: &str) -> String {
    format!("instruments.{index}.{path}")
}
