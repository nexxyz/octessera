use super::*;

fn held_keys_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.instruments[0].note_behavior = "hold".into();
    runner.sync_engine_runtime_config();
    runner
}

fn press_note(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
            request_snapshot: None,
        })
        .unwrap()
}

fn disable_layer(runner: &mut NativeRunner, layer: usize) -> Vec<RunnerMessage> {
    runner.display.ui.combined_modifier_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": layer }),
            request_snapshot: None,
        })
        .unwrap()
}

fn routed_events(messages: &[RunnerMessage], midi: bool) -> Vec<MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match (midi, message) {
            (false, RunnerMessage::MusicalEvents { events })
            | (true, RunnerMessage::MidiEvents { events }) => Some(events),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}

fn note_key(event: &MusicalEvent) -> Option<(u8, u8)> {
    match event {
        MusicalEvent::NoteOn { channel, note, .. } | MusicalEvent::NoteOff { channel, note } => {
            Some((*channel, *note))
        }
        MusicalEvent::Cc { .. } => None,
    }
}

#[test]
pub(crate) fn disabling_layer_drains_internal_synth_note_with_transpose_routing() {
    let mut runner = held_keys_runner();
    runner.sparks_transpose_offsets[0] = 7;

    let pressed = press_note(&mut runner, 2, 3);
    let pressed_events = routed_events(&pressed, false);
    let note_on = pressed_events
        .iter()
        .find(|event| matches!(event, MusicalEvent::NoteOn { .. }))
        .and_then(note_key)
        .expect("synth note on");

    let disabled = disable_layer(&mut runner, 0);
    let released_events = routed_events(&disabled, false);
    assert_eq!(
        released_events
            .iter()
            .filter_map(note_key)
            .collect::<Vec<_>>(),
        vec![(note_on.0, note_on.1)]
    );
    assert!(released_events
        .iter()
        .all(|event| matches!(event, MusicalEvent::NoteOff { .. })));
    assert!(runner.sparks_transpose_active_notes[0].is_empty());
}

#[test]
pub(crate) fn disabling_layer_drains_internal_sample_note_with_sample_routing() {
    let mut runner = held_keys_runner();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 2,
        y: 3,
        sample_slot: 4,
        level: None,
    }];
    runner.sync_engine_runtime_config();

    let pressed = press_note(&mut runner, 2, 3);
    assert_eq!(
        routed_events(&pressed, false).iter().find_map(note_key),
        Some((0, 40))
    );

    let disabled = disable_layer(&mut runner, 0);
    let released_events = routed_events(&disabled, false);
    assert_eq!(
        released_events,
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 40,
        }]
    );
}

#[test]
pub(crate) fn disabling_layer_drains_external_midi_note_with_midi_routing() {
    let mut runner = held_keys_runner();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].midi_enabled = true;
    runner.instruments[0].midi_channel = 3;
    runner.midi_enabled = true;
    runner.sync_engine_runtime_config();

    let pressed = press_note(&mut runner, 2, 3);
    let note_on = routed_events(&pressed, true)
        .iter()
        .find(|event| matches!(event, MusicalEvent::NoteOn { .. }))
        .and_then(note_key)
        .expect("MIDI note on");

    let disabled = disable_layer(&mut runner, 0);
    assert_eq!(
        routed_events(&disabled, true),
        vec![MusicalEvent::NoteOff {
            channel: note_on.0,
            note: note_on.1,
        }]
    );
}

#[test]
pub(crate) fn disabling_one_layer_leaves_another_layer_held() {
    let mut runner = held_keys_runner();
    runner.instruments[1] = NativeInstrumentSlot::new(1);
    runner.instruments[1].kind = "midi".into();
    runner.instruments[1].note_behavior = "hold".into();
    runner.instruments[1].midi_enabled = true;
    runner.instruments[1].midi_channel = 4;
    runner.pulses_layers[1].activate_slot = 1;
    runner.pulses_layers[1].event_enabled = true;
    runner.midi_enabled = true;
    runner.sync_engine_runtime_config();

    let layer_zero_press = press_note(&mut runner, 2, 3);
    assert!(!routed_events(&layer_zero_press, false).is_empty());
    runner.select_active_layer(1).unwrap();
    let layer_one_press = press_note(&mut runner, 2, 3);
    assert!(!routed_events(&layer_one_press, true).is_empty());
    runner.select_active_layer(0).unwrap();

    let layer_zero_disabled = disable_layer(&mut runner, 0);
    assert!(!routed_events(&layer_zero_disabled, false).is_empty());
    assert!(routed_events(&layer_zero_disabled, true).is_empty());

    runner.select_active_layer(1).unwrap();
    let layer_one_disabled = disable_layer(&mut runner, 1);
    assert_eq!(
        routed_events(&layer_one_disabled, true)
            .iter()
            .filter_map(note_key)
            .count(),
        1
    );
}

#[test]
pub(crate) fn repeated_layer_disable_is_idempotent_and_reenable_restores_gate_only() {
    let mut runner = held_keys_runner();
    runner.trigger_gate_modes[0] = "custom".into();
    runner.pulses_layers[0].trigger_probability_mode = "custom".into();
    runner.transport.tick = 7;
    runner.transport.current_ppqn_pulse = 13;
    let _ = press_note(&mut runner, 2, 3);

    let first_disable = disable_layer(&mut runner, 0);
    assert_eq!(
        routed_events(&first_disable, false)
            .iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
            .count(),
        1
    );
    assert_eq!(runner.transport.tick, 7);
    assert_eq!(runner.transport.current_ppqn_pulse, 13);

    runner.apply_trigger_gate_mode_to_layer(0, "zero");
    let repeated_disable = runner.messages_with_snapshot().unwrap();
    assert!(routed_events(&repeated_disable, false).is_empty());

    let reenabled = disable_layer(&mut runner, 0);
    assert_eq!(runner.trigger_gate_modes[0], "custom");
    assert_eq!(runner.pulses_layers[0].trigger_probability_mode, "custom");
    assert!(routed_events(&reenabled, false).is_empty());

    let new_note = press_note(&mut runner, 3, 3);
    assert!(routed_events(&new_note, false)
        .iter()
        .any(|event| matches!(event, MusicalEvent::NoteOn { .. })));
}

#[derive(Clone, Copy, Debug)]
enum SharedRoute {
    Synth,
    Sample,
    Midi,
}

fn configure_shared_route(runner: &mut NativeRunner, route: SharedRoute) {
    for instrument in runner.instruments.iter_mut().take(2) {
        instrument.note_behavior = "hold".into();
        match route {
            SharedRoute::Synth => {}
            SharedRoute::Sample => {
                instrument.kind = "sampler".into();
                instrument.sample_assignments = vec![NativeSampleAssignment {
                    x: 2,
                    y: 3,
                    sample_slot: 4,
                    level: None,
                }];
            }
            SharedRoute::Midi => {
                instrument.kind = "midi".into();
                instrument.midi_enabled = true;
                instrument.midi_channel = 3;
            }
        }
    }
    runner.pulses_layers[1] = runner.pulses_layers[0].clone();
    runner.pulses_layers[1].activate_slot = 0;
    runner.pulses_layers[1].event_enabled = true;
    if matches!(route, SharedRoute::Midi) {
        runner.midi_enabled = true;
    }
    runner.sync_engine_runtime_config();
}

#[test]
pub(crate) fn disabling_layer_releases_only_the_final_same_route_owner() {
    for route in [SharedRoute::Synth, SharedRoute::Sample, SharedRoute::Midi] {
        let mut runner = held_keys_runner();
        configure_shared_route(&mut runner, route);

        let first_press = press_note(&mut runner, 2, 3);
        let midi = matches!(route, SharedRoute::Midi);
        let first_press_events = routed_events(&first_press, midi);
        assert!(!first_press_events.is_empty());
        runner.select_active_layer(1).unwrap();
        let second_press = press_note(&mut runner, 2, 3);
        let second_press_events = routed_events(&second_press, midi);
        assert!(!second_press_events.is_empty());
        assert_eq!(
            first_press_events.iter().find_map(note_key),
            second_press_events.iter().find_map(note_key),
            "{route:?}: first={first_press_events:?} second={second_press_events:?}"
        );

        runner.select_active_layer(0).unwrap();
        let first_disable = disable_layer(&mut runner, 0);
        let first_disable_events = routed_events(&first_disable, midi);
        assert!(
            first_disable_events
                .iter()
                .all(|event| !matches!(event, MusicalEvent::NoteOff { .. })),
            "{route:?}: {first_disable_events:?}"
        );

        runner.select_active_layer(1).unwrap();
        let final_disable = disable_layer(&mut runner, 1);
        assert_eq!(
            routed_events(&final_disable, matches!(route, SharedRoute::Midi))
                .iter()
                .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
                .count(),
            1
        );
    }
}

#[test]
pub(crate) fn disabling_layer_cancels_delayed_note_without_note_off() {
    let mut runner = held_keys_runner();
    runner.pulses_layers[0].activate_timing.delay_steps = 1;

    let press = press_note(&mut runner, 2, 3);
    assert!(routed_events(&press, false).is_empty());

    let disabled = disable_layer(&mut runner, 0);
    assert!(routed_events(&disabled, false).is_empty());
    assert!(runner.delayed_link_events[0].is_empty());
}

#[test]
pub(crate) fn transpose_drain_then_layer_disable_does_not_repeat_note_off() {
    let mut runner = held_keys_runner();
    let press = press_note(&mut runner, 2, 3);
    assert_eq!(
        routed_events(&press, false)
            .iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOn { .. }))
            .count(),
        1
    );

    runner.active_sparks_mode = "transpose".into();
    let transpose = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 5 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        routed_events(&transpose, false)
            .iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
            .count(),
        1
    );

    runner.active_sparks_mode = "none".into();
    let disabled = disable_layer(&mut runner, 0);
    assert!(routed_events(&disabled, false)
        .iter()
        .all(|event| !matches!(event, MusicalEvent::NoteOff { .. })));
}

#[test]
pub(crate) fn transpose_drain_preserves_another_layer_owner_for_final_disable() {
    let mut runner = held_keys_runner();
    configure_shared_route(&mut runner, SharedRoute::Synth);
    let _ = press_note(&mut runner, 2, 3);
    runner.select_active_layer(1).unwrap();
    let _ = press_note(&mut runner, 2, 3);
    runner.select_active_layer(0).unwrap();
    runner.sparks_transpose_selected[1] = false;

    runner.active_sparks_mode = "transpose".into();
    let transpose = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 5 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        routed_events(&transpose, false)
            .iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
            .count(),
        1
    );

    runner.active_sparks_mode = "none".into();
    let first_disabled = disable_layer(&mut runner, 0);
    assert!(routed_events(&first_disabled, false)
        .iter()
        .all(|event| !matches!(event, MusicalEvent::NoteOff { .. })));

    runner.select_active_layer(1).unwrap();
    let final_disabled = disable_layer(&mut runner, 1);
    assert_eq!(
        routed_events(&final_disabled, false)
            .iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
            .count(),
        1
    );
}

#[test]
pub(crate) fn trigger_gate_page_zero_only_blocks_future_admission() {
    let mut runner = held_keys_runner();
    let press = press_note(&mut runner, 2, 3);
    assert!(!routed_events(&press, false).is_empty());

    runner.active_sparks_mode = "trigger-gate".into();
    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(routed_events(&edit, false)
        .iter()
        .all(|event| !matches!(event, MusicalEvent::NoteOff { .. })));

    runner.active_sparks_mode = "none".into();
    let blocked = press_note(&mut runner, 3, 3);
    assert!(routed_events(&blocked, false).is_empty());
}

#[test]
pub(crate) fn disabling_layer_clears_only_target_arp_state_and_preserves_random_phase() {
    let mut runner = held_keys_runner();
    runner.link_arp_random_state = 123;
    runner.link_arp_held_notes[0].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 60,
        velocity: 100,
    });
    runner.link_arp_held_notes[1].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 64,
        velocity: 100,
    });
    runner.link_arp_rotating_phase[0] = 3;
    runner.link_arp_rotating_phase[1] = 7;
    runner.delayed_link_events[0].push(DelayedRoutedEvents {
        remaining_steps: 2,
        events: RoutedMusicalEvents::default(),
    });
    runner.delayed_link_events[1].push(DelayedRoutedEvents {
        remaining_steps: 2,
        events: RoutedMusicalEvents::default(),
    });

    let _ = disable_layer(&mut runner, 0);

    assert_eq!(runner.link_arp_random_state, 123);
    assert!(runner.link_arp_held_notes[0].is_empty());
    assert_eq!(runner.link_arp_rotating_phase[0], 0);
    assert!(runner.delayed_link_events[0].is_empty());
    assert_eq!(runner.link_arp_held_notes[1].len(), 1);
    assert_eq!(runner.link_arp_rotating_phase[1], 7);
    assert_eq!(runner.delayed_link_events[1].len(), 1);
}
