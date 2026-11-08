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

fn link_action(layer: usize, mapping: usize) -> &'static str {
    LINK_ACTIONS[(layer + mapping) % LINK_ACTIONS.len()]
}

fn tile_probability(layer: usize, tile: usize) -> &'static str {
    TILE_PROBABILITIES[(layer + tile) % TILE_PROBABILITIES.len()]
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

fn every_enum_fixture(source: &str, voice_stealing: &str, play_mode: &str) -> Value {
    let mut full: Value = serde_json::from_str(source).unwrap();
    let runtime = &mut full["runtimeConfig"];
    for (index, kind) in INSTRUMENT_TYPES.iter().enumerate() {
        let instrument = &mut runtime["instruments"][index];
        instrument["type"] = json!(kind);
        instrument["noteBehavior"] = json!(note_behavior(index, kind));
        instrument["mixer"]["route"] = json!(ROUTES[index % ROUTES.len()]);
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
        let link = &mut layer["link"];
        link["scanMode"] = json!(SCAN_MODES[index % SCAN_MODES.len()]);
        link["triggerProbabilityMode"] =
            json!(TRIGGER_PROBABILITY_MODES[index % TRIGGER_PROBABILITY_MODES.len()]);
        link["triggerProbabilityMap"] = json!((0..64)
            .map(|tile| tile_probability(index, tile))
            .collect::<Vec<_>>());
        for (mapping, key) in LINK_MAPPINGS.iter().enumerate() {
            link["mapping"][key]["action"] = json!(link_action(index, mapping));
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
    let modes = VOICE_STEALING_MODES.len().max(PLAY_MODES.len());
    for index in 0..modes {
        let voice_stealing = VOICE_STEALING_MODES[index % VOICE_STEALING_MODES.len()];
        let play_mode = PLAY_MODES[index % PLAY_MODES.len()];
        let (system, patch) = assert_load_save_is_byte_identical(every_enum_fixture(
            DESKTOP_DEFAULT,
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
            let link = &layer["link"];
            assert_eq!(link["scanMode"], SCAN_MODES[index % SCAN_MODES.len()]);
            assert_eq!(
                link["triggerProbabilityMode"],
                TRIGGER_PROBABILITY_MODES[index % TRIGGER_PROBABILITY_MODES.len()]
            );
            for tile in 0..64 {
                assert_eq!(
                    link["triggerProbabilityMap"][tile],
                    tile_probability(index, tile)
                );
            }
            for (mapping, key) in LINK_MAPPINGS.iter().enumerate() {
                assert_eq!(link["mapping"][key]["action"], link_action(index, mapping));
            }
        }
        for (index, kind) in INSTRUMENT_TYPES.iter().enumerate() {
            let instrument = &runtime["instruments"][index];
            assert_eq!(instrument["type"], *kind);
            assert_eq!(instrument["mixer"]["route"], ROUTES[index % ROUTES.len()]);
            assert_eq!(instrument["noteBehavior"], note_behavior(index, kind));
        }
    }
}
