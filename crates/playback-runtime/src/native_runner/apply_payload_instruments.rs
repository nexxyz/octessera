use crate::protocol::RuntimePlatformEffect;

use super::apply_payload_instrument_values::*;
use super::apply_payload_mixer_values::*;
use super::aux_binding_payload_apply::apply_aux_bindings_payload;
use super::{default_mapping_config, velocity_curve_from_id, NativeRunner, SyncSource, Value};

impl NativeRunner {
    pub(super) fn apply_instruments_payload(&mut self, runtime: &Value) {
        let incoming_pan_positions = runtime.get("panPositions").and_then(Value::as_u64);
        let Some(instruments) = runtime.get("instruments").and_then(Value::as_array) else {
            return;
        };
        for (index, slot) in instruments.iter().take(self.instruments.len()).enumerate() {
            let Some(instrument) = self.instruments.get_mut(index) else {
                continue;
            };
            apply_instrument_identity_payload(slot, index, instrument);
            apply_instrument_mixer_payload(slot, incoming_pan_positions, instrument);
            apply_instrument_sample_payload(slot, instrument);
            apply_instrument_synth_payload(slot, instrument);
            apply_instrument_midi_payload(slot, instrument);
        }
    }

    pub(super) fn apply_runtime_ui_and_sound_payload(&mut self, runtime: &Value, payload: &Value) {
        apply_mixer_payload(
            runtime,
            &mut self.fx_buses,
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
        );
        self.apply_sound_payload(runtime);
        self.apply_ui_payload(runtime);
        self.apply_midi_payload(runtime);
        if let Some(mapping_config) = payload.get("mappingConfig") {
            self.base_mapping_config = serde_json::from_value(mapping_config.clone())
                .unwrap_or_else(|_| default_mapping_config());
            self.mapping_config = self.base_mapping_config.clone();
        }
    }

    fn apply_sound_payload(&mut self, runtime: &Value) {
        if let Some(master) = runtime.get("masterVolume").and_then(Value::as_u64) {
            self.ui.master_volume = (master as u8).min(100);
        }
        let sound = runtime.get("sound");
        if let Some(value) = sound_or_runtime_u64(sound, runtime, "noteLengthMs") {
            self.global_sound.note_length_ms = (value as u32).clamp(30, 2000);
        }
        if let Some(value) = sound_or_runtime_u64(sound, runtime, "velocityScalePct") {
            self.global_sound.velocity_scale_pct = (value as u16).min(200);
        }
        if let Some(value) = sound_or_runtime_str(sound, runtime, "velocityCurve") {
            self.global_sound.velocity_curve = velocity_curve_from_id(value);
        }
        if let Some(value) = sound_or_runtime_str(sound, runtime, "voiceStealingMode") {
            if let Some(mode) = super::normalize_voice_stealing_mode(value) {
                self.voice_stealing_mode = mode.into();
            }
        }
        if let Some(value) = sound_or_runtime_u64(sound, runtime, "audioOutputBufferFrames") {
            self.audio_output_buffer_frames =
                super::normalize_audio_output_buffer_frames(value as u32);
        }
    }

    fn apply_ui_payload(&mut self, runtime: &Value) {
        self.apply_display_payload(runtime);
        self.apply_runtime_input_payload(runtime);
        self.apply_aux_mapping_payload(runtime);
        self.apply_runtime_transport_payload(runtime);
    }

    fn apply_display_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime.get("displayBrightness").and_then(Value::as_u64) {
            self.ui.display_brightness = (value as u8).min(100);
        }
        if let Some(value) = runtime.get("gridBrightness").and_then(Value::as_u64) {
            self.ui.grid_brightness = (value as u8).min(100);
        }
        if let Some(value) = runtime.get("buttonBrightness").and_then(Value::as_u64) {
            self.ui.button_brightness = (value as u8).min(100);
        }
        if let Some(value) = runtime.get("numericDisplayMode").and_then(Value::as_str) {
            if matches!(value, "bar" | "numbers" | "bar+numbers") {
                self.ui.numeric_display_mode = value.into();
            }
        }
        let screen_sleep_seconds = runtime.get("screenSleepSeconds").and_then(Value::as_u64);
        if let Some(value) = screen_sleep_seconds {
            self.ui.screen_sleep_seconds = (value as u16).min(600);
        }
        if let Some(value) = runtime
            .get("dimTimerSeconds")
            .and_then(Value::as_u64)
            .or(screen_sleep_seconds)
        {
            self.ui.dim_timer_seconds = (value as u16).min(600);
        }
        if let Some(value) = runtime.get("autoSaveDefault").and_then(Value::as_bool) {
            self.auto_save_default = value;
        }
        self.rolling_backups = runtime
            .get("rollingBackups")
            .and_then(Value::as_bool)
            .unwrap_or(true);
    }

    fn apply_runtime_input_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime
            .get("inputEventsWhilePaused")
            .and_then(Value::as_bool)
        {
            self.input_events_while_paused = value;
        }
    }

    fn apply_aux_mapping_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime.get("auxAutoMapEnabled").and_then(Value::as_bool) {
            self.aux_auto_map_enabled = value;
        }
        if let Some(value) = runtime.get("auxBindings") {
            apply_aux_bindings_payload(&mut self.aux_bindings, value);
        }
    }

    fn apply_runtime_transport_payload(&mut self, runtime: &Value) {
        let transport = runtime.get("transport");
        if let Some(value) = transport
            .and_then(|transport| transport.get("bpm"))
            .or_else(|| runtime.get("bpm"))
            .and_then(Value::as_f64)
        {
            self.bpm = value.clamp(40.0, 240.0);
        }
        if let Some(value) = transport
            .and_then(|transport| transport.get("swingPct"))
            .or_else(|| runtime.get("swingPct"))
            .and_then(Value::as_u64)
        {
            self.swing_pct = (value as u8).min(75);
        }
        if let Some(value) = runtime.get("danceMode").and_then(Value::as_str) {
            let normalized = match value {
                "mix" | "pan" | "fx" | "trigger-gate" | "xy" => Some(value),
                "none" => Some("mix"),
                _ => None,
            };
            if let Some(value) = normalized {
                self.dance_mode = value.into();
                self.active_dance_mode = "none".into();
            }
        }
    }

    fn apply_midi_payload(&mut self, runtime: &Value) {
        let Some(midi) = runtime.get("midi") else {
            return;
        };
        self.apply_midi_enabled_payload(midi);
        self.apply_midi_port_payload(midi);
        self.apply_midi_sync_payload(midi);
        self.queue_midi_selection_effects();
    }

    fn apply_midi_enabled_payload(&mut self, midi: &Value) {
        if let Some(enabled) = midi.get("enabled").and_then(Value::as_bool) {
            self.midi_enabled = enabled;
        }
    }

    fn apply_midi_port_payload(&mut self, midi: &Value) {
        if let Some(out_id) = midi.get("outId") {
            self.selected_midi_output_id = out_id.as_str().map(str::to_string);
        }
        if let Some(in_id) = midi.get("inId") {
            self.selected_midi_input_id = in_id.as_str().map(str::to_string);
        }
    }

    fn apply_midi_sync_payload(&mut self, midi: &Value) {
        if let Some(sync_mode) = midi.get("syncMode").and_then(Value::as_str) {
            self.sync_source = if sync_mode == "external" {
                SyncSource::External
            } else {
                SyncSource::Internal
            };
        }
        if let Some(value) = midi.get("clockOutEnabled").and_then(Value::as_bool) {
            self.midi_clock_out_enabled = value;
        }
        if let Some(value) = midi.get("clockInEnabled").and_then(Value::as_bool) {
            self.midi_clock_in_enabled = value;
        }
        if let Some(value) = midi.get("respondToStartStop").and_then(Value::as_bool) {
            self.midi_respond_to_start_stop = value;
        }
    }

    fn queue_midi_selection_effects(&mut self) {
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::MidiSelectOutput {
                id: if self.midi_enabled {
                    self.selected_midi_output_id.clone()
                } else {
                    None
                },
            });
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::MidiSelectInput {
                id: if self.midi_enabled {
                    self.selected_midi_input_id.clone()
                } else {
                    None
                },
            });
    }
}
