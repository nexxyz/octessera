use super::canonical::{
    array_field, bool_field, enum_field, enum_value, object_field, object_value, signed_field,
    string_field, unsigned_field,
};
use super::Value;
use platform_core::{INSTRUMENT_COUNT, PAN_POSITION_COUNT, SAMPLE_SLOT_COUNT};
use serde_json::Map;

const VELOCITY_CURVES: &[&str] = &["linear", "soft", "hard"];
const VOICE_MODES: &[&str] = &[
    "none",
    "fixed12",
    "fixed16",
    "auto-soft",
    "auto-balanced",
    "auto-hard",
];

pub(super) fn validate_sound(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(sound) = object_field(runtime, "sound", "runtimeConfig")? else {
        return Ok(());
    };
    unsigned_field(sound, "noteLengthMs", "runtimeConfig.sound", 30, 2000)?;
    enum_field(
        sound,
        "velocityCurve",
        "runtimeConfig.sound",
        VELOCITY_CURVES,
    )?;
    enum_field(
        sound,
        "voiceStealingMode",
        "runtimeConfig.sound",
        VOICE_MODES,
    )?;
    unsigned_field(
        sound,
        "audioOutputBufferFrames",
        "runtimeConfig.sound",
        64,
        2048,
    )?;
    if let Some(value) = sound.get("audioOutputBufferFrames") {
        if !matches!(value.as_u64(), Some(64 | 128 | 256 | 512 | 1024 | 2048)) {
            return Err("runtimeConfig.sound.audioOutputBufferFrames is unsupported".into());
        }
    }
    Ok(())
}

pub(super) fn validate_instruments(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(instruments) = array_field(runtime, "instruments", "runtimeConfig", INSTRUMENT_COUNT)?
    else {
        return Ok(());
    };
    for (index, value) in instruments.iter().enumerate() {
        let path = format!("runtimeConfig.instruments[{index}]");
        let instrument = object_value(value, &path)?;
        enum_field(
            instrument,
            "type",
            &path,
            &["none", "synth", "sampler", "midi"],
        )?;
        enum_field(instrument, "noteBehavior", &path, &["oneshot", "hold"])?;
        string_field(instrument, "name", &path)?;
        bool_field(instrument, "autoName", &path)?;
        if let Some(mixer) = object_field(instrument, "mixer", &path)? {
            let mixer_path = format!("{path}.mixer");
            unsigned_field(mixer, "volume", &mixer_path, 0, 127)?;
            unsigned_field(
                mixer,
                "panPos",
                &mixer_path,
                0,
                (PAN_POSITION_COUNT - 1) as u64,
            )?;
            validate_route(mixer, &mixer_path)?;
        }
        validate_sample(instrument, &path)?;
        validate_synth(instrument, &path)?;
        for key in ["midi", "midiEngine"] {
            if let Some(midi) = object_field(instrument, key, &path)? {
                let midi_path = format!("{path}.{key}");
                bool_field(midi, "enabled", &midi_path)?;
                unsigned_field(midi, "channel", &midi_path, 1, 16)?;
                unsigned_field(midi, "velocity", &midi_path, 1, 127)?;
                unsigned_field(midi, "durationMs", &midi_path, 10, 5000)?;
            }
        }
    }
    Ok(())
}

fn validate_sample(instrument: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(sample) = object_field(instrument, "sample", path)? else {
        return Ok(());
    };
    let path = format!("{path}.sample");
    unsigned_field(
        sample,
        "selectedSlot",
        &path,
        0,
        (SAMPLE_SLOT_COUNT - 1) as u64,
    )?;
    unsigned_field(sample, "baseVelocity", &path, 1, 127)?;
    signed_field(sample, "tuneSemis", &path, -24, 24)?;
    if let Some(slots) = array_field(sample, "slots", &path, SAMPLE_SLOT_COUNT)? {
        for (index, value) in slots.iter().enumerate() {
            let slot = object_value(value, &format!("{path}.slots[{index}]"))?;
            if let Some(value) = slot.get("path") {
                if !value.is_null() && !value.is_string() {
                    return Err(format!(
                        "{path}.slots[{index}].path must be a string or null"
                    ));
                }
            }
        }
    }
    if let Some(assignments) = array_field(sample, "assignments", &path, usize::MAX)? {
        for (index, value) in assignments.iter().enumerate() {
            let assignment = object_value(value, &format!("{path}.assignments[{index}]"))?;
            let assignment_path = format!("{path}.assignments[{index}]");
            unsigned_field(assignment, "x", &assignment_path, 0, 7)?;
            unsigned_field(assignment, "y", &assignment_path, 0, 7)?;
            unsigned_field(
                assignment,
                "sampleSlot",
                &assignment_path,
                0,
                (SAMPLE_SLOT_COUNT - 1) as u64,
            )?;
            if let Some(level) = assignment.get("level") {
                if !level.is_null() {
                    enum_value(
                        level,
                        &format!("{assignment_path}.level"),
                        &["high", "medium", "low"],
                    )?;
                }
            }
        }
    }
    if let Some(amp) = object_field(sample, "amp", &path)? {
        unsigned_field(amp, "gainPct", &format!("{path}.amp"), 0, 100)?;
        unsigned_field(
            amp,
            "velocitySensitivityPct",
            &format!("{path}.amp"),
            0,
            100,
        )?;
    }
    validate_env(sample, "ampEnv", &path)?;
    validate_filter(sample, "filter", &path)?;
    validate_env(sample, "filterEnv", &path)?;
    if let Some(levels) = object_field(sample, "velocityLevels", &path)? {
        for key in ["high", "medium", "low"] {
            unsigned_field(levels, key, &format!("{path}.velocityLevels"), 1, 127)?;
        }
    }
    Ok(())
}

fn validate_synth(instrument: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(synth) = object_field(instrument, "synth", path)? else {
        return Ok(());
    };
    let path = format!("{path}.synth");
    for osc in ["osc1", "osc2"] {
        if let Some(value) = object_field(synth, osc, &path)? {
            let osc_path = format!("{path}.{osc}");
            enum_field(
                value,
                "waveform",
                &osc_path,
                &["sine", "triangle", "saw", "square", "pulse"],
            )?;
            signed_field(value, "octave", &osc_path, -2, 2)?;
            signed_field(value, "levelPct", &osc_path, 0, 100)?;
            signed_field(value, "detuneCents", &osc_path, -50, 50)?;
            signed_field(value, "pulseWidthPct", &osc_path, 5, 95)?;
        }
    }
    if let Some(amp) = object_field(synth, "amp", &path)? {
        signed_field(amp, "gainPct", &format!("{path}.amp"), 0, 100)?;
        signed_field(
            amp,
            "velocitySensitivityPct",
            &format!("{path}.amp"),
            0,
            100,
        )?;
    }
    validate_env(synth, "ampEnv", &path)?;
    validate_filter(synth, "filter", &path)?;
    validate_env(synth, "filterEnv", &path)
}

fn validate_env(parent: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    let Some(env) = object_field(parent, key, path)? else {
        return Ok(());
    };
    let path = format!("{path}.{key}");
    unsigned_field(env, "attackMs", &path, 0, 5000)?;
    unsigned_field(env, "decayMs", &path, 0, 5000)?;
    unsigned_field(env, "sustainPct", &path, 0, 100)?;
    unsigned_field(env, "releaseMs", &path, 0, 10000)
}

fn validate_filter(parent: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    let Some(filter) = object_field(parent, key, path)? else {
        return Ok(());
    };
    let path = format!("{path}.{key}");
    enum_field(
        filter,
        "type",
        &path,
        &["lowpass", "highpass", "bandpass", "notch"],
    )?;
    unsigned_field(filter, "cutoffHz", &path, 20, 20000)?;
    unsigned_field(filter, "resonance", &path, 0, 255)?;
    signed_field(filter, "envAmountPct", &path, -100, 100)?;
    unsigned_field(filter, "keyTrackingPct", &path, 0, 100)
}

fn validate_route(object: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(value) = object.get("route") else {
        return Ok(());
    };
    let route = value
        .as_str()
        .ok_or_else(|| format!("{path}.route must be a string"))?;
    let valid = route == "direct"
        || route
            .strip_prefix("fx_bus_")
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|index| index > 0 && index <= platform_core::BUS_COUNT);
    if !valid {
        return Err(format!("{path}.route has unknown route `{route}`"));
    }
    Ok(())
}
