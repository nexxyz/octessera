use super::*;

pub(super) fn instrument_labels(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .enumerate()
        .map(|(index, instrument)| format!("I{}: {}", index + 1, instrument.name))
        .collect()
}

pub(super) fn instrument_names(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.name.clone())
        .collect()
}

pub(super) fn instrument_auto_names(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.auto_name)
        .collect()
}

pub(super) fn instrument_note_behaviors(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.note_behavior.clone())
        .collect()
}

pub(super) fn note_behaviors_from_instruments(
    instruments: &[NativeInstrumentSlot],
) -> Vec<NoteBehavior> {
    let mut note_behaviors = vec![NoteBehavior::Oneshot; 16];
    for (index, instrument) in instruments.iter().enumerate().take(note_behaviors.len()) {
        note_behaviors[index] = if instrument.note_behavior == "hold" {
            NoteBehavior::Hold
        } else {
            NoteBehavior::Oneshot
        };
    }
    note_behaviors
}

pub(super) fn instrument_types(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.kind.clone())
        .collect()
}

pub(super) fn instrument_routes(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.route.clone())
        .collect()
}

pub(super) fn instrument_volumes(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.volume)
        .collect()
}

pub(super) fn instrument_pan_positions(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.pan_pos)
        .collect()
}

pub(super) fn normalize_route(route: &str) -> String {
    route
        .strip_prefix("bus_")
        .map(|suffix| format!("fx_bus_{suffix}"))
        .unwrap_or_else(|| route.to_string())
}

pub(super) fn trigger_target(
    slot: usize,
    action: &str,
    velocity: u8,
    duration_ms: u32,
) -> TriggerTarget {
    let action = if slot >= INSTRUMENT_COUNT {
        TriggerAction::None
    } else {
        match action {
            "note_off" => TriggerAction::NoteOff,
            "none" => TriggerAction::None,
            _ => TriggerAction::NoteOn,
        }
    };
    TriggerTarget {
        action,
        channel: slot.min(15) as u8,
        velocity,
        duration_ms,
    }
}

pub(super) fn slot_payload(slot: usize) -> Value {
    if slot >= INSTRUMENT_COUNT {
        Value::String("none".into())
    } else {
        Value::String(slot.to_string())
    }
}

pub(super) fn instrument_sample_slots(instruments: &[NativeInstrumentSlot]) -> Vec<usize> {
    instruments
        .iter()
        .map(|instrument| instrument.selected_sample_slot)
        .collect()
}

pub(super) fn instrument_synth_configs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.synth_config.clone())
        .collect()
}

pub(super) fn instrument_synth_osc1_waveforms(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| synth_string_at(instrument, &["osc1", "waveform"], "saw"))
        .collect()
}

pub(super) fn instrument_synth_osc2_waveforms(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| synth_string_at(instrument, &["osc2", "waveform"], "square"))
        .collect()
}

pub(super) fn instrument_synth_filter_types(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| synth_string_at(instrument, &["filter", "type"], "lowpass"))
        .collect()
}

pub(super) fn instrument_synth_filter_cutoffs(instruments: &[NativeInstrumentSlot]) -> Vec<u16> {
    instruments.iter().map(synth_filter_cutoff).collect()
}

pub(super) fn instrument_synth_gain_pct(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.synth_gain_pct)
        .collect()
}

pub(super) fn instrument_synth_filter_resonance(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments.iter().map(synth_filter_resonance).collect()
}

pub(super) fn synth_filter_resonance(instrument: &NativeInstrumentSlot) -> u8 {
    instrument
        .synth_config
        .get("filter")
        .and_then(|filter| filter.get("resonance"))
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .min(255) as u8
}

pub(super) fn synth_filter_cutoff(instrument: &NativeInstrumentSlot) -> u16 {
    instrument
        .synth_config
        .get("filter")
        .and_then(|filter| filter.get("cutoffHz"))
        .and_then(Value::as_u64)
        .unwrap_or(8000)
        .clamp(20, 20000) as u16
}

pub(super) fn synth_i32_at(instrument: &NativeInstrumentSlot, path: &[&str], fallback: i32) -> i32 {
    value_i32_at(&instrument.synth_config, path, fallback)
}

pub(super) fn synth_string_at(
    instrument: &NativeInstrumentSlot,
    path: &[&str],
    fallback: &str,
) -> String {
    value_string_at(&instrument.synth_config, path, fallback)
}

pub(super) fn cutoff_display_to_hz(display: i32) -> i32 {
    let t = f64::from(display.clamp(0, 255)) / 255.0;
    (80.0 * (16_000.0_f64 / 80.0).ln().mul_add(t, 0.0).exp()).round() as i32
}

pub(super) fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(80, 16_000) as f64;
    ((h / 80.0).ln() / (16_000.0_f64 / 80.0).ln() * 255.0).round() as i32
}

pub(super) fn value_i32_at(value: &Value, path: &[&str], fallback: i32) -> i32 {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

pub(super) fn value_string_at(value: &Value, path: &[&str], fallback: &str) -> String {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}

pub(super) fn set_json_path_string(value: &mut Value, path: &[&str], text: &str) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(object) = current.as_object_mut() else {
            return;
        };
        let Some(next) = object.get_mut(*key) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.insert((*last).to_string(), json!(text));
    }
}

pub(super) fn set_json_path_number(value: &mut Value, path: &[&str], number: f64) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(object) = current.as_object_mut() else {
            return;
        };
        let Some(next) = object.get_mut(*key) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.insert((*last).to_string(), json!(number.round() as i64));
    }
}

pub(super) fn synth_preset_config(id: &str) -> Value {
    match id {
        "soft_pad" => synth_config(
            "triangle", 78, 0, -3, 50, "pulse", 64, 0, 3, 42, 72, 85, 240, 360, 78, 460, "lowpass",
            3800, 18, 28, 20, 190, 420, 72, 500,
        ),
        "bright_pluck" => synth_config(
            "saw", 86, 0, 0, 50, "pulse", 52, 1, 6, 30, 84, 100, 3, 120, 18, 70, "lowpass", 7200,
            34, 54, 34, 2, 180, 16, 120,
        ),
        "bass_mono" => synth_config(
            "saw", 84, -1, 0, 50, "square", 68, -1, -4, 50, 88, 72, 5, 160, 56, 120, "lowpass",
            2100, 30, 22, 24, 7, 170, 44, 150,
        ),
        "hollow_pwm" => synth_config(
            "pulse", 74, 0, -6, 34, "pulse", 74, 0, 6, 66, 82, 96, 9, 260, 60, 180, "bandpass",
            2500, 48, 30, 28, 5, 220, 40, 180,
        ),
        "lead" => synth_config(
            "saw", 88, 0, 5, 50, "triangle", 64, 1, -2, 50, 85, 100, 2, 130, 26, 110, "highpass",
            650, 24, 46, 30, 3, 140, 24, 130,
        ),
        "bell" => synth_config(
            "sine", 76, 0, 0, 50, "triangle", 60, 1, 12, 50, 76, 100, 1, 540, 0, 360, "notch",
            3000, 52, 34, 12, 1, 380, 0, 280,
        ),
        "perc_hit" => synth_config(
            "square", 84, 0, 0, 50, "pulse", 48, 1, 0, 20, 88, 100, 0, 90, 0, 120, "lowpass", 4200,
            26, 72, 8, 0, 120, 0, 140,
        ),
        _ => synth_config(
            "saw", 80, 0, 0, 50, "square", 72, 0, 0, 50, 80, 100, 5, 120, 70, 180, "lowpass", 8000,
            20, 0, 0, 5, 120, 70, 180,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn synth_config(
    osc1_wave: &str,
    osc1_level: i32,
    osc1_octave: i32,
    osc1_detune: i32,
    osc1_pulse_width: i32,
    osc2_wave: &str,
    osc2_level: i32,
    osc2_octave: i32,
    osc2_detune: i32,
    osc2_pulse_width: i32,
    gain: i32,
    velocity_sensitivity: i32,
    amp_attack: i32,
    amp_decay: i32,
    amp_sustain: i32,
    amp_release: i32,
    filter_type: &str,
    cutoff: i32,
    resonance: i32,
    env_amount: i32,
    key_tracking: i32,
    filter_attack: i32,
    filter_decay: i32,
    filter_sustain: i32,
    filter_release: i32,
) -> Value {
    json!({
        "osc1": { "waveform": osc1_wave, "levelPct": osc1_level, "octave": osc1_octave, "detuneCents": osc1_detune, "pulseWidthPct": osc1_pulse_width },
        "osc2": { "waveform": osc2_wave, "levelPct": osc2_level, "octave": osc2_octave, "detuneCents": osc2_detune, "pulseWidthPct": osc2_pulse_width },
        "amp": { "gainPct": gain, "velocitySensitivityPct": velocity_sensitivity },
        "ampEnv": { "attackMs": amp_attack, "decayMs": amp_decay, "sustainPct": amp_sustain, "releaseMs": amp_release },
        "filter": { "type": filter_type, "cutoffHz": cutoff, "resonance": resonance, "envAmountPct": env_amount, "keyTrackingPct": key_tracking },
        "filterEnv": { "attackMs": filter_attack, "decayMs": filter_decay, "sustainPct": filter_sustain, "releaseMs": filter_release }
    })
}

pub(super) fn instrument_sample_tune_semis(instruments: &[NativeInstrumentSlot]) -> Vec<i8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_tune_semis)
        .collect()
}
pub(super) fn instrument_sample_gain_pct(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_gain_pct)
        .collect()
}
pub(super) fn instrument_sample_base_velocity(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_base_velocity)
        .collect()
}
pub(super) fn instrument_sample_amp_velocity_sensitivity_pct(
    instruments: &[NativeInstrumentSlot],
) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_amp_velocity_sensitivity_pct)
        .collect()
}
pub(super) fn instrument_sample_velocity_levels_enabled(
    instruments: &[NativeInstrumentSlot],
) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_levels_enabled)
        .collect()
}
pub(super) fn instrument_sample_velocity_high(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_high)
        .collect()
}
pub(super) fn instrument_sample_velocity_medium(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_medium)
        .collect()
}
pub(super) fn instrument_sample_velocity_low(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_low)
        .collect()
}
pub(super) fn instrument_sample_amp_envs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_amp_env.clone())
        .collect()
}
pub(super) fn instrument_sample_filters(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_filter.clone())
        .collect()
}
pub(super) fn instrument_sample_filter_envs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_filter_env.clone())
        .collect()
}
pub(super) fn instrument_midi_enabled(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_enabled)
        .collect()
}
pub(super) fn instrument_midi_channels(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_channel)
        .collect()
}
pub(super) fn instrument_midi_velocity(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_velocity)
        .collect()
}
pub(super) fn instrument_midi_duration_ms(instruments: &[NativeInstrumentSlot]) -> Vec<u16> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_duration_ms)
        .collect()
}
