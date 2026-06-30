use super::*;

#[test]
pub(crate) fn behavior_turns_apply_immediately() {
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
pub(crate) fn button_back_exits_after_immediate_structural_apply_without_double_apply() {
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

pub(crate) fn turn_main(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
        })
        .unwrap()
}

pub(crate) fn press_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap()
}

pub(crate) fn assert_no_audio_commands(messages: &[RunnerMessage]) {
    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
}

pub(crate) fn assert_no_store_save_default(messages: &[RunnerMessage]) {
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));
}

pub(crate) fn assert_deferred_autosave_payload(messages: &[RunnerMessage]) {
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
