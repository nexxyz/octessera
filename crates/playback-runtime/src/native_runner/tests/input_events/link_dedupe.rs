use super::*;

fn audio_events(messages: &[RunnerMessage]) -> Vec<platform_core::MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => Some(events.as_slice()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
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

fn intent(kind: platform_core::CellTriggerKind) -> platform_core::CellTriggerIntent {
    platform_core::CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind,
    }
}

fn note_ons(events: &[platform_core::MusicalEvent]) -> Vec<(u8, u8, u8, Option<u32>)> {
    events
        .iter()
        .filter_map(|event| match event {
            platform_core::MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => Some((*channel, *note, *velocity, *duration_ms)),
            _ => None,
        })
        .collect()
}

#[test]
pub(crate) fn canonical_default_life_first_tick_keeps_expected_note_ons() {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../../../config/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let step_pulses = runner.transport.algorithm_step_pulses;
    let first = pulse(&mut runner, step_pulses);

    assert_eq!(
        musical_note_ons(&first)
            .into_iter()
            .filter(|(channel, _)| *channel == 0)
            .collect::<Vec<_>>(),
        vec![(0, 62), (0, 66), (0, 69), (0, 74)]
    );
}

#[test]
pub(crate) fn coalesced_24_pulses_preserve_cross_tick_retriggers() {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../../../config/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let messages = pulse(&mut runner, 24);
    let notes = musical_note_ons(&messages)
        .into_iter()
        .filter(|(channel, _)| *channel == 0)
        .collect::<Vec<_>>();

    assert_eq!(
        notes,
        vec![
            (0, 62),
            (0, 66),
            (0, 69),
            (0, 74),
            (0, 59),
            (0, 59),
            (0, 64),
            (0, 66),
            (0, 69),
            (0, 71),
            (0, 74),
            (0, 74),
            (0, 62),
            (0, 66),
            (0, 69),
            (0, 74),
        ]
    );
    assert!(notes.windows(2).any(|window| window[0] == window[1]));
}

#[test]
pub(crate) fn second_tick_preserves_note_boundaries_between_event_kinds() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let activate = intent(platform_core::CellTriggerKind::Activate);
    let deactivate = intent(platform_core::CellTriggerKind::Deactivate);
    let model = runner.engine.model().unwrap();
    let messages = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 70,
                    duration_ms: Some(150),
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 110,
                    duration_ms: Some(150),
                },
                platform_core::MusicalEvent::NoteOff {
                    channel: 0,
                    note: 60,
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 80,
                    duration_ms: Some(150),
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![activate.clone(), deactivate.clone(), activate.clone()],
            event_intents: vec![
                Some(activate.clone()),
                Some(activate),
                Some(deactivate),
                Some(intent(platform_core::CellTriggerKind::Activate)),
            ],
            model,
        })
        .unwrap();

    assert_eq!(
        audio_events(&messages),
        vec![
            platform_core::MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 110,
                duration_ms: Some(150),
            },
            platform_core::MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
            platform_core::MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 80,
                duration_ms: Some(150),
            },
        ]
    );
}

#[test]
pub(crate) fn same_note_from_two_layers_remains_two_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[1].activate_slot = 0;
    runner.pulses_layers[1].deactivate_slot = 0;
    runner.transport.layer_algorithm_step_pulses[0] = 12;
    runner.transport.layer_algorithm_step_pulses[1] = 12;
    runner.pulses_layers[1].event_enabled = true;

    let messages = pulse(&mut runner, 12);
    let notes = musical_note_ons(&messages);

    assert!(notes
        .iter()
        .any(|candidate| { notes.iter().filter(|other| *other == candidate).count() >= 2 }));
}

#[test]
pub(crate) fn same_external_midi_destination_from_two_mappings_remains_two_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for instrument in runner.instruments.iter_mut().take(2) {
        instrument.kind = "midi".into();
        instrument.midi_enabled = true;
        instrument.midi_channel = 4;
    }
    let activate = intent(platform_core::CellTriggerKind::Activate);
    let model = runner.engine.model().unwrap();
    let messages = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 80,
                    duration_ms: Some(120),
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 1,
                    note: 60,
                    velocity: 100,
                    duration_ms: Some(120),
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![activate.clone(), activate.clone()],
            event_intents: vec![Some(activate.clone()), Some(activate)],
            model,
        })
        .unwrap();
    let midi = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::MidiEvents { events } => Some(note_ons(events)),
            _ => None,
        })
        .unwrap();

    assert_eq!(midi, vec![(3, 60, 80, Some(120)), (3, 60, 100, Some(120))]);
}

#[test]
pub(crate) fn sampler_source_pitches_dedupe_after_assignment_routing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_assignments = vec![
        NativeSampleAssignment {
            x: 2,
            y: 3,
            sample_slot: 2,
            level: None,
        },
        NativeSampleAssignment {
            x: 4,
            y: 3,
            sample_slot: 2,
            level: None,
        },
    ];
    let first = intent(platform_core::CellTriggerKind::Activate);
    let second = platform_core::CellTriggerIntent {
        x: 4,
        y: 3,
        ..first.clone()
    };
    let model = runner.engine.model().unwrap();
    let messages = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 70,
                    duration_ms: Some(120),
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 64,
                    velocity: 110,
                    duration_ms: Some(120),
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![first.clone(), second.clone()],
            event_intents: vec![Some(first), Some(second)],
            model,
        })
        .unwrap();

    assert_eq!(
        note_ons(&audio_events(&messages)),
        vec![(0, 38, 86, Some(120))]
    );
}

#[test]
pub(crate) fn due_retrigger_and_fresh_identical_note_remain_two_events() {
    let mut probe = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    probe.transport.transport = RuntimeTransportState::Playing;
    let first_tick = probe.advance_algorithm(12).unwrap();
    let expected = first_tick
        .audio
        .into_iter()
        .find(|event| matches!(event, platform_core::MusicalEvent::NoteOn { .. }))
        .expect("first life tick note");

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;
    let activate = intent(platform_core::CellTriggerKind::Activate);
    let immediate = runner.apply_link_timing(
        0,
        std::slice::from_ref(&activate),
        RoutedMusicalEvents {
            audio: vec![expected.clone()],
            midi: vec![],
        },
    );
    assert_eq!(immediate.audio, vec![expected.clone()]);

    let events = runner.advance_algorithm(12).unwrap();
    assert_eq!(
        events
            .audio
            .iter()
            .filter(|event| *event == &expected)
            .count(),
        2
    );
}

#[test]
pub(crate) fn probability_filtering_happens_before_local_dedupe() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    let mut cells = vec![false; 64];
    for (x, y) in [(4, 4), (4, 5), (4, 6)] {
        cells[y * 8 + x] = true;
    }
    payload["runtimeConfig"]["layers"][0]["worlds"]["savedState"]["cells"] = json!(cells);
    runner.apply_config_payload(payload).unwrap();
    runner.pulses_layers[0].trigger_probability_mode = "custom".into();
    runner.trigger_probability_maps[0].fill("full".into());
    runner.trigger_probability_maps[0][5 * 8 + 3] = "zero".into();
    runner.pulses_layers[0].x_velocity = NativeValueLane {
        enabled: true,
        from: 10,
        to: 110,
        grid_offset: 0,
        curve: "linear".into(),
    };
    runner.transport.transport = RuntimeTransportState::Playing;

    let step_pulses = runner.transport.algorithm_step_pulses;
    let messages = pulse(&mut runner, step_pulses);

    assert_eq!(
        note_ons(&audio_events(&messages)),
        vec![(0, 71, 81, Some(150))]
    );
}

#[test]
pub(crate) fn local_dedupe_keeps_duration_none_and_non_note_order() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let activate = intent(platform_core::CellTriggerKind::Activate);
    let model = runner.engine.model().unwrap();
    let messages = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 50,
                    duration_ms: None,
                },
                platform_core::MusicalEvent::Cc {
                    channel: 0,
                    controller: 74,
                    value: 40,
                },
                platform_core::MusicalEvent::NoteOff {
                    channel: 0,
                    note: 60,
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100,
                    duration_ms: None,
                },
                platform_core::MusicalEvent::Cc {
                    channel: 0,
                    controller: 74,
                    value: 50,
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![activate.clone()],
            event_intents: vec![
                Some(activate.clone()),
                Some(activate.clone()),
                Some(activate.clone()),
                Some(activate.clone()),
                Some(activate),
            ],
            model,
        })
        .unwrap();

    assert_eq!(
        audio_events(&messages),
        vec![
            platform_core::MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: None,
            },
            platform_core::MusicalEvent::Cc {
                channel: 0,
                controller: 74,
                value: 40,
            },
            platform_core::MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
            platform_core::MusicalEvent::Cc {
                channel: 0,
                controller: 74,
                value: 50,
            },
        ]
    );
}

#[test]
pub(crate) fn local_dedupe_keeps_same_note_with_different_durations() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let activate = intent(platform_core::CellTriggerKind::Activate);
    let model = runner.engine.model().unwrap();
    let messages = runner
        .messages_with_input_result(platform_core::NativeInputResult {
            events: vec![
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 50,
                    duration_ms: Some(80),
                },
                platform_core::MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100,
                    duration_ms: Some(120),
                },
            ],
            emitted_events: vec![],
            mapped_intents: vec![activate.clone(), activate.clone()],
            event_intents: vec![Some(activate.clone()), Some(activate)],
            model,
        })
        .unwrap();

    assert_eq!(
        note_ons(&audio_events(&messages)),
        vec![(0, 60, 50, Some(80)), (0, 60, 100, Some(120))]
    );
}
