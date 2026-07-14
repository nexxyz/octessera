use super::*;

#[test]
pub(crate) fn fx_bus_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot1.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["type"],
        "tremolo"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["rateHz"],
        4.0
    );
}

#[test]
pub(crate) fn global_fx_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.master.slots.0.type", 1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["type"],
        "vinyl"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]
            ["cracklePct"],
        8
    );
}

#[test]
pub(crate) fn fx_params_edit_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay", "params": { "timeMs": 250, "feedback": 0.35, "mixPct": 35 } } }],
                    "master": { "slots": [{ "type": "distortion", "params": { "drive": 2.5, "clip": 0.6, "mixPct": 100 } }] }
                }
            }
        }))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner
        .menu
        .turn_key("mixer.buses.0.slot1.params.feedback", 1);
    runner.menu.turn_key("mixer.master.slots.0.params.clip", 1);
    runner.apply_menu_state().unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["feedback"],
        0.36
    );
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.65
    );
}

#[test]
pub(crate) fn invalid_bus_and_global_fx_types_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "pitch_shift", "params": {} });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "delay", "params": {} });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.fx_buses[0].slot1_type, "none");
    assert_eq!(runner.global_fx_slots[0], "none");
}
