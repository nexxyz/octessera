use super::*;

fn scanning_runner(scan_sections: u8, scan_unit: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.link_layers[0].scan_mode = "scanning".into();
    runner.link_layers[0].scan_axis = "rows".into();
    runner.link_layers[0].scan_unit = scan_unit.into();
    runner.link_layers[0].scan_sections = scan_sections;
    runner.link_layers[0].scanned_slot = 0;
    runner.link_layers[0].scanned_action = "note_on".into();
    runner.link_layers[0].scanned_empty_action = "none".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
}

fn add_cell(runner: &mut NativeRunner, x: usize, y: usize) {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
            request_snapshot: None,
        })
        .unwrap();
}

fn pulse(runner: &mut NativeRunner, pulses: u32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap()
}

fn start(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner.midi_enabled = true;
    runner.send(HostMessage::MidiRealtimeStart).unwrap()
}

fn replace_sequencer_layer(runner: &mut NativeRunner, layer_index: usize) {
    let saved_state = runner.serialized_state_for_layer(layer_index).unwrap();
    runner
        .replace_layer_engine_with_config(
            layer_index,
            platform_core::get_native_behavior("sequencer").unwrap(),
            Value::Null,
            Some(saved_state),
        )
        .unwrap();
}

#[test]
fn clearing_running_sequencer_preserves_section_and_fractional_phase() {
    let mut runner = scanning_runner(2, "1/16");
    runner.transport.transport = RuntimeTransportState::Playing;
    pulse(&mut runner, 56);
    assert_eq!(runner.transport.layer_ticks[0], 9);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 2);

    runner.display.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner.display.ui.shift_held = false;
    assert_eq!(runner.transport.layer_ticks[0], 9);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 2);

    add_cell(&mut runner, 4, 1);
    let messages = pulse(&mut runner, 4);
    assert_eq!(musical_note_ons(&messages).len(), 1);
}

#[test]
fn active_and_inactive_sequencer_replacements_keep_canonical_phase() {
    let mut runner = scanning_runner(1, "1/16");
    runner.select_layer_behavior(1, "sequencer").unwrap();
    runner.select_active_layer(1).unwrap();
    runner.link_layers[1].scan_mode = "scanning".into();
    runner.link_layers[1].scan_axis = "rows".into();
    runner.link_layers[1].scan_unit = "1/8".into();
    runner.link_layers[1].event_enabled = false;
    runner.link_layers[1].scanned_slot = 0;
    runner.link_layers[1].scanned_action = "note_on".into();
    runner.link_layers[1].scanned_empty_action = "none".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    add_cell(&mut runner, 0, 7);
    runner.select_active_layer(0).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.tick = 9;
    runner.transport.layer_ticks[0] = 9;
    runner.transport.layer_ticks[1] = 7;
    runner.transport.layer_pulse_accumulators[0] = 2;
    runner.transport.layer_pulse_accumulators[1] = 11;

    replace_sequencer_layer(&mut runner, 0);
    replace_sequencer_layer(&mut runner, 1);

    assert_eq!(runner.transport.layer_ticks[0], 9);
    assert_eq!(runner.transport.layer_ticks[1], 7);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 2);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 11);

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(musical_note_ons(&messages).len(), 1);
    assert_eq!(runner.transport.layer_ticks[1], 8);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 0);
}

#[test]
fn origin_prime_uses_each_scanning_layer_rate() {
    let mut runner = scanning_runner(1, "1/16");
    runner.select_layer_behavior(1, "sequencer").unwrap();
    runner.select_active_layer(1).unwrap();
    runner.link_layers[1].scan_mode = "scanning".into();
    runner.link_layers[1].scan_axis = "rows".into();
    runner.link_layers[1].scan_unit = "1/8".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.select_active_layer(0).unwrap();

    start(&mut runner);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 6);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 12);
    pulse(&mut runner, 1);
    assert_eq!(runner.transport.layer_ticks[0], 1);
    assert_eq!(runner.transport.layer_ticks[1], 1);
    pulse(&mut runner, 5);
    assert_eq!(runner.transport.layer_ticks[0], 2);
    assert_eq!(runner.transport.layer_ticks[1], 1);
}

#[test]
fn transport_origin_primes_only_scanning_sequencers_and_first_pulse_scans_tick_zero() {
    let mut runner = scanning_runner(1, "1/16");
    runner.midi_enabled = true;
    add_cell(&mut runner, 0, 0);
    let start_messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(musical_note_ons(&start_messages).is_empty());
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 6);
    assert_eq!(musical_note_ons(&pulse(&mut runner, 1)).len(), 1);

    let duplicate_start = start(&mut runner);
    assert!(musical_note_ons(&duplicate_start).is_empty());
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 6);
    assert_eq!(musical_note_ons(&pulse(&mut runner, 1)).len(), 1);

    runner.send(HostMessage::MidiRealtimeStop).unwrap();
    runner.send(HostMessage::MidiRealtimeContinue).unwrap();
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 6);
    assert_eq!(musical_note_ons(&pulse(&mut runner, 1)).len(), 1);
    runner.transport.transport = RuntimeTransportState::Paused;
    runner.transport.layer_pulse_accumulators[0] = 1;
    runner.send(HostMessage::MidiRealtimeContinue).unwrap();
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 1);

    let mut immediate = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    immediate.send(HostMessage::MidiRealtimeStart).unwrap();
    assert_eq!(immediate.transport.layer_pulse_accumulators[0], 0);
}

#[test]
fn sequencer_origin_batch_partitioning_is_invariant() {
    let mut batched = scanning_runner(1, "1/16");
    let mut partitioned = scanning_runner(1, "1/16");
    for y in 0..3 {
        add_cell(&mut batched, 0, y);
        add_cell(&mut partitioned, 0, y);
    }
    start(&mut batched);
    start(&mut partitioned);

    let batched_notes = musical_note_ons(&pulse(&mut batched, 13));
    let mut partitioned_notes = Vec::new();
    for pulses in [1, 5, 7] {
        partitioned_notes.extend(musical_note_ons(&pulse(&mut partitioned, pulses)));
    }

    assert_eq!(batched_notes, partitioned_notes);
    assert_eq!(
        batched.transport.layer_pulse_accumulators,
        partitioned.transport.layer_pulse_accumulators
    );
    assert_eq!(
        batched.transport.layer_ticks,
        partitioned.transport.layer_ticks
    );
}

#[test]
fn sparse_sectioned_scan_has_leading_rests_and_continues_across_pages() {
    let mut runner = scanning_runner(2, "1/16");
    add_cell(&mut runner, 4, 0);
    add_cell(&mut runner, 4, 1);
    start(&mut runner);

    assert!(musical_note_ons(&pulse(&mut runner, 1)).is_empty());
    assert_eq!(musical_note_ons(&pulse(&mut runner, 48)).len(), 1);
    assert_eq!(musical_note_ons(&pulse(&mut runner, 6)).len(), 1);
}

#[test]
fn pause_resume_and_combined_trigger_gate_preserve_sequencer_phase() {
    let mut runner = scanning_runner(1, "1/16");
    add_cell(&mut runner, 0, 0);
    add_cell(&mut runner, 0, 1);
    add_cell(&mut runner, 0, 2);
    add_cell(&mut runner, 0, 3);
    start(&mut runner);
    assert_eq!(musical_note_ons(&pulse(&mut runner, 1)).len(), 1);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(musical_note_ons(&pulse(&mut runner, 5)).len(), 1);

    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
    runner.display.ui.combined_modifier_held = false;
    assert!(musical_note_ons(&pulse(&mut runner, 5)).is_empty());
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 5);
    assert!(musical_note_ons(&pulse(&mut runner, 1)).is_empty());
    assert_eq!(runner.transport.layer_ticks[0], 3);
    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.display.ui.combined_modifier_held = false;
    assert_eq!(musical_note_ons(&pulse(&mut runner, 6)).len(), 1);
}

#[test]
fn external_resync_boundary_emits_origin_tick_at_zero_without_following_duplicate() {
    let mut runner = scanning_runner(1, "1/16");
    runner.midi_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    runner.midi_clock_in_enabled = true;
    add_cell(&mut runner, 0, 0);
    add_cell(&mut runner, 0, 1);
    start(&mut runner);
    let origin_notes = musical_note_ons(
        &runner
            .send(HostMessage::MidiRealtimeClock { pulses: 1 })
            .unwrap(),
    );
    assert_eq!(origin_notes.len(), 1);

    runner.transport.current_ppqn_pulse = 95;
    runner.transport.layer_pulse_accumulators[0] = 5;
    runner.transport.pending_resync = true;
    let boundary = runner
        .send(HostMessage::MidiRealtimeClock { pulses: 1 })
        .unwrap();
    assert_eq!(musical_note_ons(&boundary), origin_notes);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert_eq!(runner.transport.layer_ticks[0], 1);
    assert_eq!(runner.transport.tick, 1);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
    assert!(boundary.iter().any(|message| matches!(
        message,
        RunnerMessage::RuntimeStatus { status } if status.current_ppqn_pulse == 0
    )));

    let following = runner
        .send(HostMessage::MidiRealtimeClock { pulses: 1 })
        .unwrap();
    assert!(musical_note_ons(&following).is_empty());
    assert_eq!(runner.transport.current_ppqn_pulse, 1);
    assert_eq!(runner.transport.layer_ticks[0], 1);
}
