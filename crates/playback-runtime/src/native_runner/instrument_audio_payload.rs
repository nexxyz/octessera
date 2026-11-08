use super::{sample_assignments_payload, NativeInstrumentSlot, NativeTileStamp, Value};
use crate::native_runner::InstrumentType;
use serde_json::json;

fn tile_stamp_payload(stamp: &NativeTileStamp) -> Value {
    let mut payload = json!({ "tuneSemis": stamp.tune_semis, "tuneCents": stamp.tune_cents });
    if let Some(level) = &stamp.level {
        payload["level"] = level.as_str().into();
    }
    payload
}

pub(super) fn instrument_audio_payload(instrument: &NativeInstrumentSlot) -> Value {
    let sample_slots = instrument
        .sample_paths
        .iter()
        .zip(&instrument.sample_playback)
        .map(|(path, playback)| {
            json!({
                "path": path,
                "loop": playback.loop_mode,
                "startPct": playback.start_pct,
                "loopStartPct": playback.loop_start_pct,
                "loopEndPct": playback.loop_end_pct
            })
        })
        .collect::<Vec<_>>();
    json!({
        "type": instrument.kind,
        "noteBehavior": if matches!(instrument.kind, InstrumentType::Drum | InstrumentType::MidiDrum) { "oneshot" } else { instrument.note_behavior.wire_name() },
        "autoName": instrument.auto_name,
        "name": instrument.name,
        "synth": instrument.synth_config,
        "fm": instrument.fm_config,
        "pluck": instrument.pluck_config,
        "drum": instrument.drum_config,
        "tileStamp": tile_stamp_payload(&instrument.tile_stamp),
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
            "followTranspose": instrument.sample_follow_transpose,
            "followPitch": instrument.sample_follow_pitch,
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
        "midiDrum": instrument.midi_drum.payload(),
        "mixer": {
            "volume": instrument.volume,
            "panPos": instrument.pan_pos,
            "route": instrument.route.clone()
        }
    })
}

pub(super) fn instrument_payload_owns_sample_bank(payload: &Value) -> bool {
    payload
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == InstrumentType::Sampler.wire_name())
        && payload.get("sample").is_some_and(Value::is_object)
}
