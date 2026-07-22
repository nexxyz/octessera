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
            "usb.audioOut" => Some(self.fast_usb_audio_out_menu_key()),
            "usb.midiOutEnabled" => Some(self.fast_usb_midi_out_menu_key()),
            "recording.maxMinutes" => Some(self.fast_recording_max_minutes_menu_key()),
            "hdmi.mode" => Some(self.fast_string_menu_key(key, |runner, value| {
                let value = match value.as_str() {
                    "none" | "live-grid" | "plain-grid" | "active-behavior" | "cycle-behaviors" => {
                        value
                    }
                    _ => "none".into(),
                };
                value_changed(&mut runner.display.hdmi.mode, value)
            })),
            "hdmi.showGridlines" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.display.hdmi.show_gridlines, value)
            })),
            "hdmi.cycleMeasures" => Some(self.fast_number_menu_key(key, |runner, value| {
                value_changed(
                    &mut runner.display.hdmi.cycle_measures,
                    value.clamp(1, 64) as u8,
                )
            })),
            "sparksMode" => Some(self.fast_sparks_mode_menu_key()),
            "sparks.page.mix" => Some(self.fast_sparks_page_key("mix")),
            "sparks.page.pan" => Some(self.fast_sparks_page_key("pan")),
            "sparks.page.fx" => Some(self.fast_sparks_page_key("fx")),
            "sparks.page.trigger-gate" => Some(self.fast_sparks_page_key("trigger-gate")),
            "sparks.page.transpose" => Some(self.fast_sparks_page_key("transpose")),
            "sparks.page.xy" => Some(self.fast_sparks_page_key("xy")),
            "algorithmStep" => Some(self.fast_algorithm_step_menu_key()),
            "masterVolume" => Some(self.fast_master_volume_menu_key()),
            "displayBrightness" => Some(self.fast_display_brightness_menu_key()),
            "buttonBrightness" => Some(self.fast_button_brightness_menu_key()),
            "gridBrightness" => Some(self.fast_number_menu_key(key, |runner, value| {
                let value = value.clamp(10, 100) as u8;
                value_changed(&mut runner.display.ui.grid_brightness, value)
            })),
            "numericDisplayMode" => Some(self.fast_string_menu_key(key, |runner, value| {
                value_changed(&mut runner.display.ui.numeric_display_mode, value)
            })),
            "screenSleepSeconds" => Some(self.fast_number_menu_key(key, |runner, value| {
                let value = value.clamp(0, 600) as u16;
                value_changed(&mut runner.display.ui.screen_sleep_seconds, value)
            })),
            "dimTimerSeconds" => Some(self.fast_number_menu_key(key, |runner, value| {
                let value = value.clamp(0, 600) as u16;
                value_changed(&mut runner.display.ui.dim_timer_seconds, value)
            })),
            "ghostCells" => Some(self.fast_bool_menu_key(key, |runner, value| {
                bool_changed(&mut runner.display.ui.ghost_cells, value)
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
        let bpm = crate::delay_timing::clamp_visible_bpm(f64::from(bpm));
        if (self.transport.bpm - bpm).abs() > f64::EPSILON {
            self.transport.bpm = bpm;
            self.retime_note_mode_bus_delays();
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_swing_menu_key(&mut self) -> bool {
        let Some(swing_pct) = self.menu.number_for_key("transport.swingPct") else {
            return false;
        };
        let swing_pct = swing_pct.clamp(0, 75) as u8;
        if self.transport.swing_pct != swing_pct {
            self.transport.swing_pct = swing_pct;
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_sync_source_menu_key(&mut self) -> bool {
        let Some(sync_source) = self.menu.selected_sync_source() else {
            return false;
        };
        if self.transport.sync_source != sync_source {
            self.transport.sync_source = sync_source;
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_usb_audio_out_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("usb.audioOut") else {
            return false;
        };
        let value = super::normalize_usb_audio_out(&value).to_string();
        if value_changed(&mut self.usb_audio_out, value) {
            self.mark_fast_autosave_dirty();
            self.show_toast("USB: Save & Reboot");
        }
        true
    }

    fn fast_usb_midi_out_menu_key(&mut self) -> bool {
        let Some(value) = self
            .menu
            .value_for_key("usb.midiOutEnabled")
            .map(|value| value == "true")
        else {
            return false;
        };
        if value_changed(&mut self.usb_midi_out_enabled, value) {
            self.mark_fast_autosave_dirty();
            self.show_toast("USB: Save & Reboot");
        }
        true
    }

    fn fast_recording_max_minutes_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("recording.maxMinutes") else {
            return false;
        };
        let value = value.parse::<u16>().unwrap_or(10).clamp(1, 120);
        if value_changed(&mut self.recording_max_minutes, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_display_brightness_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.selected_display_brightness() else {
            return false;
        };
        if value_changed(&mut self.display.ui.display_brightness, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_button_brightness_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.selected_button_brightness() else {
            return false;
        };
        if value_changed(&mut self.display.ui.button_brightness, value) {
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
            self.pending.pending_audio_output_buffer_reboot_prompt = true;
            self.mark_fast_autosave_dirty();
            self.show_toast("Restart device to apply");
        }
        true
    }

    fn fast_sparks_mode_menu_key(&mut self) -> bool {
        let Some(sparks_mode) = self.menu.selected_sparks_mode() else {
            return false;
        };
        let changed = self.sparks_mode != sparks_mode;
        if changed {
            self.sparks_mode = sparks_mode.clone();
            if self.menu.is_in_sparks_root_group() {
                self.active_sparks_mode = sparks_mode;
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_sparks_page_key(&mut self, sparks_mode: &str) -> bool {
        let changed = self.sparks_mode != sparks_mode;
        if changed {
            self.sparks_mode = sparks_mode.into();
            self.mark_fast_autosave_dirty();
        }
        if self.menu.is_in_sparks_root_group() {
            self.active_sparks_mode = self.sparks_mode.clone();
        }
        true
    }

    fn fast_algorithm_step_menu_key(&mut self) -> bool {
        let Some(step_pulses) = self.menu.selected_algorithm_step_pulses() else {
            return false;
        };
        let changed = self.transport.algorithm_step_pulses != step_pulses
            || self
                .transport
                .layer_algorithm_step_pulses
                .get(self.active_layer_index)
                .copied()
                .unwrap_or(self.transport.algorithm_step_pulses)
                != step_pulses;
        if changed {
            self.transport.algorithm_step_pulses = step_pulses;
            if let Some(layer_step) = self
                .transport
                .layer_algorithm_step_pulses
                .get_mut(self.active_layer_index)
            {
                *layer_step = step_pulses;
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    fn fast_master_volume_menu_key(&mut self) -> bool {
        let Some(master_volume) = self.menu.selected_master_volume() else {
            return false;
        };
        if self.display.ui.master_volume != master_volume {
            self.display.ui.master_volume = master_volume;
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
