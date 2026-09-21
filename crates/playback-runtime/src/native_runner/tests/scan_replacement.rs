use super::*;

fn scanning_runner(behavior_id: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: behavior_id.into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.link_layers[0].scan_mode = "scanning".into();
    runner.link_layers[0].scan_axis = "rows".into();
    runner.link_layers[0].scan_unit = "1/16".into();
    runner.link_layers[0].event_enabled = false;
    runner.link_layers[0].scanned_slot = 0;
    runner.link_layers[0].scanned_action = "note_on".into();
    runner.link_layers[0].scanned_empty_slot = 0;
    runner.link_layers[0].scanned_empty_action = "note_on".into();
    runner.link_layers[0].y_pitch_enabled = true;
    runner.link_layers[0].y_pitch_steps = 1;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.tick = 3;
    runner.transport.layer_ticks[0] = 3;
    runner.transport.layer_pulse_accumulators[0] = 5;
    runner
}

fn replace_with_current_behavior(runner: &mut NativeRunner, saved_state: Option<Value>) {
    let behavior = runner.behavior;
    runner
        .replace_layer_engine_with_config(
            runner.active_layer_index,
            behavior,
            runner.behavior_config.clone(),
            saved_state,
        )
        .unwrap();
}

fn next_boundary_notes(runner: &mut NativeRunner) -> Vec<(u8, u8)> {
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    musical_note_ons(&messages)
}

#[test]
fn generic_scanning_replacement_restores_transport_cursor_for_representative_behaviors() {
    for behavior_id in ["keys", "looper", "weave", "life"] {
        let mut runner = scanning_runner(behavior_id);
        if matches!(behavior_id, "weave" | "life") {
            runner.engine.tick(runner.transport.bpm as f32).unwrap();
        }
        replace_with_current_behavior(&mut runner, None);
        let canonical_notes = next_boundary_notes(&mut runner);

        let mut zero_cursor = scanning_runner(behavior_id);
        zero_cursor.transport.layer_ticks[0] = 0;
        replace_with_current_behavior(&mut zero_cursor, None);
        let zero_notes = next_boundary_notes(&mut zero_cursor);

        assert!(!canonical_notes.is_empty(), "{behavior_id} emitted no scan");
        assert_ne!(
            canonical_notes, zero_notes,
            "{behavior_id} ignored scan cursor"
        );
        assert_eq!(runner.transport.layer_ticks[0], 4);
        assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
        match behavior_id {
            "weave" => assert!(matches!(
                runner.engine.state(),
                platform_core::NativeBehaviorState::Pattern(state) if state.phase == 1
            )),
            "life" => assert!(matches!(
                runner.engine.state(),
                platform_core::NativeBehaviorState::Life(state) if state.tick_counter == 1
            )),
            _ => {}
        }
    }
}

#[test]
fn behavior_owned_state_contracts_remain_separate_from_scan_cursor_replacement() {
    let mut keys = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    keys.send(HostMessage::DeviceInput {
        input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        request_snapshot: None,
    })
    .unwrap();
    replace_with_current_behavior(&mut keys, None);
    assert!(!keys.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);

    let mut looper = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "looper".into(),
        behavior_config: json!({ "mode": "overdub", "lengthSteps": 2 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    looper
        .engine
        .on_input(
            DeviceInput::GridPress { x: 0, y: 0 },
            looper.transport.bpm as f32,
        )
        .unwrap();
    let recorded = looper.engine.serialized_state().unwrap()["steps"].clone();
    let saved_state = looper.engine.serialized_state().unwrap();
    looper.transport.layer_ticks[0] = 3;
    replace_with_current_behavior(&mut looper, Some(saved_state));
    assert_eq!(looper.engine.serialized_state().unwrap()["steps"], recorded);
    assert_eq!(looper.transport.layer_ticks[0], 3);
}

#[test]
fn non_sequencer_scanning_origins_keep_zero_accumulators() {
    for behavior_id in ["looper", "weave", "life"] {
        let mut runner = scanning_runner(behavior_id);
        runner.transport.layer_pulse_accumulators[0] = 0;
        runner.midi_enabled = true;
        runner.send(HostMessage::MidiRealtimeStart).unwrap();
        assert_eq!(
            runner.transport.layer_pulse_accumulators[0], 0,
            "{behavior_id}"
        );
    }
}

fn external_resync_runner() -> NativeRunner {
    let mut runner = scanning_runner("keys");
    runner.midi_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    runner.midi_clock_in_enabled = true;
    runner.transport.current_ppqn_pulse = 95;
    runner.transport.pending_resync = true;
    runner
}

fn musical_events(messages: &[RunnerMessage]) -> Vec<MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => Some(events.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn external_resync_batch_partitioning_preserves_current_semantics() {
    let mut batched = external_resync_runner();
    let mut partitioned = external_resync_runner();
    let batched_messages = batched
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    let mut partitioned_messages = partitioned
        .send(HostMessage::MidiRealtimeClock { pulses: 1 })
        .unwrap();
    partitioned_messages.extend(
        partitioned
            .send(HostMessage::MidiRealtimeClock { pulses: 1 })
            .unwrap(),
    );

    assert_eq!(
        musical_events(&batched_messages),
        musical_events(&partitioned_messages)
    );
    assert_eq!(
        batched.transport.current_ppqn_pulse,
        partitioned.transport.current_ppqn_pulse
    );
    assert_eq!(
        batched.transport.layer_ticks,
        partitioned.transport.layer_ticks
    );
    assert_eq!(
        batched.transport.layer_pulse_accumulators,
        partitioned.transport.layer_pulse_accumulators
    );
}
