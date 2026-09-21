use super::*;

fn scanning_runner(scan_unit: &str, scanned_slot: usize) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    configure_active_scan(&mut runner, scan_unit, scanned_slot);
    runner
}

fn configure_active_scan(runner: &mut NativeRunner, scan_unit: &str, scanned_slot: usize) {
    let layer = &mut runner.link_layers[runner.active_layer_index];
    layer.scan_mode = "scanning".into();
    layer.scan_axis = "rows".into();
    layer.scan_unit = scan_unit.into();
    layer.scan_sections = 1;
    layer.scanned_slot = scanned_slot;
    layer.scanned_action = "note_on".into();
    layer.scanned_empty_action = "none".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
}

fn add_cell(runner: &mut NativeRunner, x: usize, y: usize) {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
            request_snapshot: None,
        })
        .unwrap();
}

fn arm_boundary(runner: &mut NativeRunner, current_ppqn_pulse: u64) {
    runner.midi_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    runner.midi_clock_in_enabled = true;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = current_ppqn_pulse;
    runner.transport.pending_resync = true;
}

fn clock(runner: &mut NativeRunner, pulses: u32) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::MidiRealtimeClock { pulses })
        .unwrap()
}

fn lane_events(messages: &[RunnerMessage], midi: bool) -> Vec<platform_core::MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match (midi, message) {
            (false, RunnerMessage::MusicalEvents { events })
            | (true, RunnerMessage::MidiEvents { events }) => Some(events.as_slice()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}

fn routed_events(messages: &[RunnerMessage]) -> Vec<(bool, platform_core::MusicalEvent)> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => events
                .iter()
                .cloned()
                .map(|event| (false, event))
                .collect::<Vec<_>>(),
            RunnerMessage::MidiEvents { events } => events
                .iter()
                .cloned()
                .map(|event| (true, event))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn state_signature(runner: &NativeRunner) -> (u64, Vec<u64>, Vec<u32>, bool) {
    (
        runner.transport.current_ppqn_pulse,
        runner.transport.layer_ticks.clone(),
        runner.transport.layer_pulse_accumulators.clone(),
        runner.transport.pending_resync,
    )
}

fn prepared_boundary_runner(current_ppqn_pulse: u64) -> NativeRunner {
    let mut runner = scanning_runner("1/16", 0);
    add_cell(&mut runner, 0, 0);
    arm_boundary(&mut runner, current_ppqn_pulse);
    runner.prime_sequencer_layer_origins();
    runner
}

#[test]
fn external_resync_batching_matches_single_clocks_at_and_across_boundary() {
    let mut batched = prepared_boundary_runner(95);
    let batched_messages = clock(&mut batched, 2);
    let mut partitioned = prepared_boundary_runner(95);
    let mut partitioned_messages = clock(&mut partitioned, 1);
    partitioned_messages.extend(clock(&mut partitioned, 1));
    assert_eq!(
        routed_events(&batched_messages),
        routed_events(&partitioned_messages)
    );
    assert_eq!(state_signature(&batched), state_signature(&partitioned));

    let mut crossed_batched = prepared_boundary_runner(94);
    let crossed_batched_messages = clock(&mut crossed_batched, 3);
    let mut crossed_partitioned = prepared_boundary_runner(94);
    let mut crossed_partitioned_messages = clock(&mut crossed_partitioned, 1);
    crossed_partitioned_messages.extend(clock(&mut crossed_partitioned, 1));
    crossed_partitioned_messages.extend(clock(&mut crossed_partitioned, 1));
    assert_eq!(
        routed_events(&crossed_batched_messages),
        routed_events(&crossed_partitioned_messages)
    );
    assert_eq!(
        state_signature(&crossed_batched),
        state_signature(&crossed_partitioned)
    );
}

fn dual_scanning_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.select_layer_behavior(1, "sequencer").unwrap();
    runner.instruments[0].note_behavior = "hold".into();
    runner.instruments[1].kind = "midi".into();
    runner.instruments[1].midi_enabled = true;
    runner.instruments[1].midi_channel = 1;
    runner.instruments[1].note_behavior = "hold".into();
    runner.select_active_layer(1).unwrap();
    configure_active_scan(&mut runner, "1/8", 1);
    add_cell(&mut runner, 0, 0);
    runner.select_active_layer(0).unwrap();
    configure_active_scan(&mut runner, "1/16", 0);
    add_cell(&mut runner, 0, 0);
    runner.sync_engine_runtime_config();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

#[test]
fn active_and_inactive_sequencers_emit_origin_ticks_with_route_ownership_after_cleanup() {
    let mut runner = dual_scanning_runner();
    runner.prime_sequencer_layer_origins();
    let old_events = runner.advance_due_layer_ticks().unwrap();
    assert_eq!(
        old_events
            .audio
            .iter()
            .filter(|event| matches!(event, platform_core::MusicalEvent::NoteOn { .. }))
            .count(),
        1
    );
    assert_eq!(
        old_events
            .midi
            .iter()
            .filter(|event| matches!(event, platform_core::MusicalEvent::NoteOn { .. }))
            .count(),
        1
    );

    arm_boundary(&mut runner, 95);
    let messages = clock(&mut runner, 1);
    let audio = lane_events(&messages, false);
    let midi = lane_events(&messages, true);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert_eq!(runner.transport.layer_ticks[0], 1);
    assert_eq!(runner.transport.layer_ticks[1], 1);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
    assert_eq!(runner.transport.layer_pulse_accumulators[1], 0);
    assert_eq!(audio.len(), 2);
    assert_eq!(midi.len(), 2);
    assert!(matches!(
        audio[0],
        platform_core::MusicalEvent::NoteOff { .. }
    ));
    assert!(matches!(
        midi[0],
        platform_core::MusicalEvent::NoteOff { .. }
    ));
    let (audio_channel, audio_note) = match audio[1] {
        platform_core::MusicalEvent::NoteOn {
            channel,
            note,
            duration_ms: None,
            ..
        } => (channel, note),
        _ => panic!("expected internal origin NoteOn"),
    };
    let (midi_channel, midi_note) = match midi[1] {
        platform_core::MusicalEvent::NoteOn {
            channel,
            note,
            duration_ms: None,
            ..
        } => (channel, note),
        _ => panic!("expected MIDI origin NoteOn"),
    };
    assert_eq!(audio_note, midi_note);
    assert!(matches!(
        audio[0],
        platform_core::MusicalEvent::NoteOff { channel, note }
            if channel == audio_channel && note == audio_note
    ));
    assert!(matches!(
        midi[0],
        platform_core::MusicalEvent::NoteOff { channel, note }
            if channel == midi_channel && note == midi_note
    ));
    assert!(runner.emitted_route_note_owners[0].contains(&(true, audio_channel, audio_note)));
    assert!(runner.emitted_route_note_owners[1].contains(&(false, midi_channel, midi_note)));
}

#[test]
fn muted_sequencer_consumes_origin_tick_without_events() {
    let mut runner = scanning_runner("1/16", 0);
    add_cell(&mut runner, 0, 0);
    runner.link_layers[0].trigger_probability_mode = "zero".into();
    arm_boundary(&mut runner, 95);
    runner.prime_sequencer_layer_origins();

    let messages = clock(&mut runner, 1);

    assert!(routed_events(&messages).is_empty());
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert_eq!(runner.transport.layer_ticks[0], 1);
    assert_eq!(runner.transport.tick, 1);
    assert_eq!(runner.transport.layer_pulse_accumulators[0], 0);
}

#[test]
fn external_resync_clears_old_link_and_arp_state_but_keeps_new_origin_delay() {
    let mut runner = scanning_runner("1/16", 0);
    runner.instruments[0].note_behavior = "hold".into();
    runner.sync_engine_runtime_config();
    runner.link_layers[0].scanned_timing.delay_steps = 2;
    runner.link_layers[0].arp.mode = "direct".into();
    runner.delayed_link_events[0].push(DelayedRoutedEvents {
        remaining_steps: 1,
        events: RoutedMusicalEvents {
            audio: vec![platform_core::MusicalEvent::NoteOn {
                channel: 0,
                note: 99,
                velocity: 100,
                duration_ms: None,
            }],
            midi: Vec::new(),
        },
    });
    runner.link_arp_held_notes[0].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 99,
        velocity: 100,
    });
    add_cell(&mut runner, 0, 0);
    arm_boundary(&mut runner, 95);
    runner.prime_sequencer_layer_origins();

    let messages = clock(&mut runner, 1);

    assert!(routed_events(&messages).is_empty());
    assert_eq!(runner.delayed_link_events[0].len(), 1);
    assert_eq!(runner.delayed_link_events[0][0].remaining_steps, 2);
    assert!(runner.delayed_link_events[0][0]
        .events
        .audio
        .iter()
        .all(|event| !matches!(event, platform_core::MusicalEvent::NoteOn { note: 99, .. })));
    assert!(runner.link_arp_held_notes[0]
        .iter()
        .all(|held| held.note != 99));
}

#[test]
fn non_sequencer_resync_does_not_zero_time_tick_looper_pattern_quantized_keys_or_life() {
    for (behavior_id, behavior_config) in [
        ("looper", json!({ "mode": "overdub", "lengthSteps": 2 })),
        ("weave", Value::Null),
        ("keys", json!({ "quantize": "step" })),
        ("life", Value::Null),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: behavior_id.into(),
            behavior_config,
            ..NativeRunnerConfig::default()
        })
        .unwrap();
        if behavior_id == "keys" {
            runner
                .engine
                .on_input(
                    DeviceInput::GridPress { x: 0, y: 0 },
                    runner.transport.bpm as f32,
                )
                .unwrap();
        }
        arm_boundary(&mut runner, 95);

        let messages = clock(&mut runner, 1);

        assert!(routed_events(&messages).is_empty(), "{behavior_id}");
        assert_eq!(runner.transport.layer_ticks[0], 0, "{behavior_id}");
        assert_eq!(
            runner.transport.layer_pulse_accumulators[0], 0,
            "{behavior_id}"
        );
        match runner.engine.state() {
            platform_core::NativeBehaviorState::Looper(state) => assert_eq!(state.step_index, 0),
            platform_core::NativeBehaviorState::Pattern(state) => assert_eq!(state.phase, 0),
            platform_core::NativeBehaviorState::Keys(state) => {
                assert!(state.cells.iter().all(|cell| !cell));
                assert!(state.held_cells[platform_core::grid_index(0, 0)]);
            }
            platform_core::NativeBehaviorState::Life(state) => assert_eq!(state.tick_counter, 0),
            state => panic!("unexpected state for {behavior_id}: {state:?}"),
        }
    }
}
