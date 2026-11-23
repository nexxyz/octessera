use super::*;

fn held_keys_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.instruments[0].note_behavior = NoteBehavior::Hold;
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
    runner.transpose.offsets[0] = 7;

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
    assert_eq!(runner.held_keys.held_count(), 0);
}

#[test]
pub(crate) fn disabling_layer_drains_internal_sample_note_with_sample_routing() {
    let mut runner = held_keys_runner();
    runner.instruments[0].kind = InstrumentType::Sampler;
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 2,
        y: 3,
        sample_slot: 4,
        level: None,
        tune_semis: 0,
        tune_cents: 0,
    }];
    runner.sync_engine_runtime_config();

    let pressed = press_note(&mut runner, 2, 3);
    assert_eq!(
        routed_events(&pressed, false).iter().find_map(note_key),
        Some((0, 26))
    );

    let disabled = disable_layer(&mut runner, 0);
    let released_events = routed_events(&disabled, false);
    assert_eq!(
        released_events,
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 26,
        }]
    );
}

#[test]
pub(crate) fn disabling_layer_drains_external_midi_note_with_midi_routing() {
    let mut runner = held_keys_runner();
    runner.instruments[0].kind = InstrumentType::Midi;
    runner.instruments[0].midi_enabled = true;
    runner.instruments[0].midi_channel = 3;
    runner.test_set_midi_enabled(true);
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
    runner.instruments[1].kind = InstrumentType::Midi;
    runner.instruments[1].note_behavior = NoteBehavior::Hold;
    runner.instruments[1].midi_enabled = true;
    runner.instruments[1].midi_channel = 4;
    runner.layers[1].link.activate_slot = 1;
    runner.layers[1].link.note_preview = true;
    runner.test_set_midi_enabled(true);
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
    runner.layers[0].link.trigger_probability_mode = TriggerProbabilityMode::Custom;
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

    runner.apply_trigger_gate_mode_to_layer(0, TriggerProbabilityMode::Zero);
    let repeated_disable = runner.messages_with_snapshot().unwrap();
    assert!(routed_events(&repeated_disable, false).is_empty());

    let reenabled = disable_layer(&mut runner, 0);
    assert_eq!(
        runner.layers[0].link.trigger_probability_mode,
        TriggerProbabilityMode::Custom
    );
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
        instrument.note_behavior = NoteBehavior::Hold;
        match route {
            SharedRoute::Synth => {}
            SharedRoute::Sample => {
                instrument.kind = InstrumentType::Sampler;
                instrument.sample_assignments = vec![NativeSampleAssignment {
                    x: 2,
                    y: 3,
                    sample_slot: 4,
                    level: None,
                    tune_semis: 0,
                    tune_cents: 0,
                }];
            }
            SharedRoute::Midi => {
                instrument.kind = InstrumentType::Midi;
                instrument.midi_enabled = true;
                instrument.midi_channel = 3;
            }
        }
    }
    runner.layers[1].link = runner.layers[0].link.clone();
    runner.layers[1].link.activate_slot = 0;
    runner.layers[1].link.note_preview = true;
    if matches!(route, SharedRoute::Midi) {
        runner.test_set_midi_enabled(true);
    }
    runner.sync_engine_runtime_config();
}

/// Two layers pressing one key share its sound, so the first layer whose gate closes ends it for
/// both, on every route, and the second gate has nothing left to end.
#[test]
pub(crate) fn disabling_either_layer_releases_the_key_both_layers_share_once() {
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
        let offs = |messages: &[RunnerMessage]| {
            routed_events(messages, midi)
                .iter()
                .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
                .count()
        };
        assert_eq!(offs(&first_disable), 1, "{route:?}");

        runner.select_active_layer(1).unwrap();
        let final_disable = disable_layer(&mut runner, 1);
        assert_eq!(offs(&final_disable), 0, "{route:?}");
    }
}

#[test]
pub(crate) fn disabling_layer_cancels_delayed_note_without_note_off() {
    let mut runner = held_keys_runner();
    runner.layers[0].link.activate_timing.delay_steps = 1;

    let press = press_note(&mut runner, 2, 3);
    assert!(routed_events(&press, false).is_empty());

    let disabled = disable_layer(&mut runner, 0);
    assert!(routed_events(&disabled, false).is_empty());
    assert!(runner.delayed_link_events[0].is_empty());
}

fn note_offs(messages: &[RunnerMessage]) -> usize {
    routed_events(messages, false)
        .iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
        .count()
}

#[test]
pub(crate) fn trigger_gate_page_zero_releases_held_notes_and_blocks_future_admission() {
    let mut runner = held_keys_runner();
    let press = press_note(&mut runner, 2, 3);
    assert!(!routed_events(&press, false).is_empty());

    runner.active_play_mode = ActivePlayMode::Page(PlayMode::TriggerGate);
    let edit = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(note_offs(&edit), 1);

    runner.active_play_mode = ActivePlayMode::None;
    let blocked = press_note(&mut runner, 3, 3);
    assert!(routed_events(&blocked, false).is_empty());
}

#[test]
pub(crate) fn releasing_a_key_after_the_layer_toggle_sends_no_second_note_off() {
    let mut runner = held_keys_runner();
    press_note(&mut runner, 2, 3);
    assert_eq!(note_offs(&disable_layer(&mut runner, 0)), 1);
    runner.display.ui.combined_modifier_held = false;

    let released = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(note_offs(&released), 0);
}

#[test]
pub(crate) fn trigger_prob_mode_menu_zero_releases_held_notes() {
    let mut runner = held_keys_runner();
    press_note(&mut runner, 2, 3);

    assert!(runner
        .menu
        .focus_item_key("layers.0.link.triggerProbabilityMode"));
    runner.menu.state.editing = true;
    let edit = device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": -2, "id": "main" }),
    );
    assert_eq!(
        runner.layers[0].link.trigger_probability_mode,
        TriggerProbabilityMode::Zero
    );
    assert_eq!(note_offs(&edit), 1);
}

#[test]
pub(crate) fn disabling_layer_clears_only_target_arp_state_and_preserves_random_phase() {
    let mut runner = held_keys_runner();
    runner.link_arp.random_state = 123;
    runner.link_arp.keys[0].push(arp_key(0, 60));
    runner.link_arp.keys[1].push(arp_key(0, 64));
    runner.link_arp.rotating_phase[0] = 3;
    runner.link_arp.rotating_phase[1] = 7;
    runner.delayed_link_events[0].push(DelayedRoutedEvents {
        remaining_steps: 2,
        events: RoutedMusicalEvents::default(),
    });
    runner.delayed_link_events[1].push(DelayedRoutedEvents {
        remaining_steps: 2,
        events: RoutedMusicalEvents::default(),
    });

    let _ = disable_layer(&mut runner, 0);

    assert_eq!(runner.link_arp.random_state, 123);
    assert!(runner.link_arp.keys[0].is_empty());
    assert_eq!(runner.link_arp.rotating_phase[0], 0);
    assert!(runner.delayed_link_events[0].is_empty());
    assert_eq!(runner.link_arp.keys[1].len(), 1);
    assert_eq!(runner.link_arp.rotating_phase[1], 7);
    assert_eq!(runner.delayed_link_events[1].len(), 1);
}
