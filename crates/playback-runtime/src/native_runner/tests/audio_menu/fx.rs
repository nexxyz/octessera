use super::*;

#[test]
pub(crate) fn fx_bus_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.edit_menu_key("mixer.buses.0.slot3.type", 1).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot3"]["type"],
        "tremolo"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot3"]["params"]["rateHz"],
        4.0
    );
}

#[test]
pub(crate) fn global_fx_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .edit_menu_key("mixer.master.slots.0.type", 1)
        .unwrap();

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
        .apply_config_payload(config_with(&runner, json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay", "params": { "timeMs": 250, "feedback": 0.35, "mixPct": 35 } } }],
                    "master": { "slots": [{ "type": "distortion", "params": { "drive": 2.5, "clip": 0.6, "mixPct": 100 } }] }
                }
            }
        })))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner
        .edit_menu_key("mixer.buses.0.slot1.params.feedback", 1)
        .unwrap();
    runner
        .edit_menu_key("mixer.master.slots.0.params.clip", 1)
        .unwrap();

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
pub(crate) fn invalid_bus_and_global_fx_types_are_rejected_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "pitch_shift", "params": {} });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "delay", "params": {} });

    assert_rejected_without_byte_changes(&mut runner, payload);
}

fn bind_aux2_to_bus_delay_mix(runner: &mut NativeRunner) -> &'static str {
    let key = "mixer.buses.0.slot1.params.mixPct";
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params =
        json!({ "timeMode": "ms", "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    runner.menu.rebuild(runner.menu_config());
    runner.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some(key.into()),
        press_action: None,
    });
    key
}

fn turn_aux2(runner: &mut NativeRunner) {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "aux2" }),
            request_snapshot: None,
        })
        .unwrap();
}

#[test]
pub(crate) fn aux_sweep_on_a_bus_fx_param_searches_the_menu_tree_only_once() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let key = bind_aux2_to_bus_delay_mix(&mut runner);
    turn_aux2(&mut runner);
    let searches = runner.menu.key_paths.tree_searches();

    for _ in 0..10 {
        turn_aux2(&mut runner);
    }

    assert_eq!(runner.menu.key_paths.tree_searches(), searches);
    let mix = runner.menu.number_for_key(key).unwrap();
    assert!(mix < 35, "{mix}");
    assert_eq!(runner.fx_buses[0].slot1_params["mixPct"], mix);
}

#[test]
pub(crate) fn aux_sweep_follows_its_row_after_the_menu_tree_shifts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let key = bind_aux2_to_bus_delay_mix(&mut runner);
    turn_aux2(&mut runner);
    let first = runner.menu.root.children[0].clone();
    runner.menu.root.children.insert(0, first);

    turn_aux2(&mut runner);

    let mix = runner.menu.number_for_key(key).unwrap();
    assert!(mix < 34, "{mix}");
    assert_eq!(runner.fx_buses[0].slot1_params["mixPct"], mix);
}
