use super::*;

#[test]
fn fx_bus_slot1_type_turn_applies_immediately() {
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
fn fx_bus_slot2_type_turn_applies_immediately() {
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
fn global_fx_type_turn_applies_immediately() {
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
fn instrument_type_turn_applies_immediately() {
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
fn synth_filter_type_turn_is_not_misclassified_as_instrument_type_draft() {
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
fn instrument_route_turn_applies_immediately() {
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

#[test]
fn behavior_turns_apply_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    assert!(runner.menu.focus_item_key("behaviorId"));
    runner.menu.state.editing = true;

    let first_messages = turn_main(&mut runner, 1);
    let messages = turn_main(&mut runner, 1);

    let selected = runner.menu.selected_behavior().unwrap();
    assert_eq!(runner.behavior.id(), selected);
    assert!(runner.flush_deferred_menu_apply().unwrap().is_empty());
    assert_eq!(runner.part_behavior_ids[0], selected);
    assert_no_store_save_default(&first_messages);
    assert_no_store_save_default(&messages);

    let messages = press_main(&mut runner);
    assert_eq!(runner.behavior.id(), selected);
    assert_eq!(runner.part_behavior_ids[0], selected);
    assert_no_store_save_default(&messages);
    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    assert_deferred_autosave_payload(&messages);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { payload, mode }
                    if mode.as_deref() == Some("deferred")
                        && payload["runtimeConfig"]["parts"][0]["l1"]["behaviorId"] == selected
            ))
    )));
}

#[test]
fn button_back_exits_after_immediate_structural_apply_without_double_apply() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("mixer.buses.0.slot1.type"));
    runner.menu.state.editing = true;
    let _ = turn_main(&mut runner, 1);

    assert_eq!(runner.fx_buses[0].slot1_type, "tremolo");

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].slot1_type, "tremolo");
    assert!(!runner.menu.state.editing);
    assert_no_audio_commands(&messages);
}

fn turn_main(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
        })
        .unwrap()
}

fn press_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap()
}

fn assert_no_audio_commands(messages: &[RunnerMessage]) {
    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
}

fn assert_no_store_save_default(messages: &[RunnerMessage]) {
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));
}

fn assert_deferred_autosave_payload(messages: &[RunnerMessage]) {
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { mode, .. }
                    if mode.as_deref() == Some("deferred")
            ))
    )));
}
