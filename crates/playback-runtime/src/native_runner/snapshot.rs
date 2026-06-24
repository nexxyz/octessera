use crate::protocol::{
    RunnerMessage, RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState, RuntimeUiPulse,
};

use super::{
    clip_display_line, display_index, json, sample_assignments_payload, scrolled_toast,
    velocity_curve_id, GridInteraction, LedColor, NativeOledMode, NativeRunner, NativeToast,
    RuntimeTransportState, SyncSource, Value, GRID_HEIGHT, GRID_WIDTH, OLED_BODY_ROWS,
    PAN_POSITION_COUNT,
};

impl NativeRunner {
    pub(super) fn trigger_ui_pulse_message(&self) -> RunnerMessage {
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TriggerPulse { duration_ms: 45 },
        }
    }

    pub(super) fn transport_ui_pulse_message(&self) -> Option<RunnerMessage> {
        if self.transport_flash_pulses_remaining != 6 {
            return None;
        }
        match self.transport_flash {
            "measure" | "beat" => Some(RunnerMessage::UiPulse {
                pulse: RuntimeUiPulse::TransportFlash {
                    flash: self.transport_flash.into(),
                    duration_ms: 90,
                },
            }),
            _ => None,
        }
    }

    pub(super) fn snapshot(&self) -> Result<Value, String> {
        self.snapshot_with_audio_config(true)
    }

    fn audio_snapshot_payload(&self) -> Value {
        json!({
            "instruments": self.instruments.iter().map(|instrument| {
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
            }).collect::<Vec<_>>(),
            "mixer": self.mixer_payload(),
            "panPositions": PAN_POSITION_COUNT,
        })
    }

    fn snapshot_with_audio_config(&self, include_audio_config: bool) -> Result<Value, String> {
        let model = self.engine.model()?;
        let menu = self.menu.snapshot();
        let mut leds = self.base_led_snapshot(&model);
        self.apply_scan_progress_overlay(&mut leds);
        self.apply_sample_assignment_overlay(&mut leds);
        self.apply_trigger_probability_overlay(&mut leds);
        self.apply_dance_overlay(&mut leds);
        self.apply_param_mod_overlay(&mut leds);
        self.apply_fn_overlay(&mut leds);
        let led_values = leds.into_iter().map(LedColor::to_value).collect::<Vec<_>>();
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
                "cells": led_values
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
                "auxAutoMapEnabled": self.aux_auto_map_enabled,
                "audioConfigRevision": self.audio_config_revision,
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

    fn base_led_snapshot(&self, model: &platform_core::BehaviorRenderModel) -> Vec<LedColor> {
        let mut leds = vec![LedColor::rgb(15, 15, 22); GRID_WIDTH * GRID_HEIGHT];
        for (logical_index, alive) in model.cells.iter().enumerate() {
            let x = logical_index % GRID_WIDTH;
            let y = logical_index / GRID_WIDTH;
            let display_index = display_index(x, y);
            let trigger = model
                .trigger_types
                .as_ref()
                .and_then(|types| types.get(logical_index))
                .copied();
            leds[display_index] = base_led_color(*alive, trigger);
        }
        leds
    }

    fn display_snapshot(&self, menu: crate::native_menu::NativeMenuSnapshot) -> DisplaySnapshot {
        let mut display = if let Some(confirm) = &self.confirm_dialog {
            confirm_dialog_display(confirm)
        } else if let Some(help) = &self.help_popup {
            help_popup_display(help)
        } else if let Some((title, lines)) = self.aux_mapping_overlay() {
            overlay_display(title, lines)
        } else {
            menu_display(self, menu)
        };
        display.title = clip_display_line(&display.title, 28);
        display.lines = display
            .lines
            .into_iter()
            .take(OLED_BODY_ROWS)
            .map(|line| clip_display_line(&line, 28))
            .collect();
        display.colors.truncate(display.lines.len());
        display.bar_values.truncate(display.lines.len());
        display
    }

    pub(super) fn next_snapshot(&mut self) -> Result<Value, String> {
        let include_audio_config =
            self.last_snapshot_audio_config_revision != Some(self.audio_config_revision);
        let snapshot = self.snapshot_with_audio_config(include_audio_config)?;
        if include_audio_config {
            self.last_snapshot_audio_config_revision = Some(self.audio_config_revision);
        }
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
        if self.oled_mode == NativeOledMode::Splash
            && self.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            self.startup_splash_presented = true;
        }
        self.advance_toast_state();
        let snapshot = self.next_snapshot()?;
        let save_default_effect = if self.auto_save_default && self.config_dirty {
            self.config_dirty = false;
            self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
            self.auto_save_flash_pulses_remaining = 8;
            self.toast = Some(NativeToast {
                message: "Saved default".into(),
                offset: 0,
            });
            Some(RuntimePlatformEffect::StoreSaveDefault {
                payload: self.config_payload(),
                mode: Some("deferred".into()),
            })
        } else {
            None
        };
        let mut messages = Vec::with_capacity(4);
        if let Some(effect) = save_default_effect {
            messages.push(RunnerMessage::PlatformEffects {
                effects: vec![effect],
            });
        }
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
            self.event_dot_pulses_remaining = 1;
            messages.push(self.trigger_ui_pulse_message());
            messages.push(RunnerMessage::MusicalEvents { events });
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }
}

struct DisplaySnapshot {
    title: String,
    lines: Vec<String>,
    colors: Vec<u16>,
    bar_values: Vec<Value>,
    scroll: Option<DisplayScrollMetadata>,
    selected_row: Option<usize>,
}

struct DisplayScrollMetadata {
    scroll_offset: usize,
    total_rows: usize,
    visible_rows: usize,
}

fn base_led_color(alive: bool, trigger: Option<platform_core::CellTriggerType>) -> LedColor {
    if !alive {
        return LedColor::rgb(15, 15, 22);
    }
    match trigger.unwrap_or(platform_core::CellTriggerType::Stable) {
        platform_core::CellTriggerType::Activate => LedColor::rgb(255, 255, 255),
        platform_core::CellTriggerType::Deactivate => LedColor::rgb(128, 128, 128),
        platform_core::CellTriggerType::Scanned => LedColor::rgb(255, 0, 0),
        _ => LedColor::rgb(0, 255, 120),
    }
}

fn confirm_dialog_display(confirm: &super::NativeConfirmDialog) -> DisplaySnapshot {
    let mut lines = confirm.lines.clone();
    for (index, option) in confirm.options.iter().enumerate() {
        let marker = if index == confirm.cursor { ">" } else { " " };
        lines.push(format!("{marker} {option}"));
    }
    lines.truncate(OLED_BODY_ROWS);
    let line_count = lines.len();
    DisplaySnapshot {
        title: confirm.title.clone(),
        lines,
        colors: vec![0xFFFF; line_count],
        bar_values: vec![Value::Null; line_count],
        scroll: None,
        selected_row: Some(
            confirm
                .lines
                .len()
                .saturating_add(confirm.cursor)
                .min(OLED_BODY_ROWS.saturating_sub(1)),
        ),
    }
}

fn help_popup_display(help: &super::NativeHelpPopup) -> DisplaySnapshot {
    let mut lines = help
        .lines
        .iter()
        .skip(help.scroll)
        .take(OLED_BODY_ROWS - 1)
        .cloned()
        .collect::<Vec<_>>();
    lines.push("> Close".into());
    let line_count = lines.len();
    DisplaySnapshot {
        title: help.title.clone(),
        lines,
        colors: vec![0xFFFF; line_count],
        bar_values: vec![Value::Null; line_count],
        scroll: None,
        selected_row: Some(
            help.lines
                .len()
                .saturating_sub(help.scroll)
                .min(OLED_BODY_ROWS - 1),
        ),
    }
}

fn overlay_display(title: String, lines: Vec<String>) -> DisplaySnapshot {
    let line_count = lines.len();
    DisplaySnapshot {
        title,
        lines,
        colors: vec![0xFFFF; line_count],
        bar_values: vec![Value::Null; line_count],
        scroll: None,
        selected_row: None,
    }
}

fn menu_display(
    runner: &NativeRunner,
    menu: crate::native_menu::NativeMenuSnapshot,
) -> DisplaySnapshot {
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
    let lines = menu
        .lines
        .into_iter()
        .enumerate()
        .map(|(row, line)| {
            let prefix = runner.auto_map_prefix_for_line(
                menu.line_keys.get(row).and_then(|key| key.as_deref()),
                menu.line_actions
                    .get(row)
                    .and_then(|action| action.as_ref()),
            );
            prefix_line(line, prefix)
        })
        .collect();
    DisplaySnapshot {
        title: menu.path,
        lines,
        colors: menu.colors,
        bar_values,
        scroll: menu.scroll.map(|scroll| DisplayScrollMetadata {
            scroll_offset: scroll.scroll_offset,
            total_rows: scroll.total_rows,
            visible_rows: scroll.visible_rows,
        }),
        selected_row: menu.selected_row,
    }
}

fn prefix_line(line: String, prefix: Option<String>) -> String {
    let Some(prefix) = prefix else {
        return line;
    };
    if let Some(stripped) = line.strip_prefix('!') {
        return format!("{prefix}{stripped}");
    }
    let indent = line.chars().take_while(|ch| *ch == ' ').count();
    let body = &line[indent..];
    if prefix.ends_with('!') {
        let stripped = body.strip_prefix("> ").unwrap_or(body);
        let stripped = stripped.strip_prefix('!').unwrap_or(stripped);
        return format!("{prefix}{stripped}");
    }
    if let Some(stripped) = body.strip_prefix("> ") {
        return format!("{}> {}{}", " ".repeat(indent), prefix, stripped);
    }
    format!("{}{}{}", " ".repeat(indent), prefix, body)
}
