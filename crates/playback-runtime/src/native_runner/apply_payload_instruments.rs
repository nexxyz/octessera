use crate::protocol::RuntimePlatformEffect;

use super::aux_binding_payload_apply::apply_aux_bindings_payload;
use super::{
    default_mapping_config, derive_bus_name, derive_instrument_name,
    sample_assignment_from_payload, sanitize_pan_position_payload, velocity_curve_from_id,
    NativeRunner, SyncSource, Value, SAMPLE_SLOT_COUNT,
};

impl NativeRunner {
    pub(super) fn apply_instruments_payload(&mut self, runtime: &Value) {
        let incoming_pan_positions = runtime.get("panPositions").and_then(Value::as_u64);
        if let Some(instruments) = runtime.get("instruments").and_then(Value::as_array) {
            for (index, slot) in instruments.iter().take(self.instruments.len()).enumerate() {
                if let Some(instrument) = self.instruments.get_mut(index) {
                    if let Some(kind) = slot.get("type").and_then(Value::as_str) {
                        if matches!(kind, "none" | "synth" | "sampler" | "midi") {
                            instrument.kind = kind.into();
                        }
                    }
                    if let Some(note_behavior) = slot.get("noteBehavior").and_then(Value::as_str) {
                        if matches!(note_behavior, "oneshot" | "hold") {
                            instrument.note_behavior = note_behavior.into();
                        }
                    }
                    if let Some(auto_name) = slot.get("autoName").and_then(Value::as_bool) {
                        instrument.auto_name = auto_name;
                    }
                    if let Some(name) = slot.get("name").and_then(Value::as_str) {
                        instrument.name = name.into();
                    } else if instrument.auto_name {
                        instrument.name = derive_instrument_name(index, &instrument.kind);
                    }
                    if let Some(mixer) = slot.get("mixer") {
                        if let Some(volume) = mixer.get("volume").and_then(Value::as_u64) {
                            instrument.volume = (volume as u8).min(127);
                        }
                        if let Some(pan_pos) = mixer.get("panPos").and_then(Value::as_u64) {
                            instrument.pan_pos =
                                sanitize_pan_position_payload(pan_pos, incoming_pan_positions);
                        }
                        if let Some(route) = mixer.get("route").and_then(Value::as_str) {
                            instrument.route = super::normalize_route(route);
                        }
                    }
                    if let Some(sample) = slot.get("sample") {
                        if let Some(selected_slot) =
                            sample.get("selectedSlot").and_then(Value::as_u64)
                        {
                            instrument.selected_sample_slot =
                                (selected_slot as usize).min(SAMPLE_SLOT_COUNT - 1);
                        }
                        if let Some(base_velocity) =
                            sample.get("baseVelocity").and_then(Value::as_u64)
                        {
                            instrument.sample_base_velocity = (base_velocity as u8).clamp(1, 127);
                        }
                        if let Some(slots) = sample.get("slots").and_then(Value::as_array) {
                            for (sample_index, sample_slot) in
                                slots.iter().take(SAMPLE_SLOT_COUNT).enumerate()
                            {
                                instrument.sample_paths[sample_index] = sample_slot
                                    .get("path")
                                    .and_then(Value::as_str)
                                    .map(str::to_string);
                            }
                        }
                        if let Some(assignments) =
                            sample.get("assignments").and_then(Value::as_array)
                        {
                            instrument.sample_assignments = assignments
                                .iter()
                                .filter_map(sample_assignment_from_payload)
                                .collect();
                        }
                        if let Some(tune) = sample.get("tuneSemis").and_then(Value::as_i64) {
                            instrument.sample_tune_semis = (tune as i8).clamp(-24, 24);
                        }
                        if let Some(gain) = sample
                            .get("amp")
                            .and_then(|amp| amp.get("gainPct"))
                            .and_then(Value::as_u64)
                        {
                            instrument.sample_gain_pct = (gain as u8).min(100);
                        }
                        if let Some(velocity_sens) = sample
                            .get("amp")
                            .and_then(|amp| amp.get("velocitySensitivityPct"))
                            .and_then(Value::as_u64)
                        {
                            instrument.sample_amp_velocity_sensitivity_pct =
                                (velocity_sens as u8).min(100);
                        }
                        if let Some(amp_env) =
                            sample.get("ampEnv").filter(|value| value.is_object())
                        {
                            instrument.sample_amp_env = amp_env.clone();
                        }
                        if let Some(filter) = sample.get("filter").filter(|value| value.is_object())
                        {
                            instrument.sample_filter = filter.clone();
                        }
                        if let Some(filter_env) =
                            sample.get("filterEnv").filter(|value| value.is_object())
                        {
                            instrument.sample_filter_env = filter_env.clone();
                        }
                        if let Some(enabled) =
                            sample.get("velocityLevelsEnabled").and_then(Value::as_bool)
                        {
                            instrument.sample_velocity_levels_enabled = enabled;
                        }
                        if let Some(levels) = sample.get("velocityLevels") {
                            if let Some(high) = levels.get("high").and_then(Value::as_u64) {
                                instrument.sample_velocity_high = (high as u8).clamp(1, 127);
                            }
                            if let Some(medium) = levels.get("medium").and_then(Value::as_u64) {
                                instrument.sample_velocity_medium = (medium as u8).clamp(1, 127);
                            }
                            if let Some(low) = levels.get("low").and_then(Value::as_u64) {
                                instrument.sample_velocity_low = (low as u8).clamp(1, 127);
                            }
                        }
                    }
                    if let Some(synth) = slot.get("synth") {
                        instrument.synth_config = synth.clone();
                        if let Some(gain) = synth
                            .get("amp")
                            .and_then(|amp| amp.get("gainPct"))
                            .and_then(Value::as_u64)
                        {
                            instrument.synth_gain_pct = (gain as u8).min(100);
                        }
                    }
                    if let Some(midi) = slot.get("midi") {
                        if let Some(enabled) = midi.get("enabled").and_then(Value::as_bool) {
                            instrument.midi_enabled = enabled;
                        }
                        if let Some(channel) = midi.get("channel").and_then(Value::as_u64) {
                            instrument.midi_channel = (channel as u8).clamp(1, 16);
                        }
                        if let Some(velocity) = midi.get("velocity").and_then(Value::as_u64) {
                            instrument.midi_velocity = (velocity as u8).clamp(1, 127);
                        }
                        if let Some(duration_ms) = midi.get("durationMs").and_then(Value::as_u64) {
                            instrument.midi_duration_ms = (duration_ms as u16).clamp(10, 5000);
                        }
                    }
                    if let Some(midi_engine) = slot.get("midiEngine") {
                        if let Some(channel) = midi_engine.get("channel").and_then(Value::as_u64) {
                            instrument.midi_channel = (channel as u8).clamp(1, 16);
                        }
                        if let Some(velocity) = midi_engine.get("velocity").and_then(Value::as_u64)
                        {
                            instrument.midi_velocity = (velocity as u8).clamp(1, 127);
                        }
                        if let Some(duration_ms) =
                            midi_engine.get("durationMs").and_then(Value::as_u64)
                        {
                            instrument.midi_duration_ms = (duration_ms as u16).clamp(10, 5000);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn apply_runtime_ui_and_sound_payload(&mut self, runtime: &Value, payload: &Value) {
        if let Some(mixer) = runtime.get("mixer") {
            if let Some(buses) = mixer.get("buses").and_then(Value::as_array) {
                for (index, payload) in buses.iter().take(self.fx_buses.len()).enumerate() {
                    if let Some(bus) = self.fx_buses.get_mut(index) {
                        if let Some(slot1) = payload
                            .get("slot1")
                            .and_then(|slot| slot.get("type"))
                            .and_then(Value::as_str)
                        {
                            bus.slot1_type = if crate::native_menu::is_valid_fx_bus_slot_type(slot1)
                            {
                                slot1.into()
                            } else {
                                "none".into()
                            };
                        }
                        if let Some(params) = payload
                            .get("slot1")
                            .and_then(|slot| slot.get("params"))
                            .filter(|params| params.is_object())
                        {
                            bus.slot1_params = params.clone();
                        }
                        if let Some(slot2) = payload
                            .get("slot2")
                            .and_then(|slot| slot.get("type"))
                            .and_then(Value::as_str)
                        {
                            bus.slot2_type = if crate::native_menu::is_valid_fx_bus_slot_type(slot2)
                            {
                                slot2.into()
                            } else {
                                "none".into()
                            };
                        }
                        if let Some(params) = payload
                            .get("slot2")
                            .and_then(|slot| slot.get("params"))
                            .filter(|params| params.is_object())
                        {
                            bus.slot2_params = params.clone();
                        }
                        if let Some(pan_pos) = payload.get("panPos").and_then(Value::as_u64) {
                            bus.pan_pos = (pan_pos as u8).min(32);
                        }
                        if let Some(auto_name) = payload.get("autoName").and_then(Value::as_bool) {
                            bus.auto_name = auto_name;
                        }
                        if let Some(name) = payload.get("name").and_then(Value::as_str) {
                            bus.name = name.into();
                        } else if bus.auto_name {
                            bus.name = derive_bus_name(bus);
                        }
                    }
                }
            }
            if let Some(slots) = mixer
                .get("master")
                .and_then(|master| master.get("slots"))
                .and_then(Value::as_array)
            {
                for (index, payload) in slots.iter().take(self.global_fx_slots.len()).enumerate() {
                    if let Some(slot_type) = payload.get("type").and_then(Value::as_str) {
                        self.global_fx_slots[index] =
                            if crate::native_menu::is_valid_global_fx_slot_type(slot_type) {
                                slot_type.into()
                            } else {
                                "none".into()
                            };
                    }
                    if let Some(params) = payload.get("params").filter(|params| params.is_object())
                    {
                        if let Some(target) = self.global_fx_params.get_mut(index) {
                            *target = params.clone();
                        }
                    }
                }
            }
        }
        if let Some(master_volume) = runtime.get("masterVolume").and_then(Value::as_u64) {
            self.ui.master_volume = (master_volume as u8).min(100);
        }
        let sound = runtime.get("sound");
        if let Some(note_length_ms) = sound
            .and_then(|sound| sound.get("noteLengthMs"))
            .or_else(|| runtime.get("noteLengthMs"))
            .and_then(Value::as_u64)
        {
            self.global_sound.note_length_ms = (note_length_ms as u32).clamp(30, 2000);
        }
        if let Some(velocity_scale_pct) = sound
            .and_then(|sound| sound.get("velocityScalePct"))
            .or_else(|| runtime.get("velocityScalePct"))
            .and_then(Value::as_u64)
        {
            self.global_sound.velocity_scale_pct = (velocity_scale_pct as u16).min(200);
        }
        if let Some(velocity_curve) = sound
            .and_then(|sound| sound.get("velocityCurve"))
            .or_else(|| runtime.get("velocityCurve"))
            .and_then(Value::as_str)
        {
            self.global_sound.velocity_curve = velocity_curve_from_id(velocity_curve);
        }
        if let Some(voice_stealing_mode) = sound
            .and_then(|sound| sound.get("voiceStealingMode"))
            .or_else(|| runtime.get("voiceStealingMode"))
            .and_then(Value::as_str)
        {
            if matches!(
                voice_stealing_mode,
                "off" | "lenient" | "balanced" | "aggressive"
            ) {
                self.voice_stealing_mode = voice_stealing_mode.into();
            }
        }
        if let Some(display_brightness) = runtime.get("displayBrightness").and_then(Value::as_u64) {
            self.ui.display_brightness = (display_brightness as u8).min(100);
        }
        if let Some(grid_brightness) = runtime.get("gridBrightness").and_then(Value::as_u64) {
            self.ui.grid_brightness = (grid_brightness as u8).min(100);
        }
        if let Some(button_brightness) = runtime.get("buttonBrightness").and_then(Value::as_u64) {
            self.ui.button_brightness = (button_brightness as u8).min(100);
        }
        if let Some(input_events_while_paused) = runtime
            .get("inputEventsWhilePaused")
            .and_then(Value::as_bool)
        {
            self.input_events_while_paused = input_events_while_paused;
        }
        if let Some(numeric_display_mode) =
            runtime.get("numericDisplayMode").and_then(Value::as_str)
        {
            if matches!(numeric_display_mode, "bar" | "numbers" | "bar+numbers") {
                self.ui.numeric_display_mode = numeric_display_mode.into();
            }
        }
        if let Some(screen_sleep_seconds) =
            runtime.get("screenSleepSeconds").and_then(Value::as_u64)
        {
            self.ui.screen_sleep_seconds = (screen_sleep_seconds as u16).min(600);
        }
        if let Some(aux_auto_map_enabled) =
            runtime.get("auxAutoMapEnabled").and_then(Value::as_bool)
        {
            self.aux_auto_map_enabled = aux_auto_map_enabled;
        }
        if let Some(aux_bindings) = runtime.get("auxBindings") {
            apply_aux_bindings_payload(&mut self.aux_bindings, aux_bindings);
        }
        if let Some(bpm) = runtime.get("bpm").and_then(Value::as_f64) {
            self.bpm = bpm.clamp(20.0, 300.0);
        }
        if let Some(dance_mode) = runtime.get("danceMode").and_then(Value::as_str) {
            let normalized = match dance_mode {
                "mix" | "pan" | "fx" | "trigger-gate" | "xy" => Some(dance_mode),
                "none" => Some("mix"),
                _ => None,
            };
            if let Some(dance_mode) = normalized {
                self.dance_mode = dance_mode.into();
                self.active_dance_mode = "none".into();
            }
        }
        if let Some(midi) = runtime.get("midi") {
            if let Some(enabled) = midi.get("enabled").and_then(Value::as_bool) {
                self.midi_enabled = enabled;
            }
            if let Some(out_id) = midi.get("outId") {
                self.selected_midi_output_id = out_id.as_str().map(str::to_string);
            }
            if let Some(in_id) = midi.get("inId") {
                self.selected_midi_input_id = in_id.as_str().map(str::to_string);
            }
            if let Some(sync_mode) = midi.get("syncMode").and_then(Value::as_str) {
                self.sync_source = if sync_mode == "external" {
                    SyncSource::External
                } else {
                    SyncSource::Internal
                };
            }
            if let Some(clock_out_enabled) = midi.get("clockOutEnabled").and_then(Value::as_bool) {
                self.midi_clock_out_enabled = clock_out_enabled;
            }
            if let Some(clock_in_enabled) = midi.get("clockInEnabled").and_then(Value::as_bool) {
                self.midi_clock_in_enabled = clock_in_enabled;
            }
            if let Some(respond_to_start_stop) =
                midi.get("respondToStartStop").and_then(Value::as_bool)
            {
                self.midi_respond_to_start_stop = respond_to_start_stop;
            }
            self.queued_platform_effects
                .push(RuntimePlatformEffect::MidiSelectOutput {
                    id: if self.midi_enabled {
                        self.selected_midi_output_id.clone()
                    } else {
                        None
                    },
                });
            self.queued_platform_effects
                .push(RuntimePlatformEffect::MidiSelectInput {
                    id: if self.midi_enabled {
                        self.selected_midi_input_id.clone()
                    } else {
                        None
                    },
                });
        }
        if let Some(mapping_config) = payload.get("mappingConfig") {
            self.base_mapping_config = serde_json::from_value(mapping_config.clone())
                .unwrap_or_else(|_| default_mapping_config());
            self.mapping_config = self.base_mapping_config.clone();
        }
    }
}
