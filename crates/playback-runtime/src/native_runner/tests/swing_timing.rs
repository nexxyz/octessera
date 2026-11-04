use super::*;

const PULSE_NANOS_AT_120: f64 = 60e9 / 24.0 / 120.0;

/// An eighth-note layer that sounds a note on every step, started from stop with the Play key.
fn eighth_note_layer(swing: (u8, u8)) -> NativeRunner {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    start_eighth_note_layer(runner, swing)
}

fn start_eighth_note_layer(mut runner: NativeRunner, swing: (u8, u8)) -> NativeRunner {
    let layer = &mut runner.link_layers[0];
    layer.scan_mode = "scanning".into();
    layer.scan_axis = "rows".into();
    layer.scan_unit = "1/8".into();
    layer.scan_sections = 1;
    layer.scanned_slot = 0;
    layer.scanned_action = "note_on".into();
    layer.scanned_empty_action = "none".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    for y in 0..GRID_HEIGHT {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 0, "y": y }),
                request_snapshot: None,
            })
            .unwrap();
    }
    set_swing_through_menu(&mut runner, swing.0.into(), swing.1.into());
    for pressed in [true, false] {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "button_s", "pressed": pressed }),
                request_snapshot: None,
            })
            .unwrap();
    }
    runner
}

/// `(pulse, offset nanos)` of every note-on in the next `pulses` single-pulse steps.
fn note_on_times(runner: &mut NativeRunner, pulses: u64) -> Vec<(u64, u64)> {
    let mut times = Vec::new();
    for pulse in 1..=pulses {
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        let mut offset = 0;
        for message in messages {
            match message {
                RunnerMessage::SequencedOffset { nanos } => offset = nanos,
                RunnerMessage::MusicalEvents { events } | RunnerMessage::MidiEvents { events } => {
                    for event in events {
                        if matches!(event, MusicalEvent::NoteOn { .. }) {
                            times.push((pulse, offset));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    times
}

#[test]
fn straight_eighths_land_on_the_pulse_grid_without_offsets() {
    let mut runner = eighth_note_layer((4, 4));
    assert_eq!(
        note_on_times(&mut runner, 48),
        vec![(12, 0), (24, 0), (36, 0), (48, 0)]
    );
}

#[test]
fn swung_offbeat_sounds_at_the_exact_ratio_point_between_pulses() {
    let mut runner = eighth_note_layer((4, 5));
    let two_thirds = (PULSE_NANOS_AT_120 * 2.0 / 3.0).round() as u64;
    // 4:5 puts the offbeat at 24 * 4 / 9 = 10 2/3 pulses into each beat.
    assert_eq!(
        note_on_times(&mut runner, 48),
        vec![(10, two_thirds), (24, 0), (34, two_thirds), (48, 0)]
    );
}

#[test]
fn late_swing_offbeat_on_a_pulse_has_no_offset() {
    let mut runner = eighth_note_layer((5, 3));
    // 5:3 puts the offbeat at 24 * 5 / 8 = 15 pulses: exactly on the grid.
    assert_eq!(
        note_on_times(&mut runner, 48),
        vec![(15, 0), (24, 0), (39, 0), (48, 0)]
    );
}

#[test]
fn swing_menu_rows_set_the_ratio_through_the_menu_dispatch() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (key, value) in [("transport.swingA", 5), ("transport.swingB", 3)] {
        assert!(runner.menu.set_number_value_for_key(key, value));
        runner.apply_or_schedule_menu_key(key).unwrap();
    }
    assert_eq!((runner.transport.swing_a, runner.transport.swing_b), (5, 3));
    let transport = &runner.config_payload()["runtimeConfig"]["transport"];
    assert_eq!(transport["swingA"], 5);
    assert_eq!(transport["swingB"], 3);
}

fn set_swing_through_menu(runner: &mut NativeRunner, a: i32, b: i32) {
    for (key, value) in [("transport.swingA", a), ("transport.swingB", b)] {
        if runner.menu.set_number_value_for_key(key, value) {
            runner.apply_or_schedule_menu_key(key).unwrap();
        }
    }
}

#[test]
fn switching_to_straight_mid_beat_keeps_layers_on_the_bar() {
    let mut runner = eighth_note_layer((7, 1));
    note_on_times(&mut runner, 30);
    set_swing_through_menu(&mut runner, 4, 4);
    // 7:1 holds the warped clock behind until pulse 21; straight time must hand back the gap.
    let times = note_on_times(&mut runner, 42);
    assert_eq!(times.last(), Some(&(42, 0)), "{times:?}");
    assert!(times
        .iter()
        .all(|(pulse, offset)| (pulse + 30) % 12 == 0 && *offset == 0));
}

#[test]
fn swing_modulated_across_eight_bars_does_not_drift() {
    let mut runner = eighth_note_layer((4, 4));
    let ratios = [(7, 1), (4, 4), (5, 3), (2, 2), (1, 7), (6, 6), (3, 5)];
    let mut notes = 0;
    for pulse in 0..(8 * 96) {
        if pulse % 5 == 0 {
            let (a, b) = ratios[pulse / 5 % ratios.len()];
            set_swing_through_menu(&mut runner, a, b);
        }
        notes += note_on_times(&mut runner, 1).len();
    }
    assert_eq!(notes, 8 * 8);
    set_swing_through_menu(&mut runner, 4, 4);
    assert_eq!(note_on_times(&mut runner, 24), vec![(12, 0), (24, 0)]);
}

#[test]
fn external_sync_swing_lands_on_the_first_clock_at_or_after_its_time() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.midi_enabled = true;
    runner.midi_clock_in_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    let mut runner = start_eighth_note_layer(runner, (4, 5));
    let mut times = Vec::new();
    for pulse in 0..=48 {
        let messages = runner
            .send(HostMessage::MidiRealtimeClock { pulses: 1 })
            .unwrap();
        for message in messages {
            match message {
                RunnerMessage::SequencedOffset { nanos } => assert_eq!(nanos, 0),
                RunnerMessage::MusicalEvents { events } | RunnerMessage::MidiEvents { events } => {
                    let notes = events
                        .iter()
                        .filter(|event| matches!(event, MusicalEvent::NoteOn { .. }));
                    times.extend(notes.map(|_| pulse));
                }
                _ => {}
            }
        }
    }
    // The first clock is the downbeat; 4:5 puts the offbeat at 10 2/3 pulses, so it waits for 11.
    assert_eq!(times, vec![0, 11, 24, 35, 48]);
}
