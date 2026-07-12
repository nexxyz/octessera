use super::*;

#[test]
pub(crate) fn behavior_selection_applies_immediately() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    select_behavior(&mut runner, "keys");

    assert_eq!(runner.behavior.id(), "keys");
    assert_eq!(runner.layer_behavior_ids[0], "keys");
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
                        && payload["runtimeConfig"]["layers"][0]["worlds"]["behaviorId"] == "keys"
            ))
    )));
}

#[test]
pub(crate) fn behavior_switch_rematerializes_visible_world_params_back_and_forth() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("behaviorId"));
    assert_visible_world_params(&runner, &["Spawn Count"], &["Quantize"]);

    select_behavior(&mut runner, "keys");
    assert_visible_world_params(&runner, &["Quantize"], &["Spawn Count"]);

    select_behavior(&mut runner, "life");
    assert_visible_world_params(&runner, &["Spawn Count"], &["Quantize"]);
}

fn assert_visible_world_params(runner: &NativeRunner, expected: &[&str], stale: &[&str]) {
    let snapshot = runner.menu.snapshot();
    let lines = snapshot.lines.join("\n");
    for label in expected {
        assert!(lines.contains(label), "missing {label} in {lines}");
    }
    for label in stale {
        assert!(!lines.contains(label), "stale {label} in {lines}");
    }
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
            request_snapshot: None,
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
            request_snapshot: None,
        })
        .unwrap()
}

pub(crate) fn press_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
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
