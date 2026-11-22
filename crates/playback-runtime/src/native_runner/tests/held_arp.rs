use super::*;

fn set_link(runner: &mut NativeRunner, field: &str, value: &str) {
    let key = format!("layers.0.link.{field}");
    assert!(runner.menu.set_enum_value_for_key(&key, value), "{key}");
    runner.apply_or_schedule_menu_key(&key).unwrap();
}

fn step(runner: &mut NativeRunner) {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.step_pulses_for_layer(0),
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
}

#[test]
pub(crate) fn a_scanning_hold_layer_forgets_the_pitches_the_scan_line_has_passed() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
        grid_tap(&mut runner, x, y);
    }
    runner.instruments[0].note_behavior = NoteBehavior::Hold;
    runner.sync_engine_runtime_config();
    set_link(&mut runner, "scanMode", "scanning");
    set_link(&mut runner, "arp.mode", "chord_strike");
    set_link(&mut runner, "arp.source", "held");
    runner.transport.transport = RuntimeTransportState::Playing;

    let mut held_any = false;
    for _ in 0..2 * GRID_WIDTH {
        step(&mut runner);
        let sounding = runner.held_notes[0].sounding_keys();
        let held = runner.held_notes[0].replays();
        held_any |= !held.is_empty();
        assert!(
            held.iter()
                .all(|(channel, note, _)| sounding.contains(&(*channel, *note))),
            "the Arp holds only sounding pitches: {held:?} vs {sounding:?}"
        );
    }
    assert!(held_any, "the scan line holds the block's pitches");
}

#[test]
pub(crate) fn a_new_held_arp_batch_drops_the_last_runs_pending_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.source = ArpSource::Held;
    for y in [1, 3] {
        device_input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": y }));
    }
    assert_eq!(runner.delayed_link_events[0].len(), 1, "two lit: one waits");

    device_input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 5 }));

    assert_eq!(
        runner.delayed_link_events[0].len(),
        2,
        "three lit: two wait, none left from the run before"
    );
}

/// Hands `events` from tiles `x` of row 0 to the layer's ledger and routing, as a step does, and
/// returns the internal note-ons that sound at once.
fn play_link_events(runner: &mut NativeRunner, notes: &[(usize, u8)]) -> Vec<u8> {
    let mut events = notes
        .iter()
        .map(|(_, note)| MusicalEvent::NoteOn {
            channel: 0,
            note: *note,
            velocity: 100,
            duration_ms: Some(150),
        })
        .collect::<Vec<_>>();
    let mut intents = notes
        .iter()
        .map(|(x, _)| {
            Some(platform_core::TileTriggerIntent {
                x: *x,
                y: 0,
                degree: 0,
                kind: platform_core::TileTriggerKind::Activate,
            })
        })
        .collect::<Vec<_>>();
    runner.hold_layer_notes(0, &mut events, &mut intents, None);
    let transpose_offset = runner.play_transpose_offset_for_layer(0);
    let routed = runner
        .route_events_with_link_timing(
            0,
            super::super::algorithm::LinkRoutingInput {
                events,
                event_intents: &intents,
                transpose_offset,
                lit_tiles: &[true; GRID_WIDTH * GRID_HEIGHT],
            },
        )
        .unwrap();
    routed
        .audio
        .iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => Some(*note),
            _ => None,
        })
        .collect()
}

#[test]
pub(crate) fn a_held_arp_replays_its_held_notes_through_the_current_transpose() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].note_behavior = NoteBehavior::Hold;
    runner.sync_engine_runtime_config();
    runner.layers[0].link.arp.mode = ArpMode::ChordStrike;
    runner.layers[0].link.arp.source = ArpSource::Held;
    runner.transport.transport = RuntimeTransportState::Playing;

    let chord = play_link_events(&mut runner, &[(0, 60), (1, 64), (2, 67)]);
    assert_eq!(chord, vec![60, 64, 67]);

    runner.handle_play_transpose_grid_press(2, 3);
    assert_eq!(runner.play_transpose_offset_for_layer(0), 2);

    let with_a = play_link_events(&mut runner, &[(3, 69)]);
    assert_eq!(with_a, vec![62, 66, 69, 71]);
}
