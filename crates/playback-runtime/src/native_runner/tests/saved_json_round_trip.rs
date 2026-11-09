use super::*;

const DESKTOP_DEFAULT: &str = include_str!("../../../../../config/generated/desktop/default.json");
const PI_DEFAULT: &str = include_str!("../../../../../config/generated/pi/default.json");

const INSTRUMENT_TYPES: [&str; 8] = [
    "none",
    "synth",
    "fm",
    "pluck",
    "drum",
    "sampler",
    "midi",
    "midi_drum",
];
const ROUTES: [&str; 5] = ["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"];
const WAVEFORMS: [&str; 5] = ["sine", "triangle", "saw", "square", "pulse"];
const FILTER_TYPES: [&str; 4] = ["lowpass", "highpass", "bandpass", "notch"];
const VOICE_STEALING_MODES: [&str; 6] = [
    "none",
    "fixed12",
    "fixed16",
    "auto-soft",
    "auto-balanced",
    "auto-hard",
];
const PLAY_MODES: [&str; 11] = [
    "mix",
    "pan",
    "route",
    "tempo",
    "lfo",
    "faders",
    "fx",
    "xy",
    "transpose",
    "rate",
    "trigger-gate",
];

const SCAN_MODES: [&str; 2] = ["none", "scanning"];
const LINK_ACTIONS: [&str; 3] = ["none", "note_on", "note_off"];
const LINK_MAPPINGS: [&str; 5] = [
    "scanned",
    "scanned_empty",
    "activate",
    "stable",
    "deactivate",
];
const TRIGGER_PROBABILITY_MODES: [&str; 3] = ["zero", "custom", "full"];
const TILE_PROBABILITIES: [&str; 4] = ["zero", "low", "high", "full"];
const SCAN_AXES: [&str; 2] = ["rows", "columns"];
const SCAN_DIRECTIONS: [&str; 2] = ["forward", "reverse"];
const OUT_OF_RANGE: [&str; 2] = ["clamp", "wrap"];
const ARP_MODES: [&str; 11] = [
    "none",
    "direct",
    "up",
    "down",
    "bounce",
    "outside_in",
    "rotating",
    "random",
    "octave_spread",
    "chord_strike",
    "strum",
];
const ARP_SOURCES: [&str; 2] = ["simultaneous", "held"];
const LANE_CURVES: [&str; 2] = ["linear", "curve"];
const VALUE_LANES: [(&str, &str); 6] = [
    ("x", "velocity"),
    ("x", "filterCutoff"),
    ("x", "filterResonance"),
    ("y", "velocity"),
    ("y", "filterCutoff"),
    ("y", "filterResonance"),
];

/// The value a layer's Link field takes in one round; `field` shifts sets of equal length apart.
fn link_value(set: &[&'static str], round: usize, layer: usize, field: usize) -> &'static str {
    set[(round + layer + field) % set.len()]
}

/// Every saved Link value of each layer for one round, as (JSON pointer, value).
fn link_values(round: usize, layer: usize) -> Vec<(String, &'static str)> {
    let mut values = vec![
        ("/scanMode".into(), link_value(&SCAN_MODES, round, layer, 0)),
        ("/scanAxis".into(), link_value(&SCAN_AXES, round, layer, 1)),
        (
            "/scanDirection".into(),
            link_value(&SCAN_DIRECTIONS, round, layer / 2, 0),
        ),
        (
            "/triggerProbabilityMode".into(),
            link_value(&TRIGGER_PROBABILITY_MODES, round, layer, 0),
        ),
        (
            "/pitch/outOfRange".into(),
            link_value(&OUT_OF_RANGE, round, layer / 4, 0),
        ),
        ("/arp/mode".into(), link_value(&ARP_MODES, round, layer, 0)),
        (
            "/arp/source".into(),
            link_value(&ARP_SOURCES, round, layer / 2, 1),
        ),
    ];
    for (mapping, key) in LINK_MAPPINGS.iter().enumerate() {
        values.push((
            format!("/mapping/{key}/action"),
            link_value(&LINK_ACTIONS, round, layer, mapping),
        ));
    }
    for (lane, (axis, key)) in VALUE_LANES.iter().enumerate() {
        values.push((
            format!("/{axis}/{key}/curve"),
            link_value(&LANE_CURVES, round, layer, lane),
        ));
    }
    for tile in 0..64 {
        values.push((
            format!("/triggerProbabilityMap/{tile}"),
            link_value(&TILE_PROBABILITIES, round, layer, tile),
        ));
    }
    values
}

fn note_behavior(index: usize, kind: &str) -> &'static str {
    if index % 2 == 1 && !matches!(kind, "drum" | "midi_drum") {
        "hold"
    } else {
        "oneshot"
    }
}

fn saved_bytes(runner: &NativeRunner) -> (String, String) {
    let documents = split_local_system_patch_documents(&runner.config_payload()).unwrap();
    (
        serde_json::to_string_pretty(&documents.system).unwrap(),
        serde_json::to_string_pretty(&documents.patch).unwrap(),
    )
}

fn loaded(full: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(full).unwrap();
    runner
}

fn assert_load_save_is_byte_identical(full: Value) -> (String, String) {
    let saved = saved_bytes(&loaded(full));
    let system: Value = serde_json::from_str(&saved.0).unwrap();
    let patch: Value = serde_json::from_str(&saved.1).unwrap();
    let composed = compose_local_system_patch_documents(&system, &patch).unwrap();
    assert_eq!(saved_bytes(&loaded(composed)), saved);
    saved
}

fn every_enum_fixture(round: usize, voice_stealing: &str, play_mode: &str) -> Value {
    let mut full: Value = serde_json::from_str(DESKTOP_DEFAULT).unwrap();
    let runtime = &mut full["runtimeConfig"];
    for (index, kind) in INSTRUMENT_TYPES.iter().enumerate() {
        let instrument = &mut runtime["instruments"][index];
        instrument["type"] = json!(kind);
        instrument["noteBehavior"] = json!(note_behavior(index, kind));
        instrument["mixer"]["route"] = json!(ROUTES[(index + round) % ROUTES.len()]);
        instrument["synth"]["osc1"]["waveform"] = json!(WAVEFORMS[index % WAVEFORMS.len()]);
        instrument["synth"]["osc2"]["waveform"] = json!(WAVEFORMS[(index + 2) % WAVEFORMS.len()]);
        instrument["synth"]["filter"]["type"] = json!(FILTER_TYPES[index % FILTER_TYPES.len()]);
        instrument["sample"]["filter"]["type"] =
            json!(FILTER_TYPES[(index + 1) % FILTER_TYPES.len()]);
    }
    for (index, layer) in runtime["layers"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        for (pointer, value) in link_values(round, index) {
            *layer["link"].pointer_mut(&pointer).unwrap() = json!(value);
        }
    }
    runtime["sound"]["voiceStealingMode"] = json!(voice_stealing);
    runtime["playMode"] = json!(play_mode);
    full["system"]["playMode"] = json!(play_mode);
    full
}

#[test]
fn shipped_defaults_load_and_save_byte_identically() {
    for source in [DESKTOP_DEFAULT, PI_DEFAULT] {
        assert_load_save_is_byte_identical(serde_json::from_str(source).unwrap());
    }
}

#[test]
fn every_saved_enum_value_loads_and_saves_byte_identically() {
    let rounds = VOICE_STEALING_MODES.len().max(PLAY_MODES.len());
    for round in 0..rounds {
        let voice_stealing = VOICE_STEALING_MODES[round % VOICE_STEALING_MODES.len()];
        let play_mode = PLAY_MODES[round % PLAY_MODES.len()];
        let (system, patch) = assert_load_save_is_byte_identical(every_enum_fixture(
            round,
            voice_stealing,
            play_mode,
        ));
        let system: Value = serde_json::from_str(&system).unwrap();
        let patch: Value = serde_json::from_str(&patch).unwrap();
        let runtime = &patch["runtimeConfig"];
        assert_eq!(runtime["sound"]["voiceStealingMode"], voice_stealing);
        assert_eq!(runtime["playMode"], play_mode);
        assert!(system["runtimeConfig"].get("playMode").is_none());
        for (index, layer) in runtime["layers"].as_array().unwrap().iter().enumerate() {
            for (pointer, value) in link_values(round, index) {
                assert_eq!(layer["link"].pointer(&pointer).unwrap(), value, "{pointer}");
            }
        }
        for (index, kind) in INSTRUMENT_TYPES.iter().enumerate() {
            let instrument = &runtime["instruments"][index];
            assert_eq!(instrument["type"], *kind);
            assert_eq!(
                instrument["mixer"]["route"],
                ROUTES[(index + round) % ROUTES.len()]
            );
            assert_eq!(instrument["noteBehavior"], note_behavior(index, kind));
        }
    }
}
