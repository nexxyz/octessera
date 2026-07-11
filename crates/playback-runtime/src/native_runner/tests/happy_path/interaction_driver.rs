use super::*;
use crate::native_menu::{NativeMenuAction, NativeMenuItem, NativeMenuValue};

pub(super) fn runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner
}

pub(super) fn edit_visible_params(
    runner: &mut NativeRunner,
    prefix: &str,
    is_safe: fn(&str, &NativeMenuItem) -> bool,
) {
    let keys = editable_keys_under(&runner.menu.root, prefix, is_safe);
    assert!(!keys.is_empty(), "no editable keys under {prefix}");
    for key in keys {
        edit_key_by_turns(runner, &key, 1);
    }
}

pub(super) fn item_label_for_key(item: &NativeMenuItem, target: &str) -> Option<String> {
    if item.key.as_deref() == Some(target) {
        return Some(item.label.clone());
    }
    item.children
        .iter()
        .find_map(|child| item_label_for_key(child, target))
}

pub(super) fn edit_key_by_turns(
    runner: &mut NativeRunner,
    key: &str,
    delta: i32,
) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key(key), "missing {key}");
    let _ = press_main(runner);
    let messages = turn_main(runner, delta);
    let _ = press_main(runner);
    messages
}

pub(super) fn edit_key_to_value(runner: &mut NativeRunner, key: &str, expected: &str, delta: i32) {
    let messages = edit_key_by_turns(runner, key, delta);
    assert_snapshot(messages);
    assert_eq!(runner.menu.value_for_key(key).as_deref(), Some(expected));
}

pub(super) fn select_behavior_via_menu_action(runner: &mut NativeRunner, behavior_id: &str) {
    assert!(runner.menu.focus_item_key("behaviorId"));
    let _ = press_main(runner);
    let (stack, cursor) = find_select_behavior(&runner.menu.root, behavior_id)
        .unwrap_or_else(|| panic!("missing behavior action {behavior_id}"));
    runner.menu.state.stack = stack;
    runner.menu.state.cursor = cursor;
    let messages = press_main(runner);
    assert_snapshot(messages);
}

pub(super) fn behavior_param_is_safe(key: &str, item: &NativeMenuItem) -> bool {
    !item.label.contains("Reset") && !key.contains("name")
}

pub(super) fn pulses_param_is_safe(key: &str, _: &NativeMenuItem) -> bool {
    key.contains("scan") || key.contains("mapping") || key.contains("triggerProbability")
}

pub(super) fn sample_param_is_safe(key: &str, _: &NativeMenuItem) -> bool {
    !key.contains("selectedSlot") && !key.contains("velocityLevelsEnabled")
}

pub(super) fn synth_param_is_safe(_: &str, _: &NativeMenuItem) -> bool {
    true
}

pub(super) fn midi_param_is_safe(key: &str, _: &NativeMenuItem) -> bool {
    !key.ends_with("channel")
}

pub(super) fn fx_param_is_safe(_: &str, _: &NativeMenuItem) -> bool {
    true
}

pub(super) fn pick_sample_for_selected_slot(runner: &mut NativeRunner) {
    assert!(runner.menu.focus_item_key("sample.choose:0:7"));
    let open = press_main(runner);
    assert!(open.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::SampleListRequest {
                instrument_slot: 0,
                sample_slot: 7,
                dir: "".into(),
            }]
    )));

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot: 0,
                sample_slot: 7,
                dir: "".into(),
                entries: vec![SampleEntry {
                    name: "kick.wav".into(),
                    path: "Drums/kick.wav".into(),
                    is_dir: false,
                }],
            },
        })
        .unwrap();
    assert!(runner.menu.focus_item_key("sample.pick.0.7.Drums/kick.wav"));
    let preview = button_s(runner);
    assert!(preview.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::SamplePreview {
                    instrument_slot: 0,
                    sample_slot: 7,
                    path: "Drums/kick.wav".into(),
                    velocity: 100,
                },
            }]
    )));

    let picked = press_main(runner);
    assert!(picked
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(picked.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentSlot { instrument_slot: 0, .. }
            ))
    )));
    assert!(runner.sample_browser.is_none());
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.sample.selectedSlot")
            .as_deref(),
        Some("8")
    );
    runner.make_deferred_menu_apply_due_for_test();
    assert_deferred_autosave_payload(&runner.flush_deferred_menu_apply().unwrap());
}

pub(super) fn assign_selected_sample_and_use_cell(runner: &mut NativeRunner) {
    assert!(runner.menu.focus_item_key("sample.assign.0.7"));
    let _ = press_main(runner);
    assert_eq!(runner.sample_assign, Some((0, 7)));
    assert_snapshot(grid_press(runner, 1, 2));
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .any(|assignment| assignment.x == 1 && assignment.y == 2 && assignment.sample_slot == 7));
    assert_snapshot(grid_press(runner, 1, 2));
}

pub(super) fn press_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: Some(true),
        })
        .unwrap()
}

pub(super) fn grid_press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
            request_snapshot: Some(true),
        })
        .unwrap()
}

pub(super) fn grid_release(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": x, "y": y }),
            request_snapshot: Some(true),
        })
        .unwrap()
}

pub(super) fn transport_step(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap()
}

pub(super) fn assert_snapshot(messages: Vec<RunnerMessage>) {
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

pub(super) fn contains_momentary_start(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(
                    effect,
                    RuntimePlatformEffect::AudioCommand {
                        command: RuntimeAudioCommand::MomentaryFxStart { fx_type, .. },
                    } if fx_type == "stutter"
                ))
        )
    })
}

pub(super) fn contains_momentary_stop(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(
                    effect,
                    RuntimePlatformEffect::AudioCommand {
                        command: RuntimeAudioCommand::MomentaryFxStop { .. },
                    }
                ))
        )
    })
}

fn editable_keys_under(
    item: &NativeMenuItem,
    prefix: &str,
    is_safe: fn(&str, &NativeMenuItem) -> bool,
) -> Vec<String> {
    let mut keys = Vec::new();
    collect_editable_keys(item, prefix, is_safe, &mut keys);
    keys
}

fn collect_editable_keys(
    item: &NativeMenuItem,
    prefix: &str,
    is_safe: fn(&str, &NativeMenuItem) -> bool,
    keys: &mut Vec<String>,
) {
    if let Some(key) = item.key.as_deref() {
        if key.starts_with(prefix) && is_editable(&item.value) && is_safe(key, item) {
            keys.push(key.into());
        }
    }
    for child in &item.children {
        collect_editable_keys(child, prefix, is_safe, keys);
    }
}

fn is_editable(value: &NativeMenuValue) -> bool {
    matches!(
        value,
        NativeMenuValue::Enum { .. }
            | NativeMenuValue::Number { .. }
            | NativeMenuValue::Bool { .. }
            | NativeMenuValue::Text { .. }
    )
}

fn find_select_behavior(item: &NativeMenuItem, behavior_id: &str) -> Option<(Vec<usize>, usize)> {
    find_select_behavior_inner(item, behavior_id, &mut Vec::new())
}

fn find_select_behavior_inner(
    item: &NativeMenuItem,
    behavior_id: &str,
    stack: &mut Vec<usize>,
) -> Option<(Vec<usize>, usize)> {
    for (index, child) in item.children.iter().enumerate() {
        if matches!(
            &child.value,
            NativeMenuValue::Action(NativeMenuAction::SelectBehavior(id)) if id == behavior_id
        ) {
            return Some((stack.clone(), index));
        }
        stack.push(index);
        let found = find_select_behavior_inner(child, behavior_id, stack);
        stack.pop();
        if found.is_some() {
            return found;
        }
    }
    None
}

fn turn_main(runner: &mut NativeRunner, delta: i32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
            request_snapshot: Some(true),
        })
        .unwrap()
}

fn button_s(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: Some(true),
        })
        .unwrap()
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
