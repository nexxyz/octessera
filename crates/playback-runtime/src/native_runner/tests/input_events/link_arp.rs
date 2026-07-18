use super::*;

#[test]
pub(crate) fn link_arp_payload_round_trips_and_clamps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "layers": [{ "pulses": { "arp": {
                "mode": "strum", "source": "held", "stepIntervalSteps": 99,
                "noteLengthMs": 9999, "gatePct": 0, "octaveSpread": 9
            } } }] }
        }))
        .unwrap();

    let arp = &runner.pulses_layers[0].arp;
    assert_eq!(arp.mode, "strum");
    assert_eq!(arp.source, "held");
    assert_eq!(arp.step_interval_steps, 16);
    assert_eq!(arp.note_length_ms, 2000);
    assert_eq!(arp.gate_pct, 1);
    assert_eq!(arp.octave_spread, 3);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["pulses"]["arp"]["source"],
        "held"
    );

    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "layers": [{ "pulses": { "arp": {
                "stepIntervalSteps": -9, "noteLengthMs": -9,
                "gatePct": -9, "octaveSpread": -9
            } } }] }
        }))
        .unwrap();
    let arp = &runner.pulses_layers[0].arp;
    assert_eq!(arp.step_interval_steps, 1);
    assert_eq!(arp.note_length_ms, 10);
    assert_eq!(arp.gate_pct, 1);
    assert_eq!(arp.octave_spread, 0);
}

#[test]
pub(crate) fn link_arp_menu_apply_paths_round_trip_held_source() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("layers.0.pulses.arp.source", 1);
    runner.menu.turn_key("layers.0.pulses.arp.mode", 2);
    assert!(runner.apply_menu_key_fast("layers.0.pulses.arp.source"));
    assert!(runner.apply_menu_key_fast("layers.0.pulses.arp.mode"));
    assert_eq!(runner.pulses_layers[0].arp.source, "held");
    assert_eq!(runner.pulses_layers[0].arp.mode, "up");

    runner.link_arp_held_notes[0].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 60,
        velocity: 96,
    });
    runner.menu.turn_key("layers.0.pulses.arp.mode", -2);
    assert!(runner.apply_menu_key_fast("layers.0.pulses.arp.mode"));
    assert!(runner.link_arp_held_notes[0].is_empty());
    runner.menu.turn_key("layers.0.pulses.arp.mode", 2);

    runner.pulses_layers[0].arp = NativeLinkArp::default();
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.pulses_layers[0].arp.source, "held");
    assert_eq!(runner.pulses_layers[0].arp.mode, "up");
}

#[test]
pub(crate) fn link_arp_orders_simultaneous_batches_with_finite_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "up".into();
    runner.pulses_layers[0].arp.step_interval_steps = 1;
    let intent = platform_core::CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let routed = RoutedMusicalEvents {
        audio: vec![note_on(64), note_on(60), note_on(67)],
        midi: vec![],
    };

    let immediate = runner.apply_link_timing(0, &[intent], routed);
    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
    assert!(matches!(
        immediate.audio[0],
        platform_core::MusicalEvent::NoteOn {
            duration_ms: Some(96),
            ..
        }
    ));

    let first = runner.take_due_link_events(0);
    let second = runner.take_due_link_events(0);
    assert_eq!(musical_note_ons_from_events(&first.audio), vec![(0, 64)]);
    assert_eq!(musical_note_ons_from_events(&second.audio), vec![(0, 67)]);
}

#[test]
pub(crate) fn link_arp_passes_non_note_events_through() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "up".into();
    let immediate = runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![cc(74, 90), note_on(60)],
            midi: vec![],
        },
    );

    assert!(immediate.audio.iter().any(|event| matches!(
        event,
        platform_core::MusicalEvent::Cc {
            controller: 74,
            value: 90,
            ..
        }
    )));
    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
}

#[test]
pub(crate) fn link_arp_held_source_merges_non_note_events() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "direct".into();
    runner.pulses_layers[0].arp.source = "held".into();
    runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![note_on(60)],
            midi: vec![],
        },
    );

    let immediate = runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![cc(74, 90)],
            midi: vec![],
        },
    );

    assert!(immediate.audio.iter().any(|event| matches!(
        event,
        platform_core::MusicalEvent::Cc {
            controller: 74,
            value: 90,
            ..
        }
    )));
    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
}

#[test]
pub(crate) fn link_arp_octave_spread_expands_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "octave_spread".into();
    runner.pulses_layers[0].arp.octave_spread = 1;
    runner.pulses_layers[0].arp.step_interval_steps = 1;
    let immediate = runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![note_on(60)],
            midi: vec![],
        },
    );

    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
    let delayed = runner.take_due_link_events(0);
    assert_eq!(musical_note_ons_from_events(&delayed.audio), vec![(0, 72)]);
}

#[test]
pub(crate) fn link_arp_default_none_matches_existing_link_timing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].activate_timing.delay_steps = 1;
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;
    let intent = platform_core::CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let routed = RoutedMusicalEvents {
        audio: vec![finite_input_note_on(60)],
        midi: vec![],
    };

    let immediate = runner.apply_link_timing(0, &[intent], routed);
    assert!(immediate.audio.is_empty());
    let delayed = runner.take_due_link_events(0);
    assert_eq!(delayed.audio, vec![finite_input_note_on(60)]);
    let retrigger = runner.take_due_link_events(0);
    assert_eq!(retrigger.audio, vec![finite_input_note_on(60)]);
}

#[test]
pub(crate) fn link_arp_direct_preserves_input_order_and_large_batch_offsets_do_not_collapse() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "direct".into();
    let routed = RoutedMusicalEvents {
        audio: vec![note_on(67), note_on(60), note_on(64)],
        midi: vec![],
    };
    let immediate = runner.apply_link_timing(0, &[], routed);
    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 67), (0, 60), (0, 64)]
    );

    runner.pulses_layers[0].arp.mode = "up".into();
    runner.pulses_layers[0].arp.step_interval_steps = 16;
    let routed = RoutedMusicalEvents {
        audio: (0..20).map(|index| note_on(40 + index)).collect(),
        midi: vec![],
    };
    runner.apply_link_timing(0, &[], routed);
    assert!(runner.delayed_link_events[0]
        .iter()
        .any(|entry| entry.remaining_steps > u16::from(u8::MAX)));
}

#[test]
pub(crate) fn link_arp_held_source_updates_releases_and_suppresses_note_offs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "up".into();
    runner.pulses_layers[0].arp.source = "held".into();
    runner.pulses_layers[0].arp.step_interval_steps = 1;

    let first = runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![note_on(64)],
            midi: vec![],
        },
    );
    assert_eq!(musical_note_ons_from_events(&first.audio), vec![(0, 64)]);
    let second = runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![note_on(60)],
            midi: vec![],
        },
    );
    assert_eq!(musical_note_ons_from_events(&second.audio), vec![(0, 60)]);
    let tail = runner.take_due_link_events(0);
    assert_eq!(musical_note_ons_from_events(&tail.audio), vec![(0, 64)]);

    runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![note_off(64)],
            midi: vec![],
        },
    );
    assert!(runner.delayed_link_events[0]
        .iter()
        .all(|entry| !has_note_on(&entry.events.audio, 64)));
}

#[test]
pub(crate) fn link_arp_held_source_ignores_finite_notes_and_resets_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "up".into();
    runner.pulses_layers[0].arp.source = "held".into();

    let finite = runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![finite_input_note_on(60)],
            midi: vec![],
        },
    );
    assert!(finite.audio.is_empty());
    assert!(runner.link_arp_held_notes[0].is_empty());

    runner.apply_link_timing(
        0,
        &[],
        RoutedMusicalEvents {
            audio: vec![note_on(64)],
            midi: vec![],
        },
    );
    assert!(!runner.link_arp_held_notes[0].is_empty());
    runner.link_arp_random_state = 123;
    runner.reset_transport_position();
    assert!(runner.link_arp_held_notes[0].is_empty());
    assert_eq!(runner.link_arp_random_state, 0x4f43_5441);
}

#[test]
pub(crate) fn link_arp_held_source_retriggers_finite_arp_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "direct".into();
    runner.pulses_layers[0].arp.source = "held".into();
    runner.pulses_layers[0].activate_timing.retrigger_count = 1;
    let intent = platform_core::CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let immediate = runner.apply_link_timing(
        0,
        &[intent],
        RoutedMusicalEvents {
            audio: vec![note_on(60)],
            midi: vec![],
        },
    );

    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
    let retrigger = runner.take_due_link_events(0);
    assert_eq!(
        musical_note_ons_from_events(&retrigger.audio),
        vec![(0, 60)]
    );
}

#[test]
pub(crate) fn link_arp_rotating_and_random_vary() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].arp.mode = "rotating".into();
    let routed = || RoutedMusicalEvents {
        audio: vec![note_on(60), note_on(64), note_on(67)],
        midi: vec![],
    };
    let first = runner.apply_link_timing(0, &[], routed());
    let second = runner.apply_link_timing(0, &[], routed());
    assert_ne!(
        musical_note_ons_from_events(&first.audio),
        musical_note_ons_from_events(&second.audio)
    );

    runner.pulses_layers[0].arp.mode = "random".into();
    let first = runner.apply_link_timing(0, &[], routed());
    let second = runner.apply_link_timing(0, &[], routed());
    assert_ne!(
        musical_note_ons_from_events(&first.audio),
        musical_note_ons_from_events(&second.audio)
    );
}

fn note_on(note: u8) -> platform_core::MusicalEvent {
    platform_core::MusicalEvent::NoteOn {
        channel: 0,
        note,
        velocity: 96,
        duration_ms: None,
    }
}

fn finite_input_note_on(note: u8) -> platform_core::MusicalEvent {
    platform_core::MusicalEvent::NoteOn {
        channel: 0,
        note,
        velocity: 96,
        duration_ms: Some(150),
    }
}

fn note_off(note: u8) -> platform_core::MusicalEvent {
    platform_core::MusicalEvent::NoteOff { channel: 0, note }
}

fn cc(controller: u8, value: u8) -> platform_core::MusicalEvent {
    platform_core::MusicalEvent::Cc {
        channel: 0,
        controller,
        value,
    }
}

fn has_note_on(events: &[platform_core::MusicalEvent], target: u8) -> bool {
    events.iter().any(
        |event| matches!(event, platform_core::MusicalEvent::NoteOn { note, .. } if *note == target),
    )
}

fn musical_note_ons_from_events(events: &[platform_core::MusicalEvent]) -> Vec<(u8, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            platform_core::MusicalEvent::NoteOn { channel, note, .. } => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}
