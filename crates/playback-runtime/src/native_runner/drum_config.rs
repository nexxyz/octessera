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

pub(super) fn drum_voice_default(sound: &str) -> Option<Value> {
    let index = DRUM_SOUNDS.iter().position(|id| *id == sound)?;
    Some(json!({
        "sound": sound,
        "tuneSemis": 0,
        "decayMs": DRUM_DECAY_MS[index],
        "tonePct": DRUM_TONE_PCT[index],
        "attackMs": 0,
    }))
}

pub(super) fn drum_default_config() -> Value {
    let synth = synth_preset_config("init");
    json!({
        "voices": DRUM_SOUNDS.iter().map(|sound| drum_voice_default(sound).expect("fixed drum sound")).collect::<Vec<_>>(),
        "assignments": [],
        "amp": { "gainPct": 80, "velocitySensitivityPct": 100 },
        "ampEnv": { "attackMs": 0, "decayMs": 0, "sustainPct": 100, "releaseMs": 30 },
        "filter": synth["filter"],
        "filterEnv": synth["filterEnv"],
    })
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
