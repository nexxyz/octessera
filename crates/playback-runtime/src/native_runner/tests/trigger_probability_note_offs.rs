use super::*;

const NOTE_OFFS_KEY: &str = "layers.0.link.triggerProbabilityNoteOffs";

fn held_keys_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.note_behavior = NoteBehavior::Hold;
    runner.instruments[0] = instrument;
    runner.sync_engine_runtime_config();
    runner
}

fn edit_menu_key(runner: &mut NativeRunner, key: &str, apply: impl FnOnce(&mut NativeMenuModel)) {
    runner.menu.rebuild(runner.menu_config());
    apply(&mut runner.menu);
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn gate_layer(runner: &mut NativeRunner, mode: TriggerProbabilityMode) {
    let key = "layers.0.link.triggerProbabilityMode";
    edit_menu_key(runner, key, |menu| {
        assert!(menu.set_enum_value_for_key(key, mode.wire_name()), "{key}");
    });
    assert_eq!(runner.layers[0].link.trigger_probability_mode, mode);
}

fn note_ons(messages: &[RunnerMessage]) -> usize {
    routed_events(messages, false)
        .iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOn { .. }))
        .count()
}

fn note_offs(messages: &[RunnerMessage]) -> usize {
    routed_events(messages, false)
        .iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
        .count()
}

fn press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    device_input(runner, json!({ "type": "grid_press", "x": 2, "y": 3 }))
}

fn release(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    device_input(runner, json!({ "type": "grid_release", "x": 2, "y": 3 }))
}

#[test]
pub(crate) fn gating_a_held_key_to_zero_still_releases_it_by_default() {
    let mut runner = held_keys_runner();
    assert!(!runner.layers[0].link.trigger_probability_note_offs);
    assert_eq!(note_ons(&press(&mut runner)), 1);
    gate_layer(&mut runner, TriggerProbabilityMode::Zero);

    assert_eq!(note_offs(&release(&mut runner)), 1);
    assert!(runner.layers[0].emitted_note_owners.is_empty());
    assert_eq!(note_ons(&press(&mut runner)), 0);
    release(&mut runner);
    gate_layer(&mut runner, TriggerProbabilityMode::Full);
    assert_eq!(note_ons(&press(&mut runner)), 1);
}

#[test]
pub(crate) fn rolling_note_offs_lets_the_gate_swallow_the_release() {
    let mut runner = held_keys_runner();
    edit_menu_key(&mut runner, NOTE_OFFS_KEY, |menu| {
        assert!(menu.set_bool_value_for_key(NOTE_OFFS_KEY, true));
    });
    assert!(runner.layers[0].link.trigger_probability_note_offs);
    assert_eq!(note_ons(&press(&mut runner)), 1);
    gate_layer(&mut runner, TriggerProbabilityMode::Zero);

    assert_eq!(note_offs(&release(&mut runner)), 0);
}
