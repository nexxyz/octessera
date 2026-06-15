use crate::protocol::{RunnerMessage, RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState};

use super::{
    clip_display_line, display_index, json, sample_assignments_payload, scrolled_toast,
    velocity_curve_id, GridInteraction, NativeOledMode, NativeRunner, NativeToast,
    RuntimeTransportState, SyncSource, Value, GRID_HEIGHT, GRID_WIDTH, OLED_BODY_ROWS,
    PAN_POSITION_COUNT,
};

impl NativeRunner {
    pub(super) fn snapshot(&self) -> Result<Value, String> {
        let model = self.engine.model()?;
        let menu = self.menu.snapshot();
        let instruments = self
            .instruments
            .iter()
            .map(|instrument| {
                let sample_slots = instrument
                    .sample_paths
                    .iter()
                    .map(|path| json!({ "path": path }))
                    .collect::<Vec<_>>();
                json!({
                    "type": instrument.kind,
                    "noteBehavior": instrument.note_behavior,
                    "autoName": instrument.auto_name,
                    "name": instrument.name,
                    "synth": instrument.synth_config,
                    "sample": {
                        "selectedSlot": instrument.selected_sample_slot,
                        "baseVelocity": instrument.sample_base_velocity,
                        "slots": sample_slots,
                        "assignments": sample_assignments_payload(&instrument.sample_assignments),
                        "tuneSemis": instrument.sample_tune_semis,
                        "amp": {
                            "gainPct": instrument.sample_gain_pct,
                            "velocitySensitivityPct": instrument.sample_amp_velocity_sensitivity_pct
                        },
                        "ampEnv": instrument.sample_amp_env,
                        "filter": instrument.sample_filter,
                        "filterEnv": instrument.sample_filter_env,
                        "velocityLevelsEnabled": instrument.sample_velocity_levels_enabled,
                        "velocityLevels": {
                            "high": instrument.sample_velocity_high,
                            "medium": instrument.sample_velocity_medium,
                            "low": instrument.sample_velocity_low
                        }
                    },
                    "midi": {
                        "enabled": instrument.midi_enabled,
                        "channel": instrument.midi_channel,
                        "velocity": instrument.midi_velocity,
                        "durationMs": instrument.midi_duration_ms
                    },
                    "midiEngine": {
                        "channel": instrument.midi_channel,
                        "velocity": instrument.midi_velocity,
                        "durationMs": instrument.midi_duration_ms
                    },
                    "mixer": {
                        "volume": instrument.volume,
                        "panPos": instrument.pan_pos,
                        "route": instrument.route
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut leds = vec![json!({ "r": 15, "g": 15, "b": 22 }); GRID_WIDTH * GRID_HEIGHT];
        for (logical_index, alive) in model.cells.iter().enumerate() {
            let x = logical_index % GRID_WIDTH;
            let y = logical_index / GRID_WIDTH;
            let display_index = display_index(x, y);
            let trigger = model
                .trigger_types
                .as_ref()
                .and_then(|types| types.get(logical_index))
                .copied();
            leds[display_index] = if !alive {
                json!({ "r": 15, "g": 15, "b": 22 })
            } else {
                match trigger.unwrap_or(platform_core::CellTriggerType::Stable) {
                    platform_core::CellTriggerType::Activate => {
                        json!({ "r": 255, "g": 255, "b": 255 })
                    }
                    platform_core::CellTriggerType::Deactivate => {
                        json!({ "r": 128, "g": 128, "b": 128 })
                    }
                    platform_core::CellTriggerType::Scanned => json!({ "r": 255, "g": 0, "b": 0 }),
                    _ => json!({ "r": 0, "g": 255, "b": 120 }),
                }
            };
        }
        self.apply_scan_progress_overlay(&mut leds);
        self.apply_sample_assignment_overlay(&mut leds);
        self.apply_trigger_probability_overlay(&mut leds);
        self.apply_dance_overlay(&mut leds);
        self.apply_param_mod_overlay(&mut leds);
        self.apply_fn_overlay(&mut leds);

        let (
            display_lines,
            mut display_colors,
            mut display_bar_values,
            selected_row,
            display_title,
        ) = if let Some(confirm) = &self.confirm_dialog {
            let mut lines = confirm.lines.clone();
            for (index, option) in confirm.options.iter().enumerate() {
                let marker = if index == confirm.cursor { ">" } else { " " };
                lines.push(format!("{marker} {option}"));
            }
            lines.truncate(OLED_BODY_ROWS);
            let selected_row = confirm
                .lines
                .len()
                .saturating_add(confirm.cursor)
                .min(OLED_BODY_ROWS.saturating_sub(1));
            let line_count = lines.len();
            (
                lines,
                vec![0xFFFF; line_count],
                vec![Value::Null; line_count],
                Some(selected_row),
                confirm.title.clone(),
            )
        } else if let Some(help) = &self.help_popup {
            let mut lines = help
                .lines
                .iter()
                .skip(help.scroll)
                .take(OLED_BODY_ROWS - 1)
                .cloned()
                .collect::<Vec<_>>();
            lines.push("> Close".into());
            let line_count = lines.len();
            (
                lines,
                vec![0xFFFF; line_count],
                vec![Value::Null; line_count],
                Some(
                    help.lines
                        .len()
                        .saturating_sub(help.scroll)
                        .min(OLED_BODY_ROWS - 1),
                ),
                help.title.clone(),
            )
        } else {
            let bar_values = menu
                .bar_values
                .into_iter()
                .map(|bar| {
                    bar.map(|bar| {
                        json!({
                            "frac": f32::from(bar.frac_pct) / 100.0,
                            "numChars": bar.num_chars,
                            "style": bar.style,
                        })
                    })
                    .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>();
            (
                menu.lines,
                menu.colors,
                bar_values,
                menu.selected_row,
                menu.path,
            )
        };
        let display_title = clip_display_line(&display_title, 28);
        let display_lines = display_lines
            .into_iter()
            .take(OLED_BODY_ROWS)
            .map(|line| clip_display_line(&line, 28))
            .collect::<Vec<_>>();
        display_colors.truncate(display_lines.len());
        display_bar_values.truncate(display_lines.len());
        let toast = self.toast.as_ref().map(scrolled_toast).unwrap_or_default();

        let mut snapshot = json!({
            "display": {
                "page": self.behavior.id(),
                "title": display_title,
                "lines": display_lines,
                "colors": display_colors,
                "barValues": display_bar_values,
                "toast": toast,
                "off": self.oled_mode == NativeOledMode::Off,
                "splash": if self.oled_mode == NativeOledMode::Splash { self.oled_splash_text.clone() } else { String::new() },
                "editing": self.menu.state.editing && self.help_popup.is_none()
            },
            "leds": {
                "width": GRID_WIDTH,
                "height": GRID_HEIGHT,
                "cells": leds
            },
            "transport": {
                "playing": self.transport == RuntimeTransportState::Playing,
                "bpm": self.bpm,
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
                "screenSleepSeconds": self.ui.screen_sleep_seconds,
                "instruments": instruments,
                "mixer": self.mixer_payload(),
                "panPositions": PAN_POSITION_COUNT,
                "autoSaveFlash": if self.auto_save_flash_pulses_remaining > 0 { "flash" } else { "none" },
                "autoSaveFlashSerial": self.auto_save_flash_serial,
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
            "selectedRow": selected_row,
            "voiceStealingMode": self.voice_stealing_mode.clone(),
            "eventDotOn": self.event_dot_on || self.event_dot_pulses_remaining > 0,
            "transportIcon": if self.transport == RuntimeTransportState::Playing { "play" } else { "stop" },
            "transportFlash": self.transport_flash,
            "cpuLoadRatio": 0.0
        });
        snapshot["settings"]["shiftHeld"] = json!(self.ui.shift_held);
        Ok(snapshot)
    }

    pub(super) fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: RuntimeStatusState::Running,
            transport: self.transport.clone(),
            current_ppqn_pulse: self.current_ppqn_pulse,
            pending_resync: self.pending_resync,
            sync_source: self.sync_source.clone(),
            message: None,
        }
    }

    pub(super) fn messages_with_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.advance_oled_sleep_state();
        let snapshot = self.snapshot()?;
        let mut messages = Vec::new();
        if !self.queued_platform_effects.is_empty() {
            messages.push(RunnerMessage::PlatformEffects {
                effects: std::mem::take(&mut self.queued_platform_effects),
            });
        }
        messages.extend([
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus {
                status: self.status(),
            },
        ]);
        if self.auto_save_default && self.config_dirty {
            self.config_dirty = false;
            self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
            self.auto_save_flash_pulses_remaining = 8;
            self.toast = Some(NativeToast {
                message: "Saved default".into(),
                offset: 0,
            });
            messages.insert(
                0,
                RunnerMessage::PlatformEffects {
                    effects: vec![RuntimePlatformEffect::StoreSaveDefault {
                        payload: self.config_payload(),
                        mode: Some("deferred".into()),
                    }],
                },
            );
        }
        if self.auto_save_flash_pulses_remaining > 0 {
            self.auto_save_flash_pulses_remaining -= 1;
        }
        Ok(messages)
    }

    pub(super) fn messages_with_effects(
        &mut self,
        effects: Vec<RuntimePlatformEffect>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = vec![RunnerMessage::PlatformEffects { effects }];
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }

    pub(super) fn messages_with_input_result(
        &mut self,
        result: platform_core::NativeInputResult,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = Vec::new();
        self.apply_runtime_modulation(&result.mapped_intents, self.active_part_index);
        let events = self.apply_sampler_assignments(
            result.events,
            &result.mapped_intents,
            self.active_part_index,
            result.emitted_events.len(),
        );
        if !events.is_empty() {
            self.event_dot_on = true;
            self.event_dot_pulses_remaining = 6;
            messages.push(RunnerMessage::MusicalEvents { events });
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }
}
