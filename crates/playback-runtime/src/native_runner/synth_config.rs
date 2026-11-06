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
