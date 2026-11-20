use super::*;

const SAVED_CONFIGS: [(&str, &str); 4] = [
    (
        "config/default.json",
        include_str!("../../../../../config/default.json"),
    ),
    (
        "config/generated/desktop/default.json",
        include_str!("../../../../../config/generated/desktop/default.json"),
    ),
    (
        "config/generated/pi/default.json",
        include_str!("../../../../../config/generated/pi/default.json"),
    ),
    (
        "desktop e2e scenario_patch.json",
        include_str!(
            "../../../../../apps/desktop/src-tauri/tests/e2e/fixtures/scenario_patch.json"
        ),
    ),
];

/// The buses with each note-mode delay's `timeMs` dropped, since the runner derives it from
/// the tempo.
fn buses_without_derived_delay_time(config: &Value) -> Value {
    let mut buses = config["runtimeConfig"]["mixer"]["buses"].clone();
    for bus in buses.as_array_mut().unwrap() {
        for slot in ["slot1", "slot2", "slot3"] {
            let slot = &mut bus[slot];
            if slot["params"]["timeMode"] == "note" {
                slot["params"].as_object_mut().unwrap().remove("timeMs");
            }
        }
    }
    buses
}

#[test]
fn fx_bus_slots_save_exactly_as_loaded() {
    for (name, text) in SAVED_CONFIGS {
        let payload: Value = serde_json::from_str(text).unwrap();
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.apply_config_payload(payload.clone()).unwrap();
        let saved = runner.config_payload();
        assert_eq!(
            buses_without_derived_delay_time(&saved),
            buses_without_derived_delay_time(&payload),
            "{name}"
        );

        let mut reloaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        reloaded.apply_config_payload(saved.clone()).unwrap();
        assert_eq!(reloaded.config_payload(), saved, "{name}");
    }
}
