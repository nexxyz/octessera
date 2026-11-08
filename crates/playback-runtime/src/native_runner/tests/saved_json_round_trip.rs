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
        for (index, kind) in INSTRUMENT_TYPES.iter().enumerate() {
            let instrument = &runtime["instruments"][index];
            assert_eq!(instrument["type"], *kind);
            assert_eq!(instrument["mixer"]["route"], ROUTES[index % ROUTES.len()]);
            assert_eq!(instrument["noteBehavior"], note_behavior(index, kind));
        }
    }
}
