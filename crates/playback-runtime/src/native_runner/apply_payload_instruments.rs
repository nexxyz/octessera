use crate::protocol::RuntimePlatformEffect;

use super::aux_binding_payload_apply::apply_aux_bindings_payload;
use super::{
    default_mapping_config, derive_bus_name, derive_instrument_name, sample_assignment_from_payload,
    sanitize_pan_position_payload, velocity_curve_from_id, NativeFxBus, NativeInstrumentSlot,
    NativeRunner, SyncSource, Value, SAMPLE_SLOT_COUNT,
};

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
        apply_mixer_payload(runtime, &mut self.fx_buses, &mut self.global_fx_slots, &mut self.global_fx_params);
        apply_sound_payload(runtime, &mut self.ui.master_volume, &mut self.global_sound.note_length_ms, &mut self.global_sound.velocity_scale_pct, &mut self.global_sound.velocity_curve, &mut self.voice_stealing_mode);
        apply_ui_payload(
            runtime,
            &mut self.ui.display_brightness,
            &mut self.ui.grid_brightness,
            &mut self.ui.button_brightness,
            &mut self.input_events_while_paused,
            &mut self.ui.numeric_display_mode,
            &mut self.ui.screen_sleep_seconds,
            &mut self.aux_auto_map_enabled,
            &mut self.aux_bindings,
            &mut self.bpm,
            &mut self.dance_mode,
            &mut self.active_dance_mode,
        );
        apply_midi_payload(
            runtime,
            &mut self.midi_enabled,
            &mut self.selected_midi_output_id,
            &mut self.selected_midi_input_id,
            &mut self.sync_source,
            &mut self.midi_clock_out_enabled,
            &mut self.midi_clock_in_enabled,
            &mut self.midi_respond_to_start_stop,
            &mut self.queued_platform_effects,
        );
        if let Some(mapping_config) = payload.get("mappingConfig") {
            self.base_mapping_config = serde_json::from_value(mapping_config.clone())
                .unwrap_or_else(|_| default_mapping_config());
            self.mapping_config = self.base_mapping_config.clone();
        }
    }
}

fn apply_instrument_identity_payload(
    slot: &Value,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) {
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
}

fn apply_instrument_mixer_payload(
    slot: &Value,
    incoming_pan_positions: Option<u64>,
    instrument: &mut NativeInstrumentSlot,
) {
    let Some(mixer) = slot.get("mixer") else {
        return;
    };
    if let Some(volume) = mixer.get("volume").and_then(Value::as_u64) {
        instrument.volume = (volume as u8).min(127);
    }
    if let Some(pan_pos) = mixer.get("panPos").and_then(Value::as_u64) {
        instrument.pan_pos = sanitize_pan_position_payload(pan_pos, incoming_pan_positions);
    }
    if let Some(route) = mixer.get("route").and_then(Value::as_str) {
        instrument.route = super::normalize_route(route);
    }
}

fn apply_instrument_sample_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    let Some(sample) = slot.get("sample") else {
        return;
    };
    if let Some(selected_slot) = sample.get("selectedSlot").and_then(Value::as_u64) {
        instrument.selected_sample_slot = (selected_slot as usize).min(SAMPLE_SLOT_COUNT - 1);
    }
    if let Some(base_velocity) = sample.get("baseVelocity").and_then(Value::as_u64) {
        instrument.sample_base_velocity = (base_velocity as u8).clamp(1, 127);
    }
    if let Some(slots) = sample.get("slots").and_then(Value::as_array) {
        for (sample_index, sample_slot) in slots.iter().take(SAMPLE_SLOT_COUNT).enumerate() {
            instrument.sample_paths[sample_index] = sample_slot
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
    if let Some(assignments) = sample.get("assignments").and_then(Value::as_array) {
        instrument.sample_assignments = assignments
            .iter()
            .filter_map(sample_assignment_from_payload)
            .collect();
    }
    if let Some(tune) = sample.get("tuneSemis").and_then(Value::as_i64) {
        instrument.sample_tune_semis = (tune as i8).clamp(-24, 24);
    }
    if let Some(gain) = nested_u64(sample, &["amp", "gainPct"]) {
        instrument.sample_gain_pct = (gain as u8).min(100);
    }
    if let Some(velocity_sens) = nested_u64(sample, &["amp", "velocitySensitivityPct"]) {
        instrument.sample_amp_velocity_sensitivity_pct = (velocity_sens as u8).min(100);
    }
    if let Some(amp_env) = sample.get("ampEnv").filter(|value| value.is_object()) {
        instrument.sample_amp_env = amp_env.clone();
    }
    if let Some(filter) = sample.get("filter").filter(|value| value.is_object()) {
        instrument.sample_filter = filter.clone();
    }
    if let Some(filter_env) = sample.get("filterEnv").filter(|value| value.is_object()) {
        instrument.sample_filter_env = filter_env.clone();
    }
    if let Some(enabled) = sample.get("velocityLevelsEnabled").and_then(Value::as_bool) {
        instrument.sample_velocity_levels_enabled = enabled;
    }
    if let Some(levels) = sample.get("velocityLevels") {
        apply_sample_velocity_levels(levels, instrument);
    }
}

fn apply_sample_velocity_levels(levels: &Value, instrument: &mut NativeInstrumentSlot) {
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

fn apply_instrument_synth_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    let Some(synth) = slot.get("synth") else {
        return;
    };
    instrument.synth_config = synth.clone();
    if let Some(gain) = nested_u64(synth, &["amp", "gainPct"]) {
        instrument.synth_gain_pct = (gain as u8).min(100);
    }
}

fn apply_instrument_midi_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(midi) = slot.get("midi") {
        apply_midi_value_block(midi, true, instrument);
    }
    if let Some(midi_engine) = slot.get("midiEngine") {
        apply_midi_value_block(midi_engine, false, instrument);
    }
}

fn apply_midi_value_block(value: &Value, allow_enabled: bool, instrument: &mut NativeInstrumentSlot) {
    if allow_enabled {
        if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
            instrument.midi_enabled = enabled;
        }
    }
    if let Some(channel) = value.get("channel").and_then(Value::as_u64) {
        instrument.midi_channel = (channel as u8).clamp(1, 16);
    }
    if let Some(velocity) = value.get("velocity").and_then(Value::as_u64) {
        instrument.midi_velocity = (velocity as u8).clamp(1, 127);
    }
    if let Some(duration_ms) = value.get("durationMs").and_then(Value::as_u64) {
        instrument.midi_duration_ms = (duration_ms as u16).clamp(10, 5000);
    }
}

fn apply_mixer_payload(
    runtime: &Value,
    fx_buses: &mut [NativeFxBus],
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
) {
    let Some(mixer) = runtime.get("mixer") else {
        return;
    };
    if let Some(buses) = mixer.get("buses").and_then(Value::as_array) {
        for (index, payload) in buses.iter().take(fx_buses.len()).enumerate() {
            if let Some(bus) = fx_buses.get_mut(index) {
                apply_fx_bus_payload(payload, bus);
            }
        }
    }
    if let Some(slots) = mixer
        .get("master")
        .and_then(|master| master.get("slots"))
        .and_then(Value::as_array)
    {
        for (index, payload) in slots.iter().take(global_fx_slots.len()).enumerate() {
            apply_global_fx_payload(payload, index, global_fx_slots, global_fx_params);
        }
    }
}

fn apply_fx_bus_payload(payload: &Value, bus: &mut NativeFxBus) {
    apply_fx_bus_slot_payload(payload.get("slot1"), &mut bus.slot1_type, &mut bus.slot1_params);
    apply_fx_bus_slot_payload(payload.get("slot2"), &mut bus.slot2_type, &mut bus.slot2_params);
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

fn apply_fx_bus_slot_payload(slot: Option<&Value>, slot_type: &mut String, slot_params: &mut Value) {
    let Some(slot) = slot else {
        return;
    };
    if let Some(kind) = slot.get("type").and_then(Value::as_str) {
        *slot_type = if crate::native_menu::is_valid_fx_bus_slot_type(kind) {
            kind.into()
        } else {
            "none".into()
        };
    }
    if let Some(params) = slot.get("params").filter(|params| params.is_object()) {
        *slot_params = params.clone();
    }
}

fn apply_global_fx_payload(
    payload: &Value,
    index: usize,
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
) {
    if let Some(slot_type) = payload.get("type").and_then(Value::as_str) {
        global_fx_slots[index] = if crate::native_menu::is_valid_global_fx_slot_type(slot_type) {
            slot_type.into()
        } else {
            "none".into()
        };
    }
    if let Some(params) = payload.get("params").filter(|params| params.is_object()) {
        if let Some(target) = global_fx_params.get_mut(index) {
            *target = params.clone();
        }
    }
}

fn apply_sound_payload(
    runtime: &Value,
    master_volume: &mut u8,
    note_length_ms: &mut u32,
    velocity_scale_pct: &mut u16,
    velocity_curve: &mut platform_core::VelocityCurve,
    voice_stealing_mode: &mut String,
) {
    if let Some(master) = runtime.get("masterVolume").and_then(Value::as_u64) {
        *master_volume = (master as u8).min(100);
    }
    let sound = runtime.get("sound");
    if let Some(value) = sound_or_runtime_u64(sound, runtime, "noteLengthMs") {
        *note_length_ms = (value as u32).clamp(30, 2000);
    }
    if let Some(value) = sound_or_runtime_u64(sound, runtime, "velocityScalePct") {
        *velocity_scale_pct = (value as u16).min(200);
    }
    if let Some(value) = sound_or_runtime_str(sound, runtime, "velocityCurve") {
        *velocity_curve = velocity_curve_from_id(value);
    }
    if let Some(value) = sound_or_runtime_str(sound, runtime, "voiceStealingMode") {
        if matches!(value, "off" | "lenient" | "balanced" | "aggressive") {
            *voice_stealing_mode = value.into();
        }
    }
}

fn apply_ui_payload(
    runtime: &Value,
    display_brightness: &mut u8,
    grid_brightness: &mut u8,
    button_brightness: &mut u8,
    input_events_while_paused: &mut bool,
    numeric_display_mode: &mut String,
    screen_sleep_seconds: &mut u16,
    aux_auto_map_enabled: &mut bool,
    aux_bindings: &mut [Option<super::NativeAuxBinding>],
    bpm: &mut f64,
    dance_mode: &mut String,
    active_dance_mode: &mut String,
) {
    if let Some(value) = runtime.get("displayBrightness").and_then(Value::as_u64) {
        *display_brightness = (value as u8).min(100);
    }
    if let Some(value) = runtime.get("gridBrightness").and_then(Value::as_u64) {
        *grid_brightness = (value as u8).min(100);
    }
    if let Some(value) = runtime.get("buttonBrightness").and_then(Value::as_u64) {
        *button_brightness = (value as u8).min(100);
    }
    if let Some(value) = runtime.get("inputEventsWhilePaused").and_then(Value::as_bool) {
        *input_events_while_paused = value;
    }
    if let Some(value) = runtime.get("numericDisplayMode").and_then(Value::as_str) {
        if matches!(value, "bar" | "numbers" | "bar+numbers") {
            *numeric_display_mode = value.into();
        }
    }
    if let Some(value) = runtime.get("screenSleepSeconds").and_then(Value::as_u64) {
        *screen_sleep_seconds = (value as u16).min(600);
    }
    if let Some(value) = runtime.get("auxAutoMapEnabled").and_then(Value::as_bool) {
        *aux_auto_map_enabled = value;
    }
    if let Some(value) = runtime.get("auxBindings") {
        apply_aux_bindings_payload(aux_bindings, value);
    }
    if let Some(value) = runtime.get("bpm").and_then(Value::as_f64) {
        *bpm = value.clamp(20.0, 300.0);
    }
    if let Some(value) = runtime.get("danceMode").and_then(Value::as_str) {
        let normalized = match value {
            "mix" | "pan" | "fx" | "trigger-gate" | "xy" => Some(value),
            "none" => Some("mix"),
            _ => None,
        };
        if let Some(value) = normalized {
            *dance_mode = value.into();
            *active_dance_mode = "none".into();
        }
    }
}

fn apply_midi_payload(
    runtime: &Value,
    midi_enabled: &mut bool,
    selected_midi_output_id: &mut Option<String>,
    selected_midi_input_id: &mut Option<String>,
    sync_source: &mut SyncSource,
    midi_clock_out_enabled: &mut bool,
    midi_clock_in_enabled: &mut bool,
    midi_respond_to_start_stop: &mut bool,
    queued_platform_effects: &mut Vec<RuntimePlatformEffect>,
) {
    let Some(midi) = runtime.get("midi") else {
        return;
    };
    if let Some(enabled) = midi.get("enabled").and_then(Value::as_bool) {
        *midi_enabled = enabled;
    }
    if let Some(out_id) = midi.get("outId") {
        *selected_midi_output_id = out_id.as_str().map(str::to_string);
    }
    if let Some(in_id) = midi.get("inId") {
        *selected_midi_input_id = in_id.as_str().map(str::to_string);
    }
    if let Some(sync_mode) = midi.get("syncMode").and_then(Value::as_str) {
        *sync_source = if sync_mode == "external" {
            SyncSource::External
        } else {
            SyncSource::Internal
        };
    }
    if let Some(value) = midi.get("clockOutEnabled").and_then(Value::as_bool) {
        *midi_clock_out_enabled = value;
    }
    if let Some(value) = midi.get("clockInEnabled").and_then(Value::as_bool) {
        *midi_clock_in_enabled = value;
    }
    if let Some(value) = midi.get("respondToStartStop").and_then(Value::as_bool) {
        *midi_respond_to_start_stop = value;
    }
    queued_platform_effects.push(RuntimePlatformEffect::MidiSelectOutput {
        id: if *midi_enabled {
            selected_midi_output_id.clone()
        } else {
            None
        },
    });
    queued_platform_effects.push(RuntimePlatformEffect::MidiSelectInput {
        id: if *midi_enabled {
            selected_midi_input_id.clone()
        } else {
            None
        },
    });
}

fn nested_u64<'a>(value: &'a Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_u64()
}

fn sound_or_runtime_u64(sound: Option<&Value>, runtime: &Value, key: &str) -> Option<u64> {
    sound
        .and_then(|sound| sound.get(key))
        .or_else(|| runtime.get(key))
        .and_then(Value::as_u64)
}

fn sound_or_runtime_str<'a>(sound: Option<&'a Value>, runtime: &'a Value, key: &str) -> Option<&'a str> {
    sound
        .and_then(|sound| sound.get(key))
        .or_else(|| runtime.get(key))
        .and_then(Value::as_str)
}
