use super::*;

fn duck_param_payload(runner: &NativeRunner, key: &str, value: Value) -> Value {
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot2"]["params"][key] = value;
    payload
}

fn duck_source_tap_payload(runner: &NativeRunner) -> Value {
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] = json!({
        "type": "duck",
        "params": {
            "source": "I1",
            "sourceTap": "pre",
            "threshold": 0.08,
            "amountPct": 60,
            "attackMs": 8,
            "releaseMs": 160
        }
    });
    payload
}

#[test]
pub(crate) fn duck_defaults_use_pre_source_tap() {
    assert_eq!(fx_default_params("duck")["sourceTap"], "pre");
}

#[test]
pub(crate) fn duck_source_tap_schema_accepts_pre_post_and_omission() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = duck_source_tap_payload(&runner);

    for value in [json!("pre"), json!("post")] {
        payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["sourceTap"] = value;
        validate_config_payload(&payload).unwrap();
    }
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]
        .as_object_mut()
        .unwrap()
        .remove("sourceTap");
    validate_config_payload(&payload).unwrap();
}

#[test]
pub(crate) fn duck_source_tap_schema_rejects_other_values() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = duck_source_tap_payload(&runner);

    for value in [json!("Pre"), json!("sidechain"), json!(0), Value::Null] {
        payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["sourceTap"] = value;
        assert!(validate_config_payload(&payload)
            .unwrap_err()
            .contains("sourceTap"));
    }
}

#[test]
pub(crate) fn omitted_duck_source_tap_defaults_to_pre() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{
                        "slot1": {
                            "type": "duck",
                            "params": {
                                "source": "I1",
                                "threshold": 0.08,
                                "amountPct": 60,
                                "attackMs": 8,
                                "releaseMs": 160
                            }
                        }
                    }]
                }
            }
        }))
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["sourceTap"], "pre");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]
            ["sourceTap"],
        "pre"
    );
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot1.params.sourceTap"),
        Some("pre".into())
    );
}

#[test]
pub(crate) fn duck_source_tap_fast_edit_persists_and_queues_audio_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    runner.fx_buses[0].slot1_type = "duck".into();
    runner.fx_buses[0].slot1_params = fx_default_params("duck");
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.sourceTap"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["sourceTap"], "post");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]
            ["sourceTap"],
        "post"
    );
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot {
                    bus_index: 0,
                    slot_index: 0,
                    fx_type,
                    params, ..
                } if fx_type == "duck" && params.get("sourceTap") == Some(&json!("post"))
            ))
    )));
}

#[test]
pub(crate) fn duck_fx_menu_serializes_accepted_boundaries_without_rescaling() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot2_type = "duck".into();
    runner.fx_buses[0].slot2_params = json!({
        "source": "I1",
        "threshold": 0.08,
        "amountPct": 60,
        "attackMs": 8,
        "releaseMs": 160
    });
    runner.menu.rebuild(runner.menu_config());
    for (key, value) in [
        ("threshold", 100),
        ("amountPct", 100),
        ("attackMs", 500),
        ("releaseMs", 5000),
    ] {
        assert!(
            runner
                .menu
                .set_number_value_for_key(&format!("mixer.buses.0.slot2.params.{key}"), value),
            "{key} menu value was not changed"
        );
    }

    runner.apply_menu_state().unwrap();
    let params = &runner.fx_buses[0].slot2_params;
    assert_eq!(params["threshold"], 1.0);
    assert_eq!(params["amountPct"], 100);
    assert_eq!(params["attackMs"], 500);
    assert_eq!(params["releaseMs"], 5000);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot2"]["params"],
        *params
    );
}

#[test]
pub(crate) fn duck_fx_schema_rejects_values_outside_canonical_ranges() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (key, below, above) in [
        ("threshold", json!(-0.001), json!(1.001)),
        ("amountPct", json!(-1), json!(100.001)),
        ("attackMs", json!(0), json!(500.001)),
        ("releaseMs", json!(0), json!(5000.001)),
    ] {
        let below_payload = duck_param_payload(&runner, key, below);
        assert_rejected_without_byte_changes(&mut runner, below_payload);
        let above_payload = duck_param_payload(&runner, key, above);
        assert_rejected_without_byte_changes(&mut runner, above_payload);
    }
}
