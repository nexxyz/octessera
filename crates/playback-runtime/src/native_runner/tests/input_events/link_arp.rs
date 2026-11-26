use super::*;

#[test]
pub(crate) fn link_arp_orders_simultaneous_batches_with_finite_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.step_interval_steps = 1;
    let routed = vec![note_on(64), note_on(60), note_on(67)];

    let immediate = route(&mut runner, routed);
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

    let first = due(&mut runner);
    let second = due(&mut runner);
    assert_eq!(musical_note_ons_from_events(&first.audio), vec![(0, 64)]);
    assert_eq!(musical_note_ons_from_events(&second.audio), vec![(0, 67)]);
}

#[test]
pub(crate) fn link_arp_floors_short_gated_notes_at_ten_ms_on_both_lanes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Direct;
    runner.layers[0].link.arp.note_length_ms = 50;
    runner.layers[0].link.arp.gate_pct = 10;
    runner.instruments[1].kind = InstrumentType::Midi;
    runner.instruments[1].midi_enabled = true;
    runner.instruments[1].midi_channel = 1;
    let mut midi_note = note_on(62);
    if let platform_core::MusicalEvent::NoteOn { channel, .. } = &mut midi_note {
        *channel = 1;
    }

    let immediate = route(&mut runner, vec![note_on(60), midi_note]);
    for event in immediate.audio.iter().chain(&immediate.midi) {
        assert!(matches!(
            event,
            platform_core::MusicalEvent::NoteOn {
                duration_ms: Some(10),
                ..
            }
        ));
    }
    assert_eq!(immediate.audio.len() + immediate.midi.len(), 2);
}

#[test]
pub(crate) fn link_arp_passes_non_note_events_through() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Up;
    let immediate = route(&mut runner, vec![cc(74, 90), note_on(60)]);

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
pub(crate) fn link_arp_held_source_passes_non_note_events_without_replaying_the_held_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Direct;
    runner.layers[0].link.arp.source = ArpSource::Held;
    route(&mut runner, vec![note_on(60)]);

    let immediate = route(&mut runner, vec![cc(74, 90)]);

    assert!(immediate.audio.iter().any(|event| matches!(
        event,
        platform_core::MusicalEvent::Cc {
            controller: 74,
            value: 90,
            ..
        }
    )));
    assert!(musical_note_ons_from_events(&immediate.audio).is_empty());

    let replayed = route(&mut runner, vec![note_on(64)]);
    assert_eq!(
        musical_note_ons_from_events(&replayed.audio),
        vec![(0, 60), (0, 64)]
    );
}

#[test]
pub(crate) fn link_arp_steps_one_batch_across_interleaved_note_offs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.step_interval_steps = 1;

    let immediate = route(&mut runner, vec![note_on(67), note_off(62), note_on(60)]);

    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
    let next = due(&mut runner);
    assert_eq!(musical_note_ons_from_events(&next.audio), vec![(0, 67)]);
}

#[test]
pub(crate) fn link_arp_rotating_moves_a_two_note_chord_one_place_per_batch() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Rotating;
    runner.layers[0].link.arp.step_interval_steps = 1;

    let firsts = (0..3)
        .map(|_| {
            let immediate = route(&mut runner, vec![note_on(60), note_on(64)]);
            runner.clear_delayed_link_events_for_layer(0);
            musical_note_ons_from_events(&immediate.audio)
        })
        .collect::<Vec<_>>();

    assert_eq!(firsts, vec![vec![(0, 60)], vec![(0, 64)], vec![(0, 60)]]);
}

#[test]
pub(crate) fn link_arp_held_source_does_not_replay_the_held_notes_on_a_release() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Direct;
    runner.layers[0].link.arp.source = ArpSource::Held;
    route(&mut runner, vec![note_on(60), note_on(64)]);
    runner.clear_delayed_link_events_for_layer(0);

    let released = route(&mut runner, vec![note_off(64)]);
    assert!(musical_note_ons_from_events(&released.audio).is_empty());
    assert!(due(&mut runner).is_empty());

    let pressed = route(&mut runner, vec![note_on(67)]);
    assert_eq!(
        musical_note_ons_from_events(&pressed.audio),
        vec![(0, 60), (0, 67)]
    );
}

#[test]
pub(crate) fn link_arp_releases_a_pitch_before_the_same_step_plays_it_again() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Direct;
    runner.layers[0].link.arp.source = ArpSource::Held;
    route(&mut runner, vec![note_on(60)]);

    route(&mut runner, vec![note_on(60), note_off(60)]);
    let replayed = route(&mut runner, vec![note_on(64)]);

    assert_eq!(
        musical_note_ons_from_events(&replayed.audio),
        vec![(0, 60), (0, 64)]
    );
}

#[test]
pub(crate) fn link_arp_octave_spread_expands_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::OctaveSpread;
    runner.layers[0].link.arp.octave_spread = 1;
    runner.layers[0].link.arp.step_interval_steps = 1;
    let immediate = route(&mut runner, vec![note_on(60)]);

    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
    let delayed = due(&mut runner);
    assert_eq!(musical_note_ons_from_events(&delayed.audio), vec![(0, 72)]);
}

#[test]
pub(crate) fn link_arp_default_none_matches_existing_link_timing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.activate_timing.delay_steps = 1;
    runner.layers[0].link.activate_timing.retrigger_count = 1;
    let routed = vec![finite_input_note_on(60)];

    let immediate = route(&mut runner, routed);
    assert!(immediate.audio.is_empty());
    let delayed = due(&mut runner);
    assert_eq!(delayed.audio, vec![finite_input_note_on(60)]);
    let retrigger = due(&mut runner);
    assert_eq!(retrigger.audio, vec![finite_input_note_on(60)]);
}

#[test]
pub(crate) fn link_arp_direct_preserves_input_order_and_large_batch_offsets_do_not_collapse() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Direct;
    let routed = vec![note_on(67), note_on(60), note_on(64)];
    let immediate = route(&mut runner, routed);
    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 67), (0, 60), (0, 64)]
    );

    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.step_interval_steps = 16;
    let routed = (0..20).map(|index| note_on(40 + index)).collect();
    route(&mut runner, routed);
    assert!(runner.delayed_link_events[0]
        .iter()
        .any(|entry| entry.remaining_steps > u16::from(u8::MAX)));
}

#[test]
pub(crate) fn link_arp_held_source_updates_releases_and_suppresses_note_offs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.source = ArpSource::Held;
    runner.layers[0].link.arp.step_interval_steps = 1;

    let first = route(&mut runner, vec![note_on(64)]);
    assert_eq!(musical_note_ons_from_events(&first.audio), vec![(0, 64)]);
    let second = route(&mut runner, vec![note_on(60)]);
    assert_eq!(musical_note_ons_from_events(&second.audio), vec![(0, 60)]);
    let tail = due(&mut runner);
    assert_eq!(musical_note_ons_from_events(&tail.audio), vec![(0, 64)]);

    route(&mut runner, vec![note_off(64)]);
    assert!(runner.delayed_link_events[0]
        .iter()
        .all(|entry| !entry.events.presses().any(|press| press.pitch == 64)));
}

#[test]
pub(crate) fn link_arp_held_source_holds_a_one_shot_key_until_its_release_and_stop_forgets_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.source = ArpSource::Held;

    let lit = route(&mut runner, vec![finite_input_note_on(60)]);
    assert_eq!(musical_note_ons_from_events(&lit.audio), vec![(0, 60)]);
    assert_eq!(runner.link_arp.keys[0].len(), 1);
    route(&mut runner, vec![note_off(60)]);
    assert!(runner.link_arp.keys[0].is_empty());

    route(&mut runner, vec![note_on(64)]);
    assert!(!runner.link_arp.keys[0].is_empty());
    *runner.arp_random_state_mut(0, false) = 123;
    runner.reset_transport_position();
    assert!(runner.link_arp.keys[0].is_empty());
    assert_ne!(*runner.arp_random_state_mut(0, false), 123);
}

#[test]
pub(crate) fn link_arp_held_source_retriggers_finite_arp_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Direct;
    runner.layers[0].link.arp.source = ArpSource::Held;
    runner.layers[0].link.activate_timing.retrigger_count = 1;

    let immediate = route(&mut runner, vec![note_on(60)]);

    assert_eq!(
        musical_note_ons_from_events(&immediate.audio),
        vec![(0, 60)]
    );
    let retrigger = due(&mut runner);
    assert_eq!(
        musical_note_ons_from_events(&retrigger.audio),
        vec![(0, 60)]
    );
}

#[test]
pub(crate) fn link_arp_rotating_and_random_vary() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Rotating;
    let routed = || vec![note_on(60), note_on(64), note_on(67)];
    let first = route(&mut runner, routed());
    let second = route(&mut runner, routed());
    assert_ne!(
        musical_note_ons_from_events(&first.audio),
        musical_note_ons_from_events(&second.audio)
    );

    runner.layers[0].link.arp.mode = ArpMode::Random;
    let leads = (0..8)
        .map(|_| musical_note_ons_from_events(&route(&mut runner, routed()).audio))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(leads.len() > 1, "{leads:?}");
}

#[test]
fn random_arp_orders_are_not_a_fixed_cycle() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layers[0].link.arp.mode = ArpMode::Random;
    let mut orders = Vec::new();
    for _ in 0..600 {
        runner.clear_link_arp_state_for_layer(0);
        let routed = vec![note_on(60), note_on(64), note_on(67)];
        let immediate = route(&mut runner, routed);
        let mut order = musical_note_ons_from_events(&immediate.audio);
        order.extend(
            runner.delayed_link_events[0]
                .drain(..)
                .flat_map(|entry| entry.events.keys)
                .filter_map(|event| match event {
                    crate::native_runner::held_keys::KeyEvent::Press(press) => {
                        Some((press.slot, press.pitch))
                    }
                    _ => None,
                }),
        );
        orders.push(order.iter().map(|(_, note)| *note).collect::<Vec<_>>());
    }
    let mut counts = std::collections::BTreeMap::new();
    for order in &orders {
        *counts.entry(order.clone()).or_insert(0) += 1;
    }
    assert_eq!(counts.len(), 6, "{counts:?}");
    assert!(
        counts.values().all(|count| (60..=140).contains(count)),
        "{counts:?}"
    );

    let leads = orders.iter().map(|order| order[0] < order[1]);
    let flips = leads
        .clone()
        .zip(leads.skip(1))
        .filter(|(before, after)| before != after)
        .count();
    assert!((240..=360).contains(&flips), "{flips} flips");
}

#[test]
fn routed_random_arp_draws_from_its_layers_seeded_or_unseeded_stream() {
    for seeded in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.layers[0].link.arp.mode = ArpMode::Random;
        runner.layers[0].link.seeded = seeded;
        let walks = |runner: &mut NativeRunner| {
            [
                *runner.arp_random_state_mut(0, false),
                *runner.arp_random_state_mut(0, true),
                *runner.arp_random_state_mut(1, false),
            ]
        };
        let before = walks(&mut runner);
        let intents: Vec<_> = (0..3)
            .map(|x| {
                Some(platform_core::TileTriggerIntent {
                    x,
                    y: 0,
                    degree: 0,
                    kind: platform_core::TileTriggerKind::Activate,
                })
            })
            .collect();
        runner
            .route_events_with_link_timing(
                0,
                super::super::super::algorithm::LinkRoutingInput {
                    events: vec![note_on(60), note_on(64), note_on(67)],
                    event_intents: &intents,
                    transpose_offset: 0,
                },
            )
            .unwrap();
        let after = walks(&mut runner);
        assert_eq!(before[0] != after[0], !seeded, "seeded={seeded}");
        assert_eq!(before[1] != after[1], seeded, "seeded={seeded}");
        assert_eq!(before[2], after[2], "seeded={seeded}");
    }
}

fn route(
    runner: &mut NativeRunner,
    events: Vec<platform_core::MusicalEvent>,
) -> RoutedMusicalEvents {
    let intents = events
        .iter()
        .map(|event| {
            Some(platform_core::TileTriggerIntent {
                x: 0,
                y: 0,
                degree: 0,
                kind: if matches!(event, platform_core::MusicalEvent::NoteOff { .. }) {
                    platform_core::TileTriggerKind::Deactivate
                } else {
                    platform_core::TileTriggerKind::Activate
                },
            })
        })
        .collect::<Vec<_>>();
    let routed = runner
        .route_events_with_link_timing(
            0,
            super::super::super::algorithm::LinkRoutingInput {
                events,
                event_intents: &intents,
                transpose_offset: 0,
            },
        )
        .unwrap();
    routed.sounded(&runner.instruments)
}

/// What the layer's queue plays on its next step.
fn due(runner: &mut NativeRunner) -> RoutedMusicalEvents {
    let due = runner.take_due_link_events(0);
    due.sounded(&runner.instruments)
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

fn musical_note_ons_from_events(events: &[platform_core::MusicalEvent]) -> Vec<(u8, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            platform_core::MusicalEvent::NoteOn { channel, note, .. } => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}
