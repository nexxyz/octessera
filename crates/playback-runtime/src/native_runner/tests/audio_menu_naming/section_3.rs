use super::*;

#[test]
pub(crate) fn turning_instrument_and_bus_auto_name_on_replaces_manual_names() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "manual inst".into();
    runner.instruments[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot2_type = "duck".into();
    runner.fx_buses[0].name = "manual bus".into();
    runner.fx_buses[0].auto_name = false;
    runner.menu.rebuild(runner.menu_config());

    runner.menu.turn_key("instruments.0.autoName", 1);
    runner.menu.turn_key("mixer.buses.0.autoName", 1);
    runner.apply_menu_state().unwrap();

    assert!(runner.instruments[0].auto_name);
    assert_eq!(runner.instruments[0].name, "Sampler");
    assert!(runner.fx_buses[0].auto_name);
    assert_eq!(runner.fx_buses[0].name, "Delay+Duck");
}

#[test]
pub(crate) fn worlds_layer_config_always_exposes_auto_name() {
    for behavior_id in ["life", "none", "brain"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: behavior_id.into(),
            ..NativeRunnerConfig::default()
        })
        .unwrap();

        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        let entered = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();

        let snapshot = snapshot_from(&entered);
        let lines = snapshot["display"]["lines"].as_array().unwrap();
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Layer Label")),
            "{behavior_id} should show Layer Label"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.as_str().unwrap_or("").contains("Auto Label")),
            "{behavior_id} should show Auto Label"
        );
    }
}

#[test]
pub(crate) fn behavior_change_updates_active_layer_auto_name_label() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    select_behavior(&mut runner, "keys");

    assert_eq!(runner.layer_behavior_ids[0], "keys");
    runner.menu.back();
    runner.menu.rebuild(runner.menu_config());
    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> L1: keys >"));
}
