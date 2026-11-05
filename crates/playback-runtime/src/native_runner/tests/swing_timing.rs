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
fn switching_to_straight_mid_beat_finishes_the_beat_swung_then_runs_straight() {
    let mut runner = eighth_note_layer((7, 1));
    note_on_times(&mut runner, 30);
    set_swing_through_menu(&mut runner, 4, 4);
    // Beat 2 (pulses 24..48) keeps 7:1, its offbeat at 24 + 21; straight eighths from pulse 48.
    assert_eq!(
        note_on_times(&mut runner, 42),
        vec![(15, 0), (18, 0), (30, 0), (42, 0)]
    );
}

#[test]
fn a_mid_beat_swing_change_waits_for_the_next_beat_without_flam_or_gap() {
    // 7:1 puts the offbeat at pulse 21 of the beat, 1:7 at pulse 3.
    for (from, to, expected) in [
        (
            (7, 1),
            (1, 7),
            vec![(15, 0), (18, 0), (21, 0), (42, 0), (45, 0)],
        ),
        ((1, 7), (7, 1), vec![(18, 0), (39, 0), (42, 0)]),
    ] {
        let mut runner = eighth_note_layer(from);
        note_on_times(&mut runner, 30);
        set_swing_through_menu(&mut runner, to.0, to.1);
        assert_eq!(
            note_on_times(&mut runner, 48),
            expected,
            "{from:?} -> {to:?}"
        );
    }
}

#[test]
fn swing_changed_on_every_pulse_plays_each_beat_on_its_starting_ratio() {
    let mut runner = eighth_note_layer((4, 4));
    let ratios: [(u8, u8); 7] = [(7, 1), (4, 5), (5, 3), (1, 7), (2, 6), (6, 6), (3, 5)];
    let mut times = Vec::new();
    let mut expected = Vec::new();
    for pulse in 0..(4 * 96u64) {
        let (a, b) = ratios[pulse as usize % ratios.len()];
        set_swing_through_menu(&mut runner, a.into(), b.into());
        if (pulse + 1) % 24 == 0 {
            let offbeat = 24 * u64::from(a);
            let units = u64::from(a + b);
            let offset = (PULSE_NANOS_AT_120 * (offbeat % units) as f64 / units as f64).round();
            expected.push((pulse + 1 + offbeat / units, offset as u64));
            expected.push((pulse + 25, 0));
        }
        times.extend(
            note_on_times(&mut runner, 1)
                .into_iter()
                .map(|(_, offset)| (pulse + 1, offset)),
        );
    }
    expected.retain(|(pulse, _)| *pulse <= 4 * 96);
    // The first beat runs on the ratio set before Play.
    assert_eq!(&times[..2], &[(12, 0), (24, 0)]);
    assert_eq!(&times[2..], &expected[..], "{times:?}");
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
    // The beat that started on 3:5 finishes on it; straight eighths from the next beat.
    assert_eq!(
        note_on_times(&mut runner, 48),
        vec![(9, 0), (24, 0), (36, 0), (48, 0)]
    );
}

#[test]
fn external_sync_swing_lands_on_the_first_clock_at_or_after_its_time() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.midi_active = true;
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
