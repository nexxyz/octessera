use super::{HdmiGrid, HdmiScene, PresentationScene};
use crate::native_runner::snapshot::hdmi_frame_from_model;
use crate::native_runner::snapshot_audio_settings::audio_payload;
use crate::native_runner::{
    json, RuntimeTransportState, SyncSource, Value, GRID_HEIGHT, GRID_WIDTH,
};

impl PresentationScene {
    pub fn into_snapshot(self) -> Value {
        let hdmi = self.hdmi.into_snapshot(&self.led_rgb, &self.active_cells);
        let display = self.display;
        let bar_values = display
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
        let ui = self.ui;
        let sound = self.sound;
        let transport = self.transport;
        let [back, space, shift, function] = self.neo_key_colors;
        let mut snapshot = json!({
            "display": {
                "page": self.behavior_id,
                "bodyLayout": display.body_layout,
                "title": display.title,
                "lines": display.lines,
                "colors": display.colors,
                "barValues": bar_values,
                "scrollOffset": display.scroll.as_ref().map(|scroll| scroll.scroll_offset),
                "totalRows": display.scroll.as_ref().map(|scroll| scroll.total_rows),
                "visibleRows": display.scroll.as_ref().map(|scroll| scroll.visible_rows),
                "toast": self.toast,
                "off": self.off,
                "splash": self.splash,
                "editing": self.editing
            },
            "leds": {"width": GRID_WIDTH, "height": GRID_HEIGHT, "rgb": self.led_rgb, "active": self.active_cells},
            "hdmi": hdmi,
            "transport": {
                "playing": transport.transport == RuntimeTransportState::Playing,
                "bpm": transport.bpm, "swingPct": transport.swing_pct,
                "tick": transport.tick, "ppqnPulse": transport.current_ppqn_pulse
            },
            "activeBehavior": self.behavior_id,
            "playMode": self.play_mode,
            "activePlayMode": self.active_play_mode,
            "gridInteraction": if self.momentary_grid { "momentary" } else { "paint" },
            "settings": {
                "displayBrightness": ui.display_brightness, "gridBrightness": ui.grid_brightness,
                "buttonBrightness": ui.button_brightness, "masterVolume": ui.master_volume,
                "sound": {
                    "noteLengthMs": sound.note_length_ms,
                    "velocityScalePct": sound.velocity_scale_pct,
                    "velocityCurve": super::super::velocity_curve_id(sound.velocity_curve),
                    "voiceStealingMode": self.voice_stealing_mode
                },
                "noteLengthMs": sound.note_length_ms,
                "velocityScalePct": sound.velocity_scale_pct,
                "velocityCurve": super::super::velocity_curve_id(sound.velocity_curve),
                "voiceStealingMode": self.voice_stealing_mode,
                "ghostCells": ui.ghost_cells,
                "inputEventsWhilePaused": self.input_events_while_paused,
                "numericDisplayMode": ui.numeric_display_mode,
                "dimTimerSeconds": ui.dim_timer_seconds,
                "screenSleepSeconds": ui.screen_sleep_seconds,
                "ledsDimmed": self.leds_dimmed,
                "auxAutoMapEnabled": self.aux_auto_map_enabled,
                "audioConfigRevision": self.audio_config_revision,
                "autoSaveFlash": if self.auto_save_flash { "flash" } else { "none" },
                "autoSaveFlashSerial": self.auto_save_flash_serial,
                "transport": {"bpm": transport.bpm, "swingPct": transport.swing_pct},
                "stopLatched": false,
                "fnHeld": ui.fn_held, "combinedModifierHeld": ui.combined_modifier_held,
                "midi": {
                    "enabled": self.midi_enabled,
                    "outId": self.selected_midi_output_id,
                    "inId": self.selected_midi_input_id,
                    "outputs": self.midi_outputs,
                    "inputs": self.midi_inputs,
                    "status": self.midi_status,
                    "syncMode": match transport.sync_source {
                        SyncSource::Internal => "internal", SyncSource::External => "external"
                    },
                    "clockOutEnabled": self.midi_clock_out_enabled,
                    "clockInEnabled": self.midi_clock_in_enabled,
                    "respondToStartStop": self.midi_respond_to_start_stop
                }
            },
            "selectedRow": display.selected_row,
            "voiceStealingMode": self.voice_stealing_mode,
            "neoKeyLeds": {"back": back, "space": space, "shift": shift, "fn": function},
            "eventDotOn": self.transient.event_dot_on,
            "voiceSteal": false,
            "transportIcon": match transport.transport {
                RuntimeTransportState::Playing => "play",
                RuntimeTransportState::Paused => "pause",
                RuntimeTransportState::Stopped => "stop",
            },
            "transportFlash": self.transient.transport_flash.as_str(),
            "cpuLoadRatio": 0.0
        });
        if let Some(audio) = self.audio {
            let Some(settings) = snapshot.get_mut("settings").and_then(Value::as_object_mut) else {
                unreachable!("scene settings are an object");
            };
            let Value::Object(payload) = audio_payload(
                &audio.instruments,
                &audio.buses,
                &audio.slots,
                &audio.params,
            ) else {
                unreachable!("audio snapshot payload is an object");
            };
            settings.extend(payload);
        }
        snapshot["settings"]["shiftHeld"] = json!(ui.shift_held);
        snapshot
    }
}

impl HdmiScene {
    fn into_snapshot(self, live_rgb: &[u8], live_active: &[bool]) -> Value {
        let (rgb, active) = match self.grid {
            HdmiGrid::Black => (
                vec![0; GRID_WIDTH * GRID_HEIGHT * 3],
                vec![false; GRID_WIDTH * GRID_HEIGHT],
            ),
            HdmiGrid::Live => (live_rgb.to_vec(), live_active.to_vec()),
            HdmiGrid::Model(model) => hdmi_frame_from_model(&model),
        };
        json!({
            "mode": self.mode,
            "showGridlines": self.show_gridlines,
            "cycleMeasures": self.cycle_measures,
            "sourceLayerIndex": self.source_layer_index,
            "sourceBehaviorId": self.source_behavior_id,
            "grid": {"width": GRID_WIDTH, "height": GRID_HEIGHT, "rgb": rgb, "active": active}
        })
    }
}
