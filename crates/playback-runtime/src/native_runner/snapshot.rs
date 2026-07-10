use super::{
    display_index, json, scrolled_toast, velocity_curve_id, GridInteraction, NativeOledMode,
    NativeRunner, RuntimeTransportState, SyncSource, Value, GRID_HEIGHT, GRID_WIDTH,
};

impl NativeRunner {
    pub(super) fn snapshot(&self) -> Result<Value, String> {
        self.snapshot_with_audio_config(true)
    }

    fn snapshot_with_audio_config(&self, include_audio_config: bool) -> Result<Value, String> {
        let model = self.engine.model()?;
        let active_cells = display_active_cells(&model.cells);
        let menu = self.menu.snapshot();
        let mut leds = self.base_led_snapshot(&model);
        self.apply_scan_progress_overlay(&mut leds);
        self.apply_sample_assignment_overlay(&mut leds);
        self.apply_trigger_probability_overlay(&mut leds);
        self.apply_dance_overlay(&mut leds);
        self.apply_param_mod_overlay(&mut leds);
        self.apply_fn_overlay(&mut leds);
        let mut led_rgb = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT * 3);
        for led in leds {
            led.append_rgb(&mut led_rgb);
        }
        let display = self.display_snapshot(menu);
        let toast = self.toast.as_ref().map(scrolled_toast).unwrap_or_default();

        let mut snapshot = json!({
            "display": {
                "page": self.behavior.id(),
                "title": display.title,
                "lines": display.lines,
                "colors": display.colors,
                "barValues": display.bar_values,
                "scrollOffset": display.scroll.as_ref().map(|scroll| scroll.scroll_offset),
                "totalRows": display.scroll.as_ref().map(|scroll| scroll.total_rows),
                "visibleRows": display.scroll.as_ref().map(|scroll| scroll.visible_rows),
                "toast": toast,
                "off": self.oled_mode == NativeOledMode::Off,
                "splash": if self.oled_mode == NativeOledMode::Splash { self.oled_splash_text.clone() } else { String::new() },
                "editing": self.menu.state.editing && self.help_popup.is_none()
            },
            "leds": {
                "width": GRID_WIDTH,
                "height": GRID_HEIGHT,
                "rgb": led_rgb,
                "active": active_cells
            },
            "transport": {
                "playing": self.transport == RuntimeTransportState::Playing,
                "bpm": self.bpm,
                "swingPct": self.swing_pct,
                "tick": self.tick,
                "ppqnPulse": self.current_ppqn_pulse
            },
            "activeBehavior": self.behavior.id(),
            "danceMode": self.dance_mode,
            "activeDanceMode": self.active_dance_mode,
            "gridInteraction": match self.behavior.grid_interaction().unwrap_or(GridInteraction::Paint) {
                GridInteraction::Paint => "paint",
                GridInteraction::Momentary => "momentary",
            },
            "settings": {
                "displayBrightness": self.ui.display_brightness,
                "gridBrightness": self.ui.grid_brightness,
                "buttonBrightness": self.ui.button_brightness,
                "masterVolume": self.ui.master_volume,
                "sound": {
                    "noteLengthMs": self.global_sound.note_length_ms,
                    "velocityScalePct": self.global_sound.velocity_scale_pct,
                    "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                    "voiceStealingMode": self.voice_stealing_mode.clone()
                },
                "noteLengthMs": self.global_sound.note_length_ms,
                "velocityScalePct": self.global_sound.velocity_scale_pct,
                "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                "voiceStealingMode": self.voice_stealing_mode.clone(),
                "ghostCells": self.ui.ghost_cells,
                "inputEventsWhilePaused": self.input_events_while_paused,
                "numericDisplayMode": self.ui.numeric_display_mode,
                "dimTimerSeconds": self.ui.dim_timer_seconds,
                "screenSleepSeconds": self.ui.screen_sleep_seconds,
                "ledsDimmed": self.leds_dimmed(),
                "auxAutoMapEnabled": self.aux_auto_map_enabled,
                "audioConfigRevision": self.audio_config_revision,
                "autoSaveFlash": if self.auto_save_flash_pulses_remaining > 0 { "flash" } else { "none" },
                "autoSaveFlashSerial": self.auto_save_flash_serial,
                "transport": {
                    "bpm": self.bpm,
                    "swingPct": self.swing_pct
                },
                "transportFlash": "none",
                "stopLatched": false,
                "fnHeld": self.ui.fn_held,
                "combinedModifierHeld": self.ui.combined_modifier_held,
                "midi": {
                    "enabled": self.midi_enabled,
                    "outId": self.selected_midi_output_id,
                    "inId": self.selected_midi_input_id,
                    "outputs": self.midi_outputs,
                    "inputs": self.midi_inputs,
                    "status": self.midi_status,
                    "syncMode": match self.sync_source {
                        SyncSource::Internal => "internal",
                        SyncSource::External => "external",
                    },
                    "clockOutEnabled": self.midi_clock_out_enabled,
                    "clockInEnabled": self.midi_clock_in_enabled,
                    "respondToStartStop": self.midi_respond_to_start_stop
                }
            },
            "selectedRow": display.selected_row,
            "voiceStealingMode": self.voice_stealing_mode.clone(),
            "eventDotOn": self.event_dot_on || self.event_dot_pulses_remaining > 0,
            "voiceSteal": false,
            "transportIcon": match self.transport {
                RuntimeTransportState::Playing => "play",
                RuntimeTransportState::Paused => "pause",
                RuntimeTransportState::Stopped => "stop",
            },
            "transportFlash": self.transport_flash,
            "cpuLoadRatio": 0.0
        });
        if include_audio_config {
            if let Some(settings) = snapshot.get_mut("settings").and_then(Value::as_object_mut) {
                let Value::Object(audio) = self.audio_snapshot_payload() else {
                    unreachable!("audio snapshot payload is an object");
                };
                settings.extend(audio);
            }
        }
        snapshot["settings"]["shiftHeld"] = json!(self.ui.shift_held);
        Ok(snapshot)
    }

    pub(super) fn next_snapshot(&mut self) -> Result<Value, String> {
        let include_audio_config =
            self.last_snapshot_audio_config_revision != Some(self.audio_config_revision);
        let snapshot = self.snapshot_with_audio_config(include_audio_config)?;
        self.queue_audio_config_if_changed();
        Ok(snapshot)
    }

    pub(super) fn queue_audio_config_if_changed(&mut self) {
        if self.last_snapshot_audio_config_revision != Some(self.audio_config_revision) {
            self.queue_audio_command(self.full_audio_config_command());
            self.last_snapshot_audio_config_revision = Some(self.audio_config_revision);
        }
    }
}

fn display_active_cells(cells: &[bool]) -> Vec<bool> {
    let mut active = vec![false; GRID_WIDTH * GRID_HEIGHT];
    for (logical_index, alive) in cells.iter().enumerate() {
        let x = logical_index % GRID_WIDTH;
        let y = logical_index / GRID_WIDTH;
        active[display_index(x, y)] = *alive;
    }
    active
}
