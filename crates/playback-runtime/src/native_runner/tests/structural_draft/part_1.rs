use super::*;

#[test]
pub(crate) fn fx_bus_slot1_type_turn_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.buses.0.slot1.type"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();

    let selected = runner
        .menu
        .value_for_key("mixer.buses.0.slot1.type")
        .unwrap();
    assert_eq!(selected, "delay");
    assert_eq!(runner.fx_buses[0].slot1_type, selected);
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, fx_type, .. }
                    if fx_type == "delay"
            ))
    )));
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());

    let flushed = press_main(&mut runner);
    assert_eq!(runner.fx_buses[0].slot1_type, selected);
    assert_eq!(runner.audio_config_revision, 0);
    assert_no_audio_commands(&flushed);
}

#[test]
pub(crate) fn fx_bus_slot2_type_turn_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.buses.0.slot2.type"));
    runner.menu.state.editing = true;

    let messages = turn_main(&mut runner, 1);

    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot2.type")
            .as_deref(),
        Some("tremolo")
    );
    assert_eq!(runner.fx_buses[0].slot2_type, "tremolo");
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 1, fx_type, .. }
                    if fx_type == "tremolo"
            ))
    )));
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());

    let flushed = press_main(&mut runner);
    assert_eq!(runner.fx_buses[0].slot2_type, "tremolo");
    assert_eq!(runner.audio_config_revision, 0);
    assert_no_audio_commands(&flushed);
}

#[test]
pub(crate) fn global_fx_type_turn_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.master.slots.0.type"));
    runner.menu.state.editing = true;

    let messages = turn_main(&mut runner, 1);

    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.master.slots.0.type")
            .as_deref(),
        Some("vinyl")
    );
    assert_eq!(runner.global_fx_slots[0], "vinyl");
    assert_eq!(runner.audio_config_revision, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetGlobalFxSlot { slot_index: 0, fx_type, .. }
                    if fx_type == "vinyl"
            ))
    )));
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());

    let flushed = press_main(&mut runner);
    assert_eq!(runner.global_fx_slots[0], "vinyl");
    assert_eq!(runner.audio_config_revision, 0);
    assert_no_audio_commands(&flushed);
}

#[test]
pub(crate) fn instrument_type_turn_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    assert!(runner.menu.focus_item_key("instruments.0.type"));
    runner.menu.state.editing = true;

    let messages = turn_main(&mut runner, 1);

    assert_eq!(
        runner.menu.value_for_key("instruments.0.type").as_deref(),
        Some("sampler")
    );
    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.audio_config_revision, 1);
    assert_no_store_save_default(&messages);
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());

    let messages = press_main(&mut runner);
    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.audio_config_revision, 1);
    assert_no_audio_commands(&messages);
    assert_no_store_save_default(&messages);
    runner.make_deferred_menu_apply_due_for_test();
    assert_deferred_autosave_payload(&runner.flush_deferred_menu_apply().unwrap());
}

#[test]
pub(crate) fn synth_filter_type_turn_is_not_misclassified_as_instrument_type_draft() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner
        .menu
        .focus_item_key("instruments.0.synth.filter.type"));
    runner.menu.state.editing = true;

    let messages = turn_main(&mut runner, 1);

    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.synth.filter.type")
            .as_deref(),
        Some("highpass")
    );
    assert_eq!(
        runner.instruments[0].synth_config["filter"]["type"],
        "highpass"
    );
    assert_eq!(runner.audio_config_revision, 1);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetAudioConfig { revision: 1, .. }
            ))
    )));
}

#[test]
pub(crate) fn instrument_route_turn_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    let _ = runner.messages_with_snapshot().unwrap();
    assert!(runner.menu.focus_item_key("instruments.0.mixer.route"));
    runner.menu.state.editing = true;

    let messages = turn_main(&mut runner, 1);

    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.mixer.route")
            .as_deref(),
        Some("fx_bus_1")
    );
    assert_eq!(runner.instruments[0].route, "fx_bus_1");
    assert_eq!(runner.audio_config_revision, 1);
    assert_no_store_save_default(&messages);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetAudioConfig { revision: 1, .. }
            ))
    )));

    let messages = press_main(&mut runner);
    assert_eq!(runner.instruments[0].route, "fx_bus_1");
    assert_eq!(runner.audio_config_revision, 1);
    assert_no_store_save_default(&messages);
    assert_no_audio_commands(&messages);
    runner.make_deferred_menu_apply_due_for_test();
    assert_deferred_autosave_payload(&runner.flush_deferred_menu_apply().unwrap());
}
