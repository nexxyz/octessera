use super::*;

#[test]
pub(crate) fn synth_preset_load_changes_full_synth_payload_and_filter_resonance_is_editable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let initial = runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"].clone();

    runner.load_synth_preset(0, "bright_pluck");
    let changed = runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"].clone();

    assert_ne!(changed, initial);
    assert_eq!(changed["filter"]["resonance"], 34);
    assert_eq!(changed["filter"]["type"], "lowpass");
    assert_eq!(changed["filter"]["cutoffHz"], 7200);
    assert_eq!(changed["osc1"]["waveform"], "saw");
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.synth.osc1.waveform"),
        Some("saw".into())
    );
    assert_eq!(
        runner.menu.value_for_key("instruments.0.synth.filter.type"),
        Some("lowpass".into())
    );

    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 1;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 2;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 3;
    runner.menu.turn(10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.menu.turn(-2);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 4];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.menu.turn(-20);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 5];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["waveform"],
        "square"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["octave"],
        1
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["levelPct"],
        91
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["detuneCents"],
        10
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["type"],
        "highpass"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["cutoffHz"],
        6969
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["resonance"],
        39
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]
            ["velocitySensitivityPct"],
        80
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["ampEnv"]["attackMs"],
        28
    );
}

#[test]
pub(crate) fn sample_slot_menu_is_one_based_but_payload_remains_zero_based_and_back_exits_assign() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.sample.selectedSlot")
            .unwrap(),
        "1"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["selectedSlot"],
        0
    );

    runner.sample_assign = Some((0, 0));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.sample_assign.is_none());
}

#[test]
pub(crate) fn legacy_bus_route_normalizes_on_config_load_and_persists_canonical_route() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = legacy_payload(runner.config_payload());
    payload["runtimeConfig"]["instruments"][0]["mixer"]["route"] = json!("bus_2");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].route, "fx_bus_2");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["mixer"]["route"],
        "fx_bus_2"
    );
}

#[test]
pub(crate) fn legacy_trigger_gates_migrate_to_probability_map() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = legacy_payload(runner.config_payload());
    payload["runtimeConfig"]["layers"][0]["worlds"]["triggerGates"] = Value::Array(
        (0..GRID_WIDTH * GRID_HEIGHT)
            .map(|index| Value::Bool(index != 3))
            .collect(),
    );
    payload["runtimeConfig"]["layers"][0]["pulses"]
        .as_object_mut()
        .unwrap()
        .remove("triggerProbabilityMap");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.trigger_probability_maps[0][3], "zero");
    assert_eq!(runner.trigger_probability_maps[0][4], "full");
}

#[test]
pub(crate) fn legacy_eight_position_pan_payload_scales_to_native_pan_range() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["panPositions"] = json!(8);
    payload["runtimeConfig"]["instruments"][0]["mixer"]["panPos"] = json!(7);
    payload["runtimeConfig"]["instruments"][1]["mixer"]["panPos"] = json!(3);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].pan_pos, PAN_POSITION_COUNT - 1);
    assert_eq!(runner.instruments[1].pan_pos, PAN_POSITION_COUNT / 2);
}

#[test]
pub(crate) fn sample_slots_and_assignments_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = legacy_payload(runner.config_payload());
    payload["runtimeConfig"]["instruments"][0]["sample"]["selectedSlot"] = json!(99);
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"] = Value::Array(
        (0..12)
            .map(|index| json!({ "path": format!("s{index}.wav") }))
            .collect(),
    );
    payload["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "x": 99, "y": 99, "sampleSlot": 99, "level": "loud" }
    ]);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].selected_sample_slot, 7);
    assert_eq!(
        runner.instruments[0].sample_paths[7].as_deref(),
        Some("s7.wav")
    );
    assert_eq!(runner.instruments[0].sample_assignments[0].x, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].y, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].sample_slot, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].level, None);
}
