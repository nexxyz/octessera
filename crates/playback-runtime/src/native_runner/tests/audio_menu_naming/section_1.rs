use super::*;

#[test]
pub(crate) fn layer_and_bus_names_round_trip_with_auto_name_flags() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_names[0] = "lead".into();
    runner.layer_auto_names[0] = false;
    runner.fx_buses[0].name = "space".into();
    runner.fx_buses[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    let payload = runner.config_payload();

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();

    assert_eq!(restored.layer_names[0], "lead");
    assert!(!restored.layer_auto_names[0]);
    assert_eq!(restored.fx_buses[0].name, "space");
    assert!(!restored.fx_buses[0].auto_name);

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{ "slot1": { "type": "delay" }, "slot2": { "type": "duck" }, "autoName": true }]
                }
            }
        }))
        .unwrap();
    assert_eq!(restored.fx_buses[0].name, "Delay+Duck");

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "instruments": [{ "type": "sampler", "name": "sampler", "autoName": true }],
                "mixer": { "buses": [{ "slot1": { "type": "duck" }, "slot2": { "type": "none" }, "name": "duck", "autoName": true }] }
            }
        }))
        .unwrap();
    assert_eq!(restored.instruments[0].name, "Sampler");
    assert_eq!(restored.fx_buses[0].name, "Duck");

    restored
        .apply_config_payload(json!({
            "runtimeConfig": {
                "instruments": [{ "type": "sampler", "name": "manual lower", "autoName": false }],
                "mixer": { "buses": [{ "slot1": { "type": "duck" }, "slot2": { "type": "none" }, "name": "side duck", "autoName": false }] }
            }
        }))
        .unwrap();
    assert_eq!(restored.instruments[0].name, "manual lower");
    assert_eq!(restored.fx_buses[0].name, "side duck");
}

#[test]
pub(crate) fn native_text_row_edits_layer_name_and_clears_auto_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;

    runner.menu.press();
    runner.menu.turn(1);
    let snapshot = runner.menu.snapshot();
    runner.apply_menu_state().unwrap();

    assert!(snapshot.lines.iter().any(|line| line == "    * lifeA"));
    assert!(snapshot.lines.iter().all(|line| !line.contains('@')));
    assert_eq!(runner.layer_names[0], "lifeA");
    assert!(!runner.layer_auto_names[0]);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["name"],
        "lifeA"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["autoName"],
        false
    );
}

#[test]
pub(crate) fn lowercase_text_edit_and_manual_instrument_name_round_trip() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "instruments": [{ "type": "synth", "name": "lead bass", "autoName": false }]
            }
        }))
        .unwrap();
    assert_eq!(runner.instruments[0].name, "lead bass");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["name"],
        "lead bass"
    );

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 2;
    runner.menu.press();
    runner.menu.turn(27);
    let snapshot = runner.menu.snapshot();
    assert!(snapshot.lines.iter().any(|line| line == "    * lifea"));
}

#[test]
pub(crate) fn factory_payload_uses_display_style_auto_names() {
    let payload = super::super::native_factory_payload();
    let runtime = &payload["runtimeConfig"];
    assert_eq!(runtime["instruments"][0]["name"], "Synth");
    assert_eq!(runtime["instruments"][1]["name"], "drums");
    assert_eq!(runtime["instruments"][1]["autoName"], false);
    assert!(runtime["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .all(|instrument| {
            !instrument["autoName"].as_bool().unwrap_or(false)
                || instrument["name"] != "synth" && instrument["name"] != "sampler"
        }));
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.fx_buses[0].name, "Delay+Duck");
}

#[test]
pub(crate) fn auto_named_layer_renames_when_behavior_changes_to_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.layer_names[1] = "sequencer".into();
    runner.layer_auto_names[1] = false;
    runner.select_active_layer(1).unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner.menu.turn_key("layers.1.autoName", 1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.layer_names[1], "sequencer");
    assert!(runner.layer_auto_names[1]);

    select_behavior(&mut runner, "none");

    assert_eq!(runner.layer_behavior_ids[1], "none");
    assert_eq!(runner.layer_names[1], "none");
    assert!(runner.layer_auto_names[1]);
    assert_eq!(
        runner.menu.value_for_key("layers.1.name").as_deref(),
        Some("none")
    );
}

#[test]
pub(crate) fn auto_named_layer_renames_after_toggling_auto_name_off_and_on() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[3] = "sequencer".into();
    runner.layer_names[3] = "sequencer".into();
    runner.layer_auto_names[3] = true;
    runner.select_active_layer(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("layers.3.autoName"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("layers.3.autoName", -1);
    runner
        .apply_or_schedule_menu_key("layers.3.autoName")
        .unwrap();
    assert!(!runner.layer_auto_names[3]);

    runner.menu.turn_key("layers.3.autoName", 1);
    runner
        .apply_or_schedule_menu_key("layers.3.autoName")
        .unwrap();
    assert!(runner.layer_auto_names[3]);
    assert_eq!(runner.layer_names[3], "sequencer");

    select_behavior(&mut runner, "none");

    assert_eq!(runner.layer_behavior_ids[3], "none");
    assert!(runner.layer_auto_names[3]);
    assert_eq!(runner.layer_names[3], "none");
    assert_eq!(
        runner.menu.value_for_key("layers.3.name").as_deref(),
        Some("none")
    );
}
