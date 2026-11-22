use super::*;
use platform_core::MusicalEvent;

fn keys_runner(note_behavior_turn: i8) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    if note_behavior_turn != 0 {
        menu_turn(
            &mut runner,
            "instruments.0.noteBehavior",
            note_behavior_turn,
        );
    }
    runner
}

fn menu_turn(runner: &mut NativeRunner, key: &str, delta: i8) -> Vec<MusicalEvent> {
    assert!(runner.menu.focus_item_key(key), "{key}");
    runner.menu.state.editing = true;
    let messages = device_input(
        runner,
        json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
    );
    runner.menu.state.editing = false;
    notes(&messages)
}

fn notes(messages: &[RunnerMessage]) -> Vec<MusicalEvent> {
    routed_events(messages, false)
        .into_iter()
        .chain(routed_events(messages, true))
        .filter(|event| !matches!(event, MusicalEvent::Cc { .. }))
        .collect()
}

fn grid(runner: &mut NativeRunner, kind: &str, x: usize, y: usize) -> Vec<MusicalEvent> {
    notes(&device_input(
        runner,
        json!({ "type": kind, "x": x, "y": y }),
    ))
}

fn on_off(events: &[MusicalEvent]) -> Vec<(bool, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => Some((true, *note)),
            MusicalEvent::NoteOff { note, .. } => Some((false, *note)),
            MusicalEvent::Cc { .. } => None,
        })
        .collect()
}

#[test]
pub(crate) fn two_tiles_on_one_held_pitch_sound_once_until_the_last_lets_go() {
    let mut runner = keys_runner(1);
    assert_eq!(runner.instruments[0].note_behavior, NoteBehavior::Hold);
    assert_eq!(runner.layers[0].link.x_pitch_steps, 0);

    let first = on_off(&grid(&mut runner, "grid_press", 2, 3));
    let [(true, note)] = first[..] else {
        panic!("{first:?}");
    };
    assert!(grid(&mut runner, "grid_press", 5, 3).is_empty());
    assert!(grid(&mut runner, "grid_release", 2, 3).is_empty());
    assert_eq!(
        on_off(&grid(&mut runner, "grid_release", 5, 3)),
        vec![(false, note)]
    );
}

#[test]
pub(crate) fn a_held_note_ends_on_its_own_pitch_after_the_mapping_moves() {
    let mut runner = keys_runner(1);
    let pressed = on_off(&grid(&mut runner, "grid_press", 2, 3));
    let [(true, note)] = pressed[..] else {
        panic!("{pressed:?}");
    };

    for key in [
        "layers.0.link.pitch.startingNote",
        "layers.0.link.pitch.root",
        "layers.0.link.y.pitch.steps",
    ] {
        assert!(menu_turn(&mut runner, key, 2).is_empty(), "{key}");
    }

    assert_eq!(
        on_off(&grid(&mut runner, "grid_release", 2, 3)),
        vec![(false, note)]
    );
    let moved = on_off(&grid(&mut runner, "grid_press", 2, 3));
    assert!(
        matches!(moved[..], [(true, other)] if other != note),
        "{moved:?}"
    );
}

#[test]
pub(crate) fn negative_pitch_steps_descend_through_the_set() {
    let mut runner = keys_runner(0);
    menu_turn(&mut runner, "layers.0.link.x.pitch.steps", -1);
    assert!(runner.layers[0].link.x_pitch_steps < 0);

    let mut row = Vec::new();
    for x in 0..3 {
        let pressed = on_off(&grid(&mut runner, "grid_press", x, 3));
        grid(&mut runner, "grid_release", x, 3);
        let [(true, note)] = pressed[..] else {
            panic!("{pressed:?}");
        };
        row.push(note);
    }
    assert!(row[0] > row[1] && row[1] > row[2], "{row:?}");
}

#[test]
pub(crate) fn a_one_shot_midi_slot_plays_the_layer_one_shot_length() {
    let mut runner = keys_runner(0);
    runner.test_set_midi_enabled(true);
    let instrument = &mut runner.instruments[0];
    instrument.kind = InstrumentType::Midi;
    instrument.midi_enabled = true;
    instrument.midi_channel = 2;
    runner.layers[0].link.one_shot_length_ms = 333;
    runner.refresh_active_mapping_config();

    let pressed = grid(&mut runner, "grid_press", 2, 3);

    assert!(
        matches!(
            pressed[..],
            [MusicalEvent::NoteOn {
                channel: 1,
                duration_ms: Some(333),
                ..
            }]
        ),
        "{pressed:?}"
    );
}

/// A scanning Sequencer on a Hold instrument: lit steps in a row form one held note, an empty
/// step ends it, and the second lap plays the same notes instead of droning.
#[test]
pub(crate) fn a_scanned_hold_note_lasts_while_its_pitch_stays_lit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    menu_turn(&mut runner, "instruments.0.noteBehavior", 1);
    let link = &mut runner.layers[0].link;
    link.scan_mode = ScanMode::Scanning;
    link.scan_axis = ScanAxis::Columns;
    link.scan_unit = "1/16".into();
    link.event_enabled = false;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    for x in [0, 1, 3] {
        grid_tap(&mut runner, x, 2);
    }
    runner.transport.transport = RuntimeTransportState::Playing;

    let steps = (0..16)
        .map(|_| {
            let messages = runner
                .send(HostMessage::TransportPulseStep {
                    pulses: runner.step_pulses_for_layer(0),
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: Some(false),
                })
                .unwrap();
            on_off(&notes(&messages))
                .into_iter()
                .map(|(on, _)| on)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let lap = [
        vec![true],
        vec![],
        vec![false],
        vec![true],
        vec![false],
        vec![],
        vec![],
        vec![],
    ];
    assert_eq!(steps[..8], lap[..], "{steps:?}");
    assert_eq!(steps[8..], lap[..], "{steps:?}");
}

#[test]
pub(crate) fn a_held_pitch_sounds_again_after_its_instrument_drops_its_voices() {
    let mut runner = keys_runner(1);
    let pressed = grid(&mut runner, "grid_press", 2, 3);
    assert_eq!(pressed.len(), 1);

    runner.drain_play_transpose_instrument_notes(0);

    let pressed_again = on_off(&grid(&mut runner, "grid_press", 5, 3));
    assert_eq!(
        pressed_again.iter().filter(|(on, _)| *on).count(),
        1,
        "{pressed_again:?}"
    );
}

/// A scan that stops on an otherwise idle `none` layer still ends the notes it held.
#[test]
pub(crate) fn an_idle_layer_ends_its_scan_hold_once_scanning_stops() {
    let mut runner = keys_runner(1);
    let link = &mut runner.layers[1].link;
    link.scan_mode = ScanMode::Scanning;
    link.scan_unit = "1/16".into();
    link.scanned_empty_slot = 0;
    link.scanned_empty_action = TriggerAction::NoteOn;
    let step = |runner: &mut NativeRunner| {
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: runner.step_pulses_for_layer(1),
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        on_off(&notes(&messages))
    };
    let held = step(&mut runner);
    assert!(
        !held.is_empty() && held.iter().all(|(on, _)| *on),
        "{held:?}"
    );

    runner.layers[1].link.scan_mode = ScanMode::None;
    let released = step(&mut runner);

    assert_eq!(released.len(), held.len(), "{released:?}");
    assert!(released.iter().all(|(on, _)| !on), "{released:?}");
}
