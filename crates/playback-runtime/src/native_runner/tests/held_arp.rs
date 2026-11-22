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
        let sounding = &runner.transpose.active_notes[0];
        let held = &runner.link_arp.held_notes[0];
        held_any |= !held.is_empty();
        assert!(
            held.iter()
                .all(|note| sounding.contains_key(&(note.channel, note.note))),
            "the Arp holds only sounding pitches: {held:?} vs {:?}",
            sounding.keys().collect::<Vec<_>>()
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
