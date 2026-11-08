use super::*;

pub(super) fn instrument_synth_configs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.synth_config.clone())
        .collect()
}

pub(super) fn instrument_fm_configs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.fm_config.clone())
        .collect()
}

pub(super) fn instrument_pluck_configs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.pluck_config.clone())
        .collect()
}

pub(super) fn pluck_default_config() -> Value {
    let synth = synth_default_config();
    json!({
        "decayMs": 1500,
        "brightnessPct": 65,
        "pickPositionPct": 25,
        "pickDepthPct": 65,
        "dispersionPct": 0,
        "bodyAmountPct": 0,
        "bodyFrequencyHz": 500,
        "amp": { "gainPct": 80, "velocitySensitivityPct": 100 },
        "ampEnv": { "attackMs": 0, "decayMs": 0, "sustainPct": 100, "releaseMs": 900 },
        "filter": synth["filter"],
        "filterEnv": synth["filterEnv"]
    })
}

pub(super) fn fm_default_config() -> Value {
    let synth = synth_default_config();
    json!({
        "ratio": "2",
        "ratioFineCents": 0,
        "index": 50,
        "velocityToIndexPct": 0,
        "modShapePct": 0,
        "modMixPct": 0,
        "indexEnv": { "attackMs": 0, "decayMs": 250, "sustainPct": 20, "releaseMs": 120 },
        "amp": { "gainPct": 80, "velocitySensitivityPct": 100 },
        "ampEnv": { "attackMs": 5, "decayMs": 300, "sustainPct": 70, "releaseMs": 350 },
        "filter": {
            "type": "lowpass",
            "cutoffHz": 20_000,
            "resonance": 0,
            "envAmountPct": 0,
            "keyTrackingPct": 0
        },
        "filterEnv": synth["filterEnv"]
    })
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

/// The plain numeric synth rows: their menu suffix, JSON path, and range.
pub(super) const SYNTH_NUMERIC_FIELDS: &[(&str, &[&str], i32, i32)] = &[
    ("osc1.octave", &["osc1", "octave"], -2, 2),
    ("osc1.levelPct", &["osc1", "levelPct"], 0, 100),
    ("osc1.detuneCents", &["osc1", "detuneCents"], -50, 50),
    ("osc1.pulseWidthPct", &["osc1", "pulseWidthPct"], 5, 95),
    ("osc2.octave", &["osc2", "octave"], -2, 2),
    ("osc2.levelPct", &["osc2", "levelPct"], 0, 100),
    ("osc2.detuneCents", &["osc2", "detuneCents"], -50, 50),
    ("osc2.pulseWidthPct", &["osc2", "pulseWidthPct"], 5, 95),
    (
        "filter.envAmountPct",
        &["filter", "envAmountPct"],
        -100,
        100,
    ),
    (
        "filter.keyTrackingPct",
        &["filter", "keyTrackingPct"],
        0,
        100,
    ),
    (
        "amp.velocitySensitivityPct",
        &["amp", "velocitySensitivityPct"],
        0,
        100,
    ),
    ("ampEnv.attackMs", &["ampEnv", "attackMs"], 0, 5000),
    ("ampEnv.decayMs", &["ampEnv", "decayMs"], 0, 5000),
    ("ampEnv.sustainPct", &["ampEnv", "sustainPct"], 0, 100),
    ("ampEnv.releaseMs", &["ampEnv", "releaseMs"], 0, 10000),
    ("filterEnv.attackMs", &["filterEnv", "attackMs"], 0, 5000),
    ("filterEnv.decayMs", &["filterEnv", "decayMs"], 0, 5000),
    ("filterEnv.sustainPct", &["filterEnv", "sustainPct"], 0, 100),
    ("filterEnv.releaseMs", &["filterEnv", "releaseMs"], 0, 10000),
];

/// The JSON path and range of a synth row the engine takes as a scalar param
/// (`synth.osc1.levelPct`); an octave change rebuilds the slot instead.
pub(super) fn synth_scalar_field(suffix: &str) -> Option<(&'static [&'static str], i32, i32)> {
    let field = suffix.strip_prefix("synth.")?;
    if field.ends_with(".octave") {
        return None;
    }
    SYNTH_NUMERIC_FIELDS
        .iter()
        .find(|(name, ..)| *name == field)
        .map(|&(_, path, min, max)| (path, min, max))
}

pub(super) fn normalize_voice_stealing_mode(value: &str) -> Option<&'static str> {
    match value {
        "none" => Some("none"),
        "fixed12" => Some("fixed12"),
        "fixed16" => Some("fixed16"),
        "auto-soft" => Some("auto-soft"),
        "auto-balanced" => Some("auto-balanced"),
        "auto-hard" => Some("auto-hard"),
        _ => None,
    }
}

pub(super) fn cutoff_display_to_hz(display: i32) -> i32 {
    let t = f64::from(display.clamp(0, 255)) / 255.0;
    (20.0 * (20_000.0_f64 / 20.0).ln().mul_add(t, 0.0).exp()).round() as i32
}

pub(super) fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(20, 20_000) as f64;
    ((h / 20.0).ln() / (20_000.0_f64 / 20.0).ln() * 255.0).round() as i32
}

pub(super) fn synth_default_config() -> Value {
    json!({
        "osc1": { "waveform": "saw", "levelPct": 80, "octave": 0, "detuneCents": 0, "pulseWidthPct": 50 },
        "osc2": { "waveform": "square", "levelPct": 72, "octave": 0, "detuneCents": 0, "pulseWidthPct": 50 },
        "amp": { "gainPct": 80, "velocitySensitivityPct": 100 },
        "ampEnv": { "attackMs": 5, "decayMs": 120, "sustainPct": 70, "releaseMs": 180 },
        "filter": { "type": "lowpass", "cutoffHz": 8000, "resonance": 0, "envAmountPct": 0, "keyTrackingPct": 0 },
        "filterEnv": { "attackMs": 5, "decayMs": 120, "sustainPct": 70, "releaseMs": 180 }
    })
}
