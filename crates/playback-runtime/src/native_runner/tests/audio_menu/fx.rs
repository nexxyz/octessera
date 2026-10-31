use super::*;

fn set_bus_delay(runner: &mut NativeRunner, mode: &str, time_ms: i32) {
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot1_params =
        json!({ "timeMode": mode, "timeMs": time_ms, "feedback": 0.35, "mixPct": 35 });
    runner.menu.rebuild(runner.menu_config());
}

fn turn_main(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

#[test]
pub(crate) fn delay_time_note_converts_at_current_bpm() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, "note", 250);

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
    turn_main(&mut runner, 1);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 333);

    turn_main(&mut runner, 5);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 2000);
}

#[test]
pub(crate) fn delay_menu_shows_only_the_time_row_of_its_time_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, "ms", 333);
    let time_ms = "mixer.buses.0.slot1.params.timeMs";
    let time_note = "mixer.buses.0.slot1.params.timeNote";

    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.timeMode"));
    let labels = |runner: &NativeRunner| {
        runner
            .menu
            .current_siblings()
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(&runner),
        [
            "Type",
            "Mix %",
            "Spread %",
            "Time Mode",
            "Time ms",
            "Feedback"
        ]
    );
    assert_eq!(runner.menu.value_for_key(time_note), None);

    runner.menu.state.editing = true;
    turn_main(&mut runner, 1);
    assert_eq!(runner.menu.current_label(), Some("Time Mode"));
    assert!(runner.menu.state.editing);
    assert_eq!(
        labels(&runner),
        [
            "Type",
            "Mix %",
            "Spread %",
            "Time Mode",
            "Time Note",
            "Feedback"
        ]
    );
    assert_eq!(runner.menu.value_for_key(time_note), Some("1/4T".into()));
    assert_eq!(runner.menu.number_for_key(time_ms), None);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 333);

    turn_main(&mut runner, -1);
    assert_eq!(runner.menu.number_for_key(time_ms), Some(333));
    assert_eq!(runner.menu.value_for_key(time_note), None);
}

#[test]
pub(crate) fn delay_time_rows_no_longer_switch_time_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, "ms", 250);
    let key = "mixer.buses.0.slot1.params.timeNote";
    assert!(runner.set_fx_bus_param(0, 0, "timeNote", key, json!("1/4")));
    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "ms");
    assert_eq!(runner.fx_buses[0].slot1_params["timeNote"], "1/4");
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 250);

    set_bus_delay(&mut runner, "note", 250);
    let key = "mixer.buses.0.slot1.params.timeMs";
    assert!(runner.set_fx_bus_param(0, 0, "timeMs", key, json!(400)));
    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "note");
    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 250);
}

#[test]
pub(crate) fn aux_binding_to_the_hidden_time_ms_row_reports_the_synced_time() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, "note", 250);
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("mixer.buses.0.slot1.params.timeMs".into()),
        press_action: None,
    });

    runner.handle_aux_turn(0, 1).unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 250);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "note");
    let toast = runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.clone());
    assert!(
        toast
            .as_deref()
            .is_some_and(|toast| toast.contains("Time ms") && toast.contains("250")),
        "{toast:?}"
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
    let _ = runner.messages_with_snapshot().unwrap();
    runner.transport.bpm = 120.0;
    set_bus_delay(&mut runner, "note", 250);
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
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, fx_type, params, .. }
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
    set_bus_delay(&mut runner, "ms", 333);
    assert!(runner
        .menu
        .focus_item_key("mixer.buses.0.slot1.params.timeMs"));
    runner.menu.state.editing = true;
    runner.menu.turn(-10);
    runner.apply_current_menu_edit().unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 283);
    assert_eq!(runner.fx_buses[0].slot1_params["timeMode"], "ms");
}

#[test]
pub(crate) fn bpm_edit_retimes_note_mode_delay_but_not_ms_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
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
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -60, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_params["timeMs"], 500);
    assert_eq!(runner.fx_buses[0].slot2_params["timeMs"], 250);
    assert_eq!(runner.fx_buses[0].slot3_params["timeMs"], 1000);
    assert_eq!(
        runner
            .menu
            .number_for_key("mixer.buses.0.slot2.params.timeMs"),
        Some(250)
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
        None
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
                "transport": { "bpm": 60 },
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
                "transport": { "bpm": 20 },
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
pub(crate) fn sparse_delay_payload_defaults_to_ms_mode_and_audio_strips_timing_metadata() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
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
    let _ = runner.messages_with_snapshot().unwrap();
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
                RuntimeAudioCommand::SetFxBusParam {
                    slot_index: 0,
                    param: realtime_engine::synth::FxParamId::Feedback,
                    ..
                }
            ))
    )));
}

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
pub(crate) fn sparse_fx_bus_payload_defaults_omitted_slot3_to_none() {
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
pub(crate) fn invalid_bus_and_global_fx_types_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = unversioned_payload(runner.config_payload());
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "pitch_shift", "params": {} });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "delay", "params": {} });

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.fx_buses[0].slot1_type, "none");
    assert_eq!(runner.global_fx_slots[0], "none");
}

#[test]
pub(crate) fn delay_auto_map_puts_the_active_time_row_on_aux_three() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.bpm = 120.0;
    for (mode, field) in [("ms", "timeMs"), ("note", "timeNote")] {
        set_bus_delay(&mut runner, mode, 250);
        assert!(runner
            .menu
            .focus_item_key("mixer.buses.0.slot1.params.feedback"));
        let turn = runner.effective_aux_slot(2).turn.expect("Aux 3 turn");
        assert_eq!(turn.key, format!("mixer.buses.0.slot1.params.{field}"));
    }
}
