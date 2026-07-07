use crate::protocol::RuntimeAudioCommand;

use super::menu_apply_fast::value_changed;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_runtime_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        match key {
            "transport.bpm" => Some(self.fast_bpm_menu_key()),
            "transport.swingPct" => Some(self.fast_swing_menu_key()),
            "midiSyncMode" => Some(self.fast_sync_source_menu_key()),
            "midiEnabled" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.midi_enabled, value)
            })),
            "midi.clockOutEnabled" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.midi_clock_out_enabled, value)
            })),
            "midi.clockInEnabled" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.midi_clock_in_enabled, value)
            })),
            "midi.respondToStartStop" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.midi_respond_to_start_stop, value)
            })),
            "danceMode" => Some(self.fast_dance_mode_menu_key()),
            "dance.page.mix" => Some(self.fast_dance_page_key("mix")),
            "dance.page.pan" => Some(self.fast_dance_page_key("pan")),
            "dance.page.fx" => Some(self.fast_dance_page_key("fx")),
            "dance.page.trigger-gate" => Some(self.fast_dance_page_key("trigger-gate")),
            "dance.page.transpose" => Some(self.fast_dance_page_key("transpose")),
            "dance.page.xy" => Some(self.fast_dance_page_key("xy")),
            "algorithmStep" => Some(self.fast_algorithm_step_menu_key()),
            "masterVolume" => Some(self.fast_master_volume_menu_key()),
            "displayBrightness" => Some(self.fast_display_brightness_menu_key()),
            "buttonBrightness" => Some(self.fast_button_brightness_menu_key()),
            "gridBrightness" => Some(self.fast_number_menu_key(key, |runner, value| {
                let value = value.clamp(10, 100) as u8;
                value_changed(&mut runner.ui.grid_brightness, value)
            })),
            "numericDisplayMode" => Some(self.fast_string_menu_key(key, |runner, value| {
                value_changed(&mut runner.ui.numeric_display_mode, value)
            })),
            "screenSleepSeconds" => Some(self.fast_number_menu_key(key, |runner, value| {
                let value = value.clamp(0, 600) as u16;
                value_changed(&mut runner.ui.screen_sleep_seconds, value)
            })),
            "dimTimerSeconds" => Some(self.fast_number_menu_key(key, |runner, value| {
                let value = value.clamp(0, 600) as u16;
                value_changed(&mut runner.ui.dim_timer_seconds, value)
            })),
            "ghostCells" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.ui.ghost_cells, value)
            })),
            "inputEventsWhilePaused" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.input_events_while_paused, value)
            })),
            "auxAutoMapEnabled" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.aux_auto_map_enabled, value)
            })),
            "autoSaveDefault" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.auto_save_default, value)
            })),
            "rollingBackups" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.rolling_backups, value)
            })),
            "sound.noteLengthMs" => Some(self.fast_sound_number_menu_key(key, |runner, value| {
                let value = value.clamp(30, 2000) as u32;
                value_changed(&mut runner.global_sound.note_length_ms, value)
            })),
            "sound.velocityScalePct" => {
                Some(self.fast_sound_number_menu_key(key, |runner, value| {
                    let value = value.clamp(0, 200) as u16;
                    value_changed(&mut runner.global_sound.velocity_scale_pct, value)
                }))
            }
            "sound.velocityCurve" => Some(self.fast_sound_string_menu_key(key)),
            "sound.audioOutputBufferFrames" => {
                Some(self.fast_audio_output_buffer_frames_menu_key())
            }
            _ => None,
        }
    }

    fn fast_bpm_menu_key(&mut self) -> bool {
        let Some(bpm) = self.menu.number_for_key("transport.bpm") else {
            return false;
        };
        let bpm = f64::from(bpm.clamp(40, 240));
        if (self.bpm - bpm).abs() > f64::EPSILON {
            self.bpm = bpm;
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_swing_menu_key(&mut self) -> bool {
        let Some(swing_pct) = self.menu.number_for_key("transport.swingPct") else {
            return false;
        };
        let swing_pct = swing_pct.clamp(0, 75) as u8;
        if self.swing_pct != swing_pct {
            self.swing_pct = swing_pct;
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_sync_source_menu_key(&mut self) -> bool {
        let Some(sync_source) = self.menu.selected_sync_source() else {
            return false;
        };
        if self.sync_source != sync_source {
            self.sync_source = sync_source;
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_display_brightness_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.selected_display_brightness() else {
            return false;
        };
        if value_changed(&mut self.ui.display_brightness, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_button_brightness_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.selected_button_brightness() else {
            return false;
        };
        if value_changed(&mut self.ui.button_brightness, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_bool_menu_key(
        &mut self,
        key: &str,
        apply: impl FnOnce(&mut Self, bool) -> bool,
    ) -> bool {
        let Some(value) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        if apply(self, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_number_menu_key(
        &mut self,
        key: &str,
        apply: impl FnOnce(&mut Self, i32) -> bool,
    ) -> bool {
        let Some(value) = self.menu.number_for_key(key) else {
            return false;
        };
        if apply(self, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_string_menu_key(
        &mut self,
        key: &str,
        apply: impl FnOnce(&mut Self, String) -> bool,
    ) -> bool {
        let Some(value) = self.menu.value_for_key(key) else {
            return false;
        };
        if apply(self, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_sound_number_menu_key(
        &mut self,
        key: &str,
        apply: impl FnOnce(&mut Self, i32) -> bool,
    ) -> bool {
        let Some(value) = self.menu.number_for_key(key) else {
            return false;
        };
        if apply(self, value) {
            self.sync_engine_runtime_config();
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_sound_string_menu_key(&mut self, key: &str) -> bool {
        let Some(value) = self.menu.value_for_key(key) else {
            return false;
        };
        let value = super::velocity_curve_from_id(&value);
        if value_changed(&mut self.global_sound.velocity_curve, value) {
            self.sync_engine_runtime_config();
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_audio_output_buffer_frames_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("sound.audioOutputBufferFrames") else {
            return false;
        };
        let value = value
            .parse::<u32>()
            .map(super::normalize_audio_output_buffer_frames)
            .unwrap_or(256);
        if value_changed(&mut self.audio_output_buffer_frames, value) {
            self.pending_audio_output_buffer_reboot_prompt = true;
            self.mark_fast_autosave_dirty();
            self.show_toast("Restart device to apply");
        }
        true
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
        }
        true
    }

    fn fast_dance_page_key(&mut self, dance_mode: &str) -> bool {
        let changed = self.dance_mode != dance_mode;
        if changed {
            self.dance_mode = dance_mode.into();
            self.mark_fast_autosave_dirty();
        }
        if self.menu.is_in_dance_root_group() {
            self.active_dance_mode = self.dance_mode.clone();
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
}

fn bool_changed(target: &mut bool, value: bool) -> bool {
    value_changed(target, value)
}
