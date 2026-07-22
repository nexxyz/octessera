use super::*;

fn set_bus_delay(runner: &mut NativeRunner, time_ms: i32) {
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params = json!({ "timeMs": time_ms, "feedback": 0.35, "mixPct": 35 });
    runner.menu.rebuild(runner.menu_config());
}

#[test]
pub(crate) fn delay_time_note_converts_at_current_bpm() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, 250);

    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot1.params.timeNote"),
        Some("1/8".into())
    );
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.timeNote"));
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 333);

    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 2000);
}

#[test]
pub(crate) fn delay_menu_orders_time_note_before_time_ms_and_derives_from_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, 333);

    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.timeMode"));
    assert_eq!(runner.menu.current_label(), Some("Time Mode"));
    runner.menu.turn(1);
    assert_eq!(runner.menu.current_label(), Some("Time Note"));
    runner.menu.turn(1);
    assert_eq!(runner.menu.current_label(), Some("Time ms"));
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot1.params.timeNote"),
        Some("1/4T".into())
    );
}

#[test]
pub(crate) fn invalid_delay_fields_normalize_and_non_delay_strips_timing_metadata() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{
                        "slot1": { "type": "delay", "params": { "timeMode": "banana", "timeNote": "bad", "timeMs": 9999, "feedback": 0.35, "mixPct": 35 } },
                        "slot2": { "type": "tremolo", "params": { "timeMode": "note", "timeNote": "1/4", "rateHz": 4.0, "depthPct": 60 } }
                    }]
                }
            }
        }))
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "ms");
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 2000);
    assert_eq!(runner.fx_buses[0].slot1_params["timeNote"], "1/1");
    assert!(runner.fx_buses[0].slot2_params.get("timeMode").is_none());
    assert!(runner.fx_buses[0].slot2_params.get("timeNote").is_none());
}

#[test]
pub(crate) fn selecting_delay_time_note_updates_time_ms_only_and_queues_audio() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, 250);
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.timeNote"));
    runner.menu.state.editing = true;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 333);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "note");
    assert_eq!(runner.fx_buses[0].slot1_params["timeNote"], "1/4T");
    assert_eq!(
        runner
            .menu
            .number_for_key("mixer.buses.0.slot1.params.timeMs"),
        Some(333)
    );
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, fx_type, params }
                    if fx_type == "delay"
                        && params.get("timeMs") == Some(&json!(333))
                        && !params.contains_key("timeMode")
                        && !params.contains_key("timeNote")
            ))
    )));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]
            ["timeNote"],
        "1/4T"
    );
}

#[test]
pub(crate) fn delay_time_ms_edit_remains_authoritative() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, 333);
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.timeMs"));
    runner.menu.state.editing = true;
    runner.menu.turn(-10);
    runner.apply_menu_state().unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 283);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "ms");
}

#[test]
pub(crate) fn bpm_edit_retimes_note_mode_delay_but_not_ms_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params = json!({ "timeMode": "note", "timeNote": "1/8", "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    runner.fx_buses[0].slot2_type = "delay".into();
    runner.fx_buses[0].slot2_params = json!({ "timeMode": "ms", "timeNote": "1/8", "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    runner.fx_buses[0].slot3_type = "delay".into();
    runner.fx_buses[0].slot3_params = json!({ "timeMode": "note", "timeNote": "1/4", "timeMs": 500, "feedback": 0.35, "mixPct": 35 });
    runner.menu.rebuild(runner.menu_config());

    assert!(runner.menu.focus_item_key("transport.bpm"));
    runner.menu.state.editing = true;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -60, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 500);
    assert_eq!(runner.fx_buses[0].slot2_params["timeMs"], 250);
    assert_eq!(runner.fx_buses[0].slot3_params["timeMs"], 1000);
    assert_eq!(
        runner
            .menu
            .number_for_key("mixer.buses.0.slot1.params.timeMs"),
        Some(500)
    );
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { slot_index: 0, params, .. }
                    if params.get("timeMs") == Some(&json!(500))
                        && !params.contains_key("timeMode")
                        && !params.contains_key("timeNote")
            ))
    )));
    assert_eq!(
        runner
            .menu
            .number_for_key("mixer.buses.0.slot3.params.timeMs"),
        Some(1000)
    );
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { slot_index: 2, params, .. }
                    if params.get("timeMs") == Some(&json!(1000))
            ))
    )));
}

#[test]
pub(crate) fn note_mode_delay_config_load_uses_payload_bpm() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "bpm": 60,
                "mixer": {
                    "buses": [{
                        "slot1": { "type": "delay", "params": { "timeMode": "note", "timeNote": "1/8", "timeMs": 250, "feedback": 0.35, "mixPct": 35 } }
                    }]
                }
            }
        }))
        .unwrap();

    assert_eq!(runner.transport.bpm, 60.0);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 500);
}

#[test]
pub(crate) fn note_mode_delay_config_load_uses_visible_bpm_clamp() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "bpm": 20,
                "mixer": {
                    "buses": [{
                        "slot1": { "type": "delay", "params": { "timeMode": "note", "timeNote": "1/8", "timeMs": 250, "feedback": 0.35, "mixPct": 35 } }
                    }]
                }
            }
        }))
        .unwrap();

    assert_eq!(runner.transport.bpm, 40.0);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 750);
}

#[test]
pub(crate) fn old_delay_payload_loads_as_ms_mode_and_audio_strips_timing_metadata() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": {
                    "buses": [{
                        "slot1": {
                            "type": "delay",
                            "params": {
                                "timeMs": 250,
                                "feedback": 0.35,
                                "mixPct": 35
                            }
                        }
                    }]
                }
            }
        }))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());

    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "ms");
    assert_eq!(runner.fx_buses[0].slot1_params["timeNote"], "1/8");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]
            ["timeMode"],
        "ms"
    );

    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.feedback"));
    runner.menu.state.editing = true;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { params, .. }
                    if !params.contains_key("timeMode") && !params.contains_key("timeNote")
            ))
    )));
}

#[test]
pub(crate) fn fx_bus_slot_type_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("mixer.buses.0.slot3.type", 1);
    runner.apply_menu_state().unwrap();

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
pub(crate) fn old_fx_bus_payload_defaults_slot3_to_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot3_type = "tremolo".into();
    runner.fx_buses[0].slot3_params = json!({ "rateHz": 4.0, "depthPct": 60 });
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "mixer": { "buses": [{ "slot1": { "type": "delay", "params": {} } }] }
            }
        }))
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot3_type, "none");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot3"]["type"],
        "none"
    );
}

#[test]
pub(crate) fn active_bus_fx_slot_count_includes_slot3() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot3_type = "tremolo".into();

    assert_eq!(runner.active_bus_fx_slot_count(), 1);
}

#[test]
pub(crate) fn active_bus_fx_warning_allows_exact_product_budget() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for bus in &mut runner.fx_buses {
        bus.slot1_type = "delay".into();
        bus.slot2_type = "reverb".into();
        bus.slot3_type = "eq".into();
    }

    runner.warn_if_bus_fx_over_budget();

    assert_eq!(runner.active_bus_fx_slot_count(), 12);
    assert!(runner.display.toast.is_none());
}

#[test]
pub(crate) fn active_bus_fx_warning_reports_synthetic_over_budget_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for bus in &mut runner.fx_buses {
        bus.slot1_type = "delay".into();
        bus.slot2_type = "reverb".into();
        bus.slot3_type = "eq".into();
    }
    runner.fx_buses[0].slot1_type = "none".into();
    let mut extra = runner.fx_buses[0].clone();
    extra.slot1_type = "delay".into();
    extra.slot2_type = "none".into();
    extra.slot3_type = "none".into();
    runner.fx_buses.push(extra);
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("mixer.buses.0.slot1.type"));
    runner.menu.state.editing = true;

    let _messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.active_bus_fx_slot_count(), 13);
    assert!(runner
        .display
        .toast
        .as_ref()
        .is_some_and(|toast| toast.message.contains("13/12")));
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
