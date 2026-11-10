use super::{
    sanitize_pan_position_payload, InstrumentDto, InstrumentType, MixerRoute, NativeInstrumentSlot,
    NativeSampleAssignment, NativeSamplePlayback, NativeTileStamp, SampleDto, SampleSlotDto,
    TileStampDto, GRID_HEIGHT, GRID_WIDTH, SAMPLE_SLOT_COUNT,
};
use crate::native_menu::{SAMPLE_LOOP_MODES, SAMPLE_START_MAX_PCT};
use platform_core::NoteBehavior;
use serde_json::json;

pub(super) fn apply_instrument_payload(
    slot: &InstrumentDto,
    instrument: &mut NativeInstrumentSlot,
) {
    if let Some(kind) = slot
        .r#type
        .as_deref()
        .and_then(InstrumentType::from_wire_name)
    {
        instrument.kind = kind;
    }
    if let Some(note_behavior) = slot
        .note_behavior
        .as_deref()
        .and_then(NoteBehavior::from_wire_name)
    {
        instrument.note_behavior = note_behavior;
    }
    if let Some(auto_name) = slot.auto_name {
        instrument.auto_name = auto_name;
    }
    if let Some(name) = &slot.name {
        instrument.name.clone_from(name);
    }
    if let Some(stamp) = &slot.tile_stamp {
        instrument.tile_stamp = tile_stamp_from_payload(stamp);
    }
    if let Some(mixer) = &slot.mixer {
        if let Some(volume) = mixer.volume {
            instrument.volume = volume.min(100);
        }
        if let Some(pan_pos) = mixer.pan_pos {
            instrument.pan_pos = sanitize_pan_position_payload(u64::from(pan_pos));
        }
        if let Some(route) = mixer.route.as_deref().and_then(MixerRoute::from_wire_name) {
            instrument.route = route;
        }
    }
    if let Some(sample) = &slot.sample {
        apply_sample_payload(sample, instrument);
    }
    if let Some(synth) = &slot.synth {
        instrument.synth_config = json!(synth);
        if let Some(gain) = synth
            .amp
            .as_ref()
            .and_then(|amp| amp.gain_pct)
            .and_then(|gain| u8::try_from(gain).ok())
        {
            instrument.synth_gain_pct = gain.min(100);
        }
    }
    if let Some(fm) = &slot.fm {
        instrument.fm_config = json!(fm);
    }
    if let Some(pluck) = &slot.pluck {
        instrument.pluck_config = json!(pluck);
    }
    if let Some(drum) = &slot.drum {
        instrument.drum_config = json!(drum);
    }
    if let Some(midi) = &slot.midi {
        if let Some(enabled) = midi.enabled {
            instrument.midi_enabled = enabled;
        }
        if let Some(channel) = midi.channel {
            instrument.midi_channel = channel.clamp(1, 16);
        }
        if let Some(velocity) = midi.velocity {
            instrument.midi_velocity = velocity.clamp(1, 127);
        }
        if let Some(duration_ms) = midi.duration_ms {
            instrument.midi_duration_ms = duration_ms.clamp(10, 2000);
        }
    }
    if let Some(midi_drum) = &slot.midi_drum {
        instrument.midi_drum.apply(midi_drum);
    }
}

fn tile_offset(offset: Option<i8>, limit: i8) -> i8 {
    offset.map_or(0, |offset| offset.clamp(-limit, limit))
}

fn tile_level(level: &Option<String>) -> Option<String> {
    level
        .clone()
        .filter(|level| matches!(level.as_str(), "high" | "medium" | "low"))
}

fn tile_stamp_from_payload(stamp: &TileStampDto) -> NativeTileStamp {
    NativeTileStamp {
        tune_semis: tile_offset(stamp.tune_semis, 24),
        tune_cents: tile_offset(stamp.tune_cents, 100),
        level: tile_level(&stamp.level),
    }
}

fn apply_sample_payload(sample: &SampleDto, instrument: &mut NativeInstrumentSlot) {
    if let Some(selected_slot) = sample.selected_slot {
        instrument.selected_sample_slot = selected_slot.min(SAMPLE_SLOT_COUNT - 1);
    }
    if let Some(base_velocity) = sample.base_velocity {
        instrument.sample_base_velocity = base_velocity.clamp(1, 127);
    }
    if let Some(slots) = &sample.slots {
        for (sample_index, sample_slot) in slots.iter().take(SAMPLE_SLOT_COUNT).enumerate() {
            instrument.sample_paths[sample_index].clone_from(&sample_slot.path);
            instrument.sample_playback[sample_index] = sample_playback_from_payload(sample_slot);
        }
    }
    if let Some(assignments) = &sample.assignments {
        instrument.sample_assignments = assignments
            .iter()
            .filter_map(|assignment| {
                Some(NativeSampleAssignment {
                    x: assignment.x?.min(GRID_WIDTH - 1),
                    y: assignment.y?.min(GRID_HEIGHT - 1),
                    sample_slot: assignment.sample_slot?.min(7),
                    level: tile_level(&assignment.level),
                    tune_semis: tile_offset(assignment.tune_semis, 24),
                    tune_cents: tile_offset(assignment.tune_cents, 100),
                })
            })
            .collect();
    }
    if let Some(tune) = sample.tune_semis {
        instrument.sample_tune_semis = tune.clamp(-24, 24);
    }
    if let Some(amp) = &sample.amp {
        if let Some(gain) = amp.gain_pct {
            instrument.sample_gain_pct = gain.min(100);
        }
        if let Some(velocity_sens) = amp.velocity_sensitivity_pct {
            instrument.sample_amp_velocity_sensitivity_pct = velocity_sens.min(100);
        }
    }
    if let Some(amp_env) = &sample.amp_env {
        instrument.sample_amp_env = json!(amp_env);
    }
    if let Some(filter) = &sample.filter {
        instrument.sample_filter = json!(filter);
    }
    if let Some(filter_env) = &sample.filter_env {
        instrument.sample_filter_env = json!(filter_env);
    }
    if let Some(enabled) = sample.velocity_levels_enabled {
        instrument.sample_velocity_levels_enabled = enabled;
    }
    if let Some(follow) = sample.follow_transpose {
        instrument.sample_follow_transpose = follow;
    }
    if let Some(follow) = sample.follow_pitch {
        instrument.sample_follow_pitch = follow;
    }
    if let Some(levels) = &sample.velocity_levels {
        for (level, target) in [
            (levels.high, &mut instrument.sample_velocity_high),
            (levels.medium, &mut instrument.sample_velocity_medium),
            (levels.low, &mut instrument.sample_velocity_low),
        ] {
            if let Some(level) = level {
                *target = level.clamp(1, 127);
            }
        }
    }
}

fn sample_playback_from_payload(slot: &SampleSlotDto) -> NativeSamplePlayback {
    let neutral = NativeSamplePlayback::default();
    let pct = |value: Option<u8>, fallback: u8| value.map_or(fallback, |value| value.min(100));
    let loop_start_pct = pct(slot.loop_start_pct, neutral.loop_start_pct).min(SAMPLE_START_MAX_PCT);
    NativeSamplePlayback {
        loop_mode: slot
            .loop_mode
            .as_deref()
            .and_then(|mode| SAMPLE_LOOP_MODES.into_iter().find(|known| *known == mode))
            .unwrap_or(neutral.loop_mode),
        start_pct: pct(slot.start_pct, neutral.start_pct).min(SAMPLE_START_MAX_PCT),
        loop_start_pct,
        loop_end_pct: pct(slot.loop_end_pct, neutral.loop_end_pct).max(loop_start_pct + 1),
    }
}
