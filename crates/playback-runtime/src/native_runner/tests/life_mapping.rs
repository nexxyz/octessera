use super::*;

#[test]
pub(crate) fn active_life_notes_stay_in_mapping_after_menu_changes_and_navigation() {
    let mut runner = configured_life_runner();

    assert!(runner.menu.focus_item_key("layers.0.pulses.pitch.scale"));
    main_press(&mut runner);
    main_press(&mut runner);
    main_turn(&mut runner, 1);
    let scale_change = main_press(&mut runner);
    assert_notes_in_active_mapping(&runner, &scale_change);

    assert!(runner.menu.focus_item_key("layers.0.pulses.pitch.root"));
    main_press(&mut runner);
    main_turn(&mut runner, 3);
    let root_change = main_press(&mut runner);
    assert_notes_in_active_mapping(&runner, &root_change);

    runner.pulses_layers[0].lowest_note = 50;
    runner.pulses_layers[0].starting_note = 57;
    runner.pulses_layers[0].highest_note = 74;
    runner.pulses_layers[0].out_of_range = "clamp".into();
    runner.refresh_active_mapping_config();

    let tick_messages = pulse(&mut runner);
    assert_notes_in_active_mapping(&runner, &tick_messages);
    assert!(!musical_note_ons(&tick_messages).is_empty());

    assert!(runner
        .menu
        .focus_item_key("layers.0.worlds.behaviorConfig.randomCellsPerTick"));
    main_press(&mut runner);
    main_turn(&mut runner, 2);
    let behavior_config_change = main_press(&mut runner);
    assert_notes_in_active_mapping(&runner, &behavior_config_change);

    let after_config_tick = pulse(&mut runner);
    assert_notes_in_active_mapping(&runner, &after_config_tick);

    let navigation = main_turn(&mut runner, 1);
    assert!(musical_note_ons(&navigation).is_empty());
}

fn configured_life_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].state_notes_enabled = true;
    runner.pulses_layers[0].event_enabled = true;
    runner.pulses_layers[0].scale = "natural_minor".into();
    runner.pulses_layers[0].root = "D".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

fn pulse(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap()
}

fn main_press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn main_turn(runner: &mut NativeRunner, delta: i8) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn assert_notes_in_active_mapping(runner: &NativeRunner, messages: &[RunnerMessage]) {
    let mapping = runner.mapping_config_for_layer(runner.active_layer_index);
    for (_, note) in musical_note_ons(messages) {
        assert!((mapping.base_midi_note..=mapping.max_midi_note).contains(&i32::from(note)));
        assert!(mapping.scale.contains(&i32::from(note % 12)));
    }
}
