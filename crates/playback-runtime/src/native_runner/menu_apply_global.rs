#![allow(dead_code)]

use super::{velocity_curve_from_id, NativeRunner};

impl NativeRunner {
    pub(super) fn apply_global_runtime_menu_state(&mut self) -> (bool, bool, bool) {
        let mut sparks_mode_changed = false;
        let mut config_changed = false;
        let mut audio_config_changed = false;
        if let Some(sync_source) = self.menu.selected_sync_source() {
            config_changed |= self.sync_source != sync_source;
            self.sync_source = sync_source;
        }
        if let Some(step_pulses) = self.menu.selected_algorithm_step_pulses() {
            config_changed |= self.algorithm_step_pulses != step_pulses;
            self.algorithm_step_pulses = step_pulses;
            if let Some(layer_step) = self
                .layer_algorithm_step_pulses
                .get_mut(self.active_layer_index)
            {
                config_changed |= *layer_step != step_pulses;
                *layer_step = step_pulses;
            }
        }
        if let Some(master_volume) = self.menu.selected_master_volume() {
            config_changed |= self.ui.master_volume != master_volume;
            self.ui.master_volume = master_volume;
        }
        if let Some(draft_name) = self.menu.value_for_key("system.draftName") {
            config_changed |= self.preset_draft_name != draft_name;
            self.preset_draft_name = draft_name;
        }
        config_changed |= self.apply_midi_menu_flags();
        if let Some(sparks_mode) = self.menu.selected_sparks_mode() {
            let changed = self.sparks_mode != sparks_mode;
            self.sparks_mode = sparks_mode.clone();
            sparks_mode_changed = changed;
            config_changed |= changed;
            if changed && self.menu.is_in_sparks_root_group() {
                self.active_sparks_mode = sparks_mode;
            }
        }
        config_changed |= self.apply_xy_menu_state();
        let (ui_sound_changed, sound_audio_config_changed) =
            self.apply_ui_sound_transport_menu_state();
        config_changed |= ui_sound_changed;
        audio_config_changed |= sound_audio_config_changed;
        (sparks_mode_changed, config_changed, audio_config_changed)
    }

    pub(super) fn apply_midi_menu_flags(&mut self) -> bool {
        let mut changed = false;
        if let Some(midi_enabled) = self
            .menu
            .value_for_key("midiEnabled")
            .map(|value| value == "true")
        {
            changed |= self.midi_enabled != midi_enabled;
            self.midi_enabled = midi_enabled;
        }
        if let Some(clock_out_enabled) = self
            .menu
            .value_for_key("midi.clockOutEnabled")
            .map(|value| value == "true")
        {
            changed |= self.midi_clock_out_enabled != clock_out_enabled;
            self.midi_clock_out_enabled = clock_out_enabled;
        }
        if let Some(clock_in_enabled) = self
            .menu
            .value_for_key("midi.clockInEnabled")
            .map(|value| value == "true")
        {
            changed |= self.midi_clock_in_enabled != clock_in_enabled;
            self.midi_clock_in_enabled = clock_in_enabled;
        }
        if let Some(respond_to_start_stop) = self
            .menu
            .value_for_key("midi.respondToStartStop")
            .map(|value| value == "true")
        {
            changed |= self.midi_respond_to_start_stop != respond_to_start_stop;
            self.midi_respond_to_start_stop = respond_to_start_stop;
        }
        changed
    }

    pub(super) fn apply_xy_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(xy_release) = self.menu.value_for_key("sparks.xy.release") {
            changed |= self.xy_release != xy_release;
            self.xy_release = xy_release;
        }
        if let Some(invert_x) = self.menu.value_for_key("sparks.xy.invertX") {
            let invert_x = invert_x == "true";
            changed |= self.xy_invert_x != invert_x;
            self.xy_invert_x = invert_x;
        }
        if let Some(invert_y) = self.menu.value_for_key("sparks.xy.invertY") {
            let invert_y = invert_y == "true";
            changed |= self.xy_invert_y != invert_y;
            self.xy_invert_y = invert_y;
        }
        changed
    }

    pub(super) fn apply_ui_sound_transport_menu_state(&mut self) -> (bool, bool) {
        let mut changed = self.apply_display_menu_state();
        changed |= self.apply_transport_menu_state();
        let (sound_changed, sound_audio_config_changed) = self.apply_sound_menu_state();
        changed |= sound_changed;
        changed |= self.apply_ui_behavior_menu_state();
        (changed, sound_audio_config_changed)
    }

    pub(super) fn apply_display_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(display_brightness) = self.menu.selected_display_brightness() {
            changed |= self.ui.display_brightness != display_brightness;
            self.ui.display_brightness = display_brightness;
        }
        if let Some(button_brightness) = self.menu.selected_button_brightness() {
            changed |= self.ui.button_brightness != button_brightness;
            self.ui.button_brightness = button_brightness;
        }
        if let Some(grid_brightness) = self.menu.number_for_key("gridBrightness") {
            let grid_brightness = grid_brightness.clamp(10, 100) as u8;
            changed |= self.ui.grid_brightness != grid_brightness;
            self.ui.grid_brightness = grid_brightness;
        }
        if let Some(numeric_display_mode) = self.menu.value_for_key("numericDisplayMode") {
            changed |= self.ui.numeric_display_mode != numeric_display_mode;
            self.ui.numeric_display_mode = numeric_display_mode;
        }
        if let Some(screen_sleep_seconds) = self.menu.number_for_key("screenSleepSeconds") {
            let screen_sleep_seconds = screen_sleep_seconds.clamp(0, 600) as u16;
            changed |= self.ui.screen_sleep_seconds != screen_sleep_seconds;
            self.ui.screen_sleep_seconds = screen_sleep_seconds;
        }
        if let Some(dim_timer_seconds) = self.menu.number_for_key("dimTimerSeconds") {
            let dim_timer_seconds = dim_timer_seconds.clamp(0, 600) as u16;
            changed |= self.ui.dim_timer_seconds != dim_timer_seconds;
            self.ui.dim_timer_seconds = dim_timer_seconds;
        }
        changed
    }

    pub(super) fn apply_transport_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(bpm) = self.menu.number_for_key("transport.bpm") {
            let bpm = f64::from(bpm.clamp(40, 240));
            changed |= (self.bpm - bpm).abs() > f64::EPSILON;
            self.bpm = bpm;
        }
        if let Some(swing_pct) = self.menu.number_for_key("transport.swingPct") {
            let swing_pct = swing_pct.clamp(0, 75) as u8;
            changed |= self.swing_pct != swing_pct;
            self.swing_pct = swing_pct;
        }
        changed
    }

    pub(super) fn apply_sound_menu_state(&mut self) -> (bool, bool) {
        let mut changed = false;
        let mut audio_config_changed = false;
        if let Some(note_length_ms) = self.menu.number_for_key("sound.noteLengthMs") {
            let note_length_ms = note_length_ms.clamp(30, 2000) as u32;
            changed |= self.global_sound.note_length_ms != note_length_ms;
            self.global_sound.note_length_ms = note_length_ms;
        }
        if let Some(velocity_scale_pct) = self.menu.number_for_key("sound.velocityScalePct") {
            let velocity_scale_pct = velocity_scale_pct.clamp(0, 200) as u16;
            changed |= self.global_sound.velocity_scale_pct != velocity_scale_pct;
            self.global_sound.velocity_scale_pct = velocity_scale_pct;
        }
        if let Some(velocity_curve) = self.menu.value_for_key("sound.velocityCurve") {
            let velocity_curve = velocity_curve_from_id(&velocity_curve);
            changed |= self.global_sound.velocity_curve != velocity_curve;
            self.global_sound.velocity_curve = velocity_curve;
        }
        if let Some(voice_stealing_mode) = self.menu.value_for_key("sound.voiceStealingMode") {
            if let Some(mode) = super::normalize_voice_stealing_mode(&voice_stealing_mode) {
                let mode = mode.into();
                let voice_changed = self.voice_stealing_mode != mode;
                changed |= voice_changed;
                audio_config_changed |= voice_changed;
                self.voice_stealing_mode = mode;
            }
        }
        if let Some(value) = self.menu.value_for_key("sound.audioOutputBufferFrames") {
            let value = value
                .parse::<u32>()
                .map(super::normalize_audio_output_buffer_frames)
                .unwrap_or(256);
            if self.audio_output_buffer_frames != value {
                changed = true;
                self.audio_output_buffer_frames = value;
                self.pending_audio_output_buffer_reboot_prompt = true;
                self.show_toast("Restart device to apply");
            }
        }
        (changed, audio_config_changed)
    }

    pub(super) fn apply_ui_behavior_menu_state(&mut self) -> bool {
        let mut changed = false;
        if let Some(ghost_cells) = self.menu.value_for_key("ghostCells") {
            let ghost_cells = ghost_cells == "true";
            changed |= self.ui.ghost_cells != ghost_cells;
            self.ui.ghost_cells = ghost_cells;
        }
        if let Some(input_events_while_paused) = self.menu.value_for_key("inputEventsWhilePaused") {
            let input_events_while_paused = input_events_while_paused == "true";
            changed |= self.input_events_while_paused != input_events_while_paused;
            self.input_events_while_paused = input_events_while_paused;
        }
        if let Some(aux_auto_map_enabled) = self.menu.value_for_key("auxAutoMapEnabled") {
            let aux_auto_map_enabled = aux_auto_map_enabled == "true";
            changed |= self.aux_auto_map_enabled != aux_auto_map_enabled;
            self.aux_auto_map_enabled = aux_auto_map_enabled;
        }
        changed
    }
}
