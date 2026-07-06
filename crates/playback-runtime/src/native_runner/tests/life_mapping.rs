use super::*;

#[test]
pub(crate) fn active_life_notes_stay_in_mapping_after_menu_changes_and_navigation() {
    let mut runner = configured_life_runner();

    assert!(runner.menu.focus_item_key("parts.0.l2.pitch.scale"));
    runner.menu.state.editing = true;
    let scale_change = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_notes_in_active_mapping(&runner, &scale_change);

    assert!(runner.menu.focus_item_key("parts.0.l2.pitch.root"));
    runner.menu.state.editing = true;
    let root_change = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 3, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_notes_in_active_mapping(&runner, &root_change);

    runner.sense_parts[0].lowest_note = 50;
    runner.sense_parts[0].starting_note = 57;
    runner.sense_parts[0].highest_note = 74;
    runner.sense_parts[0].out_of_range = "clamp".into();
    runner.refresh_active_mapping_config();

    let tick_messages = pulse(&mut runner);
    assert_notes_in_active_mapping(&runner, &tick_messages);
    assert!(!musical_note_ons(&tick_messages).is_empty());

    assert!(runner
        .menu
        .focus_item_key("parts.0.l1.behaviorConfig.randomCellsPerTick"));
    runner.menu.state.editing = true;
    let behavior_config_change = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_notes_in_active_mapping(&runner, &behavior_config_change);

    let after_config_tick = pulse(&mut runner);
    assert_notes_in_active_mapping(&runner, &after_config_tick);

    runner.menu.state.editing = false;
    let navigation = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(musical_note_ons(&navigation).is_empty());
}

fn configured_life_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sense_parts[0].state_notes_enabled = true;
    runner.sense_parts[0].event_enabled = true;
    runner.sense_parts[0].scale = "minor".into();
    runner.sense_parts[0].root = "D".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport = RuntimeTransportState::Playing;
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

fn assert_notes_in_active_mapping(runner: &NativeRunner, messages: &[RunnerMessage]) {
    let mapping = runner.mapping_config_for_part(runner.active_part_index);
    for (_, note) in musical_note_ons(messages) {
        assert!((mapping.base_midi_note..=mapping.max_midi_note).contains(&i32::from(note)));
        assert!(mapping.scale.contains(&i32::from(note % 12)));
    }
}
