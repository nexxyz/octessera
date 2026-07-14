use crate::protocol::RuntimeAudioCommand;

use super::menu_apply_fast_values::*;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn fast_full_instrument_synth_key(&mut self, index: usize, _key: &str) -> bool {
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

    pub(super) fn fast_full_instrument_sample_key(&mut self, index: usize, key: &str) -> bool {
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

    pub(super) fn fast_midi_instrument_key(&mut self, index: usize, _key: &str) -> bool {
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        if super::menu_apply_instrument_midi::apply_midi_menu_fields(&self.menu, index, instrument)
        {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_instrument_volume_key(&mut self, index: usize, value: Option<i32>) -> bool {
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

    pub(super) fn fast_instrument_pan_key(&mut self, index: usize, value: Option<i32>) -> bool {
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
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        let Some(audio_value) =
            fast_instrument_synth_number(value, instrument, json_path, min, max)
        else {
            return false;
        };
        self.queue_audio_command(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: command_path.into(),
            value: audio_value,
        });
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
