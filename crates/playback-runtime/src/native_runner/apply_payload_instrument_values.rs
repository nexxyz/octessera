use super::apply_payload_mixer_values::nested_u64;
use super::sample_assignment_payload::tile_offset;
use super::{
    sample_assignment_from_payload, sanitize_pan_position_payload, NativeInstrumentSlot,
    NativeSamplePlayback, NativeTileStamp, Value, SAMPLE_SLOT_COUNT,
};
use crate::native_menu::{SAMPLE_LOOP_MODES, SAMPLE_START_MAX_PCT};

pub(super) fn apply_instrument_identity_payload(
    slot: &Value,
    instrument: &mut NativeInstrumentSlot,
) {
    if let Some(kind) = slot.get("type").and_then(Value::as_str) {
        if matches!(
            kind,
            "none" | "synth" | "fm" | "pluck" | "drum" | "sampler" | "midi" | "midi_drum"
        ) {
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
    }
}

pub(super) fn apply_instrument_tile_stamp_payload(
    slot: &Value,
    instrument: &mut NativeInstrumentSlot,
) {
    let Some(stamp) = slot.get("tileStamp") else {
        return;
    };
    instrument.tile_stamp = NativeTileStamp {
        tune_semis: tile_offset(stamp, "tuneSemis", 24),
        tune_cents: tile_offset(stamp, "tuneCents", 100),
        level: stamp
            .get("level")
            .and_then(Value::as_str)
            .filter(|level| matches!(*level, "high" | "medium" | "low"))
            .map(Into::into),
    };
}

pub(super) fn apply_instrument_mixer_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    let Some(mixer) = slot.get("mixer") else {
        return;
    };
    if let Some(volume) = mixer.get("volume").and_then(Value::as_u64) {
        if let Ok(volume) = u8::try_from(volume) {
            instrument.volume = volume.min(100);
        }
    }
    if let Some(pan_pos) = mixer.get("panPos").and_then(Value::as_u64) {
        instrument.pan_pos = sanitize_pan_position_payload(pan_pos);
    }
    if let Some(route) = mixer.get("route").and_then(Value::as_str) {
        instrument.route = route.into();
    }
}

pub(super) fn apply_instrument_sample_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    let Some(sample) = slot.get("sample") else {
        return;
    };
    apply_sample_slot_selection(sample, instrument);
    apply_sample_assignments_payload(sample, instrument);
    apply_sample_amp_payload(sample, instrument);
    apply_sample_filter_payload(sample, instrument);
    apply_sample_velocity_payload(sample, instrument);
}

pub(super) fn apply_sample_slot_selection(sample: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(selected_slot) = sample.get("selectedSlot").and_then(Value::as_u64) {
        if let Ok(selected_slot) = usize::try_from(selected_slot) {
            instrument.selected_sample_slot = selected_slot.min(SAMPLE_SLOT_COUNT - 1);
        }
    }
    if let Some(base_velocity) = sample.get("baseVelocity").and_then(Value::as_u64) {
        if let Ok(base_velocity) = u8::try_from(base_velocity) {
            instrument.sample_base_velocity = base_velocity.clamp(1, 127);
        }
    }
    if let Some(slots) = sample.get("slots").and_then(Value::as_array) {
        for (sample_index, sample_slot) in slots.iter().take(SAMPLE_SLOT_COUNT).enumerate() {
            instrument.sample_paths[sample_index] = sample_slot
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string);
            instrument.sample_playback[sample_index] = sample_playback_from_payload(sample_slot);
        }
    }
}

fn sample_playback_from_payload(slot: &Value) -> NativeSamplePlayback {
    let neutral = NativeSamplePlayback::default();
    let pct = |key: &str, fallback: u8| {
        slot.get(key)
            .and_then(Value::as_u64)
            .map_or(fallback, |value| value.min(100) as u8)
    };
    let loop_start_pct = pct("loopStartPct", neutral.loop_start_pct).min(SAMPLE_START_MAX_PCT);
    NativeSamplePlayback {
        loop_mode: slot
            .get("loop")
            .and_then(Value::as_str)
            .and_then(|mode| SAMPLE_LOOP_MODES.into_iter().find(|known| *known == mode))
            .unwrap_or(neutral.loop_mode),
        start_pct: pct("startPct", neutral.start_pct).min(SAMPLE_START_MAX_PCT),
        loop_start_pct,
        loop_end_pct: pct("loopEndPct", neutral.loop_end_pct).max(loop_start_pct + 1),
    }
}

pub(super) fn apply_sample_assignments_payload(
    sample: &Value,
    instrument: &mut NativeInstrumentSlot,
) {
    if let Some(assignments) = sample.get("assignments").and_then(Value::as_array) {
        instrument.sample_assignments = assignments
            .iter()
            .filter_map(sample_assignment_from_payload)
            .collect();
    }
    if let Some(tune) = sample.get("tuneSemis").and_then(Value::as_i64) {
        if let Ok(tune) = i8::try_from(tune) {
            instrument.sample_tune_semis = tune.clamp(-24, 24);
        }
    }
}

pub(super) fn apply_sample_amp_payload(sample: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(gain) = nested_u64(sample, &["amp", "gainPct"]) {
        if let Ok(gain) = u8::try_from(gain) {
            instrument.sample_gain_pct = gain.min(100);
        }
    }
    if let Some(velocity_sens) = nested_u64(sample, &["amp", "velocitySensitivityPct"]) {
        if let Ok(velocity_sens) = u8::try_from(velocity_sens) {
            instrument.sample_amp_velocity_sensitivity_pct = velocity_sens.min(100);
        }
    }
    if let Some(amp_env) = sample.get("ampEnv").filter(|value| value.is_object()) {
        instrument.sample_amp_env = amp_env.clone();
    }
}

pub(super) fn apply_sample_filter_payload(sample: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(filter) = sample.get("filter").filter(|value| value.is_object()) {
        instrument.sample_filter = filter.clone();
    }
    if let Some(filter_env) = sample.get("filterEnv").filter(|value| value.is_object()) {
        instrument.sample_filter_env = filter_env.clone();
    }
}

pub(super) fn apply_sample_velocity_payload(sample: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(enabled) = sample.get("velocityLevelsEnabled").and_then(Value::as_bool) {
        instrument.sample_velocity_levels_enabled = enabled;
    }
    if let Some(follow) = sample.get("followTranspose").and_then(Value::as_bool) {
        instrument.sample_follow_transpose = follow;
    }
    if let Some(follow) = sample.get("followPitch").and_then(Value::as_bool) {
        instrument.sample_follow_pitch = follow;
    }
    if let Some(levels) = sample.get("velocityLevels") {
        apply_sample_velocity_levels(levels, instrument);
    }
}

pub(super) fn apply_sample_velocity_levels(levels: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(high) = levels.get("high").and_then(Value::as_u64) {
        if let Ok(high) = u8::try_from(high) {
            instrument.sample_velocity_high = high.clamp(1, 127);
        }
    }
    if let Some(medium) = levels.get("medium").and_then(Value::as_u64) {
        if let Ok(medium) = u8::try_from(medium) {
            instrument.sample_velocity_medium = medium.clamp(1, 127);
        }
    }
    if let Some(low) = levels.get("low").and_then(Value::as_u64) {
        if let Ok(low) = u8::try_from(low) {
            instrument.sample_velocity_low = low.clamp(1, 127);
        }
    }
}

pub(super) fn apply_instrument_synth_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    let Some(synth) = slot.get("synth") else {
        return;
    };
    instrument.synth_config = synth.clone();
    if let Some(gain) = nested_u64(synth, &["amp", "gainPct"]) {
        if let Ok(gain) = u8::try_from(gain) {
            instrument.synth_gain_pct = gain.min(100);
        }
    }
}

pub(super) fn apply_instrument_fm_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(fm) = slot.get("fm") {
        instrument.fm_config = fm.clone();
    }
}

pub(super) fn apply_instrument_pluck_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(pluck) = slot.get("pluck") {
        instrument.pluck_config = pluck.clone();
    }
}

pub(super) fn apply_instrument_drum_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    if let Some(drum) = slot.get("drum") {
        instrument.drum_config = drum.clone();
    }
}

pub(super) fn apply_instrument_midi_drum_payload(
    slot: &Value,
    instrument: &mut NativeInstrumentSlot,
) {
    if let Some(midi_drum) = slot.get("midiDrum") {
        instrument.midi_drum.apply_payload(midi_drum);
    }
}

pub(super) fn apply_instrument_midi_payload(slot: &Value, instrument: &mut NativeInstrumentSlot) {
    let Some(value) = slot.get("midi") else {
        return;
    };
    if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
        instrument.midi_enabled = enabled;
    }
    if let Some(channel) = value.get("channel").and_then(Value::as_u64) {
        if let Ok(channel) = u8::try_from(channel) {
            instrument.midi_channel = channel.clamp(1, 16);
        }
    }
    if let Some(velocity) = value.get("velocity").and_then(Value::as_u64) {
        if let Ok(velocity) = u8::try_from(velocity) {
            instrument.midi_velocity = velocity.clamp(1, 127);
        }
    }
    if let Some(duration_ms) = value.get("durationMs").and_then(Value::as_u64) {
        if let Ok(duration_ms) = u16::try_from(duration_ms) {
            instrument.midi_duration_ms = duration_ms.clamp(10, 2000);
        }
    }
}
