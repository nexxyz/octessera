use super::*;

#[test]
pub(crate) fn layer_and_bus_names_round_trip_with_auto_name_flags() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].name = "lead".into();
    runner.layers[0].auto_name = false;
    runner.fx_buses[0].name = "space".into();
    runner.fx_buses[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    let payload = runner.config_payload();

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();

    assert_eq!(restored.layers[0].name, "lead");
    assert!(!restored.layers[0].auto_name);
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
    runner.menu.open_labels(&["Build", "L1: life"]);
    runner.menu.select_label("Layer Label");

    runner.menu.press();
    runner.menu.turn(1);
    let snapshot = runner.menu.snapshot();
    runner.apply_current_menu_edit().unwrap();

    let row = snapshot.rows[snapshot.selected_row.unwrap()]
        .as_ref()
        .unwrap();
    assert_eq!(row.value, "lifeA");
    assert!(row.text_cursor.is_some());
    assert!(snapshot.lines.iter().all(|line| !line.contains('@')));
    assert_eq!(runner.layers[0].name, "lifeA");
    assert!(!runner.layers[0].auto_name);
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

    runner.menu.open_labels(&["Build", "L1: life"]);
    runner.menu.select_label("Layer Label");
    runner.menu.press();
    runner.menu.turn(27);
    let snapshot = runner.menu.snapshot();
    let row = snapshot.rows[snapshot.selected_row.unwrap()]
        .as_ref()
        .unwrap();
    assert_eq!(row.value, "lifea");
    assert!(row.text_cursor.is_some());
}

#[test]
pub(crate) fn factory_payload_uses_display_style_auto_names() {
    let payload = super::super::native_factory_payload();
    let runtime = &payload["runtimeConfig"];
    assert_eq!(runtime["instruments"][0]["name"], "Synth");
    assert_eq!(runtime["instruments"][1]["name"], "Sampler");
    assert_eq!(runtime["instruments"][5]["name"], "Drum");
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.fx_buses[0].name, "Delay+Reverb+Duck");
}

#[test]
pub(crate) fn auto_named_layer_renames_when_behavior_changes_to_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[1].behavior_id = "sequencer".into();
    runner.layers[1].name = "sequencer".into();
    runner.layers[1].auto_name = false;
    runner.select_active_layer(1).unwrap();
    runner.menu.rebuild(runner.menu_config());

    runner.edit_menu_key("layers.1.autoName", 1).unwrap();
    assert_eq!(runner.layers[1].name, "sequencer");
    assert!(runner.layers[1].auto_name);

    select_behavior(&mut runner, "none");

    assert_eq!(runner.layers[1].behavior_id, "none");
    assert_eq!(runner.layers[1].name, "none");
    assert!(runner.layers[1].auto_name);
    assert_eq!(
        runner.menu.value_for_key("layers.1.name").as_deref(),
        Some("none")
    );
}

#[test]
pub(crate) fn auto_named_layer_renames_after_toggling_auto_name_off_and_on() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[3].behavior_id = "sequencer".into();
    runner.layers[3].name = "sequencer".into();
    runner.layers[3].auto_name = true;
    runner.select_active_layer(3).unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("layers.3.autoName"));
    runner.menu.state.editing = true;
    runner.menu.turn_key("layers.3.autoName", -1);
    runner
        .apply_or_schedule_menu_key("layers.3.autoName")
        .unwrap();
    assert!(!runner.layers[3].auto_name);

    runner.menu.turn_key("layers.3.autoName", 1);
    runner
        .apply_or_schedule_menu_key("layers.3.autoName")
        .unwrap();
    assert!(runner.layers[3].auto_name);
    assert_eq!(runner.layers[3].name, "sequencer");

    select_behavior(&mut runner, "none");

    assert_eq!(runner.layers[3].behavior_id, "none");
    assert!(runner.layers[3].auto_name);
    assert_eq!(runner.layers[3].name, "none");
    assert_eq!(
        runner.menu.value_for_key("layers.3.name").as_deref(),
        Some("none")
    );
}
