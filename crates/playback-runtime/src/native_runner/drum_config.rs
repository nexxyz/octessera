use super::{json, synth_preset_config, NativeInstrumentSlot, Value};

pub(super) const DRUM_SOUNDS: [&str; 8] = [
    "kick",
    "snare",
    "closed_hat",
    "open_hat",
    "low_tom",
    "high_tom",
    "clap",
    "rim",
];
const DRUM_DECAY_MS: [u16; 8] = [420, 220, 85, 650, 500, 320, 240, 95];
const DRUM_TONE_PCT: [u8; 8] = [35, 70, 90, 85, 50, 60, 80, 80];
const DRUM_SWEEP_SEMIS: [u8; 8] = [24, 3, 0, 0, 8, 6, 0, 0];
const DRUM_SWEEP_MS: [u8; 8] = [55, 25, 0, 0, 70, 50, 0, 0];
const DRUM_NOISE_MIX_PCT: [u8; 8] = [12, 80, 95, 95, 15, 15, 95, 35];
const DRUM_VELOCITY_LEVELS: [(&str, u8); 3] = [("high", 120), ("medium", 85), ("low", 45)];

pub(super) fn drum_voice_default(sound: &str) -> Option<Value> {
    let index = DRUM_SOUNDS.iter().position(|id| *id == sound)?;
    Some(json!({
        "sound": sound,
        "tuneSemis": 0,
        "decayMs": DRUM_DECAY_MS[index],
        "tonePct": DRUM_TONE_PCT[index],
        "attackMs": 0,
        "sweepSemis": DRUM_SWEEP_SEMIS[index],
        "sweepMs": DRUM_SWEEP_MS[index],
        "noiseMixPct": DRUM_NOISE_MIX_PCT[index],
        "levelPct": 100,
    }))
}

pub(super) fn normalize_complete_voice_bank(payload: &mut Value) {
    let Some(instruments) = payload
        .get_mut("runtimeConfig")
        .and_then(|runtime| runtime.get_mut("instruments"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for instrument in instruments {
        if let Some(drum) = instrument.get_mut("drum").and_then(Value::as_object_mut) {
            drum.entry("velocityLevels")
                .or_insert_with(drum_velocity_levels_default);
            drum.entry("followTranspose").or_insert(Value::Bool(false));
            drum.entry("followPitch").or_insert(Value::Bool(false));
        }
        let Some(voices) = instrument
            .get_mut("drum")
            .and_then(|drum| drum.get_mut("voices"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        if voices.len() != DRUM_SOUNDS.len()
            || !voices.iter().all(|voice| {
                ["sound", "tuneSemis", "decayMs", "tonePct", "attackMs"]
                    .iter()
                    .all(|field| voice.get(field).is_some())
                    && voice
                        .get("sound")
                        .and_then(Value::as_str)
                        .is_some_and(|sound| DRUM_SOUNDS.contains(&sound))
            })
        {
            continue;
        }
        for voice in voices {
            let sound = voice["sound"].as_str().expect("validated drum sound");
            let defaults = drum_voice_default(sound).expect("validated drum sound");
            let object = voice.as_object_mut().expect("validated drum voice object");
            for field in ["sweepSemis", "sweepMs", "noiseMixPct", "levelPct"] {
                if !object.contains_key(field) {
                    object.insert(field.into(), defaults[field].clone());
                }
            }
        }
    }
}

pub(super) fn drum_default_config() -> Value {
    let synth = synth_preset_config("init");
    json!({
        "voices": DRUM_SOUNDS.iter().map(|sound| drum_voice_default(sound).expect("fixed drum sound")).collect::<Vec<_>>(),
        "assignments": [],
        "velocityLevels": drum_velocity_levels_default(),
        "followTranspose": false,
        "followPitch": false,
        "amp": { "gainPct": 80, "velocitySensitivityPct": 100 },
        "ampEnv": { "attackMs": 0, "decayMs": 0, "sustainPct": 100, "releaseMs": 30 },
        "filter": synth["filter"],
        "filterEnv": synth["filterEnv"],
    })
}

fn drum_velocity_levels_default() -> Value {
    Value::Object(
        DRUM_VELOCITY_LEVELS
            .iter()
            .map(|(level, velocity)| ((*level).into(), (*velocity).into()))
            .collect(),
    )
}

/// The velocity a stamped Drum cell level stands for; `None` for Base and unknown levels.
pub(super) fn drum_velocity_level(drum_config: &Value, level: &str) -> Option<u8> {
    let default = DRUM_VELOCITY_LEVELS
        .iter()
        .find(|(name, _)| *name == level)?
        .1;
    Some(
        drum_config["velocityLevels"][level]
            .as_u64()
            .map_or(default, |velocity| velocity.clamp(1, 127) as u8),
    )
}

pub(super) fn instrument_drum_configs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.drum_config.clone())
        .collect()
}

pub(super) fn voice_numeric_field(field: &str) -> Option<(usize, &str, i32, i32)> {
    let (voice, param) = field.strip_prefix("drum.voices.")?.split_once('.')?;
    let voice = voice.parse::<usize>().ok()?;
    if voice >= DRUM_SOUNDS.len() {
        return None;
    }
    let (min, max) = match param {
        "tuneSemis" => (-12, 12),
        "decayMs" => (20, 2000),
        "tonePct" => (0, 100),
        "attackMs" => (0, 50),
        "sweepSemis" => (0, 36),
        "sweepMs" => (0, 200),
        "noiseMixPct" | "levelPct" => (0, 100),
        _ => return None,
    };
    Some((voice, param, min, max))
}

pub(super) fn common_numeric_field(field: &str) -> Option<(&[&str], i32, i32)> {
    match field {
        "drum.amp.gainPct" => Some((&["amp", "gainPct"], 0, 100)),
        "drum.amp.velocitySensitivityPct" => Some((&["amp", "velocitySensitivityPct"], 0, 100)),
        "drum.filter.cutoffHz" => Some((&["filter", "cutoffHz"], 0, 255)),
        "drum.filter.resonance" => Some((&["filter", "resonance"], 0, 255)),
        "drum.filter.envAmountPct" => Some((&["filter", "envAmountPct"], -100, 100)),
        "drum.filter.keyTrackingPct" => Some((&["filter", "keyTrackingPct"], 0, 100)),
        _ => None,
    }
}
