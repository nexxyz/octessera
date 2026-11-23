use super::*;
use platform_core::MusicalEvent;

/// A Sequencer on each listed layer scanning columns at 1/16 on instrument 1, with `lit`
/// columns of row 2 lit on layer 0 and the instrument in `note_mode`.
fn scan_runner(note_mode: &str, lit: &[usize]) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    if note_mode == "hold" {
        assert!(runner.menu.focus_item_key("instruments.0.noteBehavior"));
        runner.menu.state.editing = true;
        device_input(
            &mut runner,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        );
        runner.menu.state.editing = false;
    }
    scan_layer(&mut runner, 0);
    for x in lit {
        grid_tap(&mut runner, *x, 2);
    }
    runner.transport.transport = RuntimeTransportState::Playing;
    runner
}

fn scan_layer(runner: &mut NativeRunner, layer: usize) {
    let link = &mut runner.layers[layer].link;
    link.scan_mode = ScanMode::Scanning;
    link.scan_axis = ScanAxis::Columns;
    link.scan_unit = "1/16".into();
    link.event_enabled = true;
    for slot in [
        &mut link.activate_slot,
        &mut link.stable_slot,
        &mut link.deactivate_slot,
    ] {
        *slot = 0;
    }
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
}

fn set_actions(runner: &mut NativeRunner, layer: usize, actions: [TriggerAction; 3]) {
    let link = &mut runner.layers[layer].link;
    [
        link.activate_action,
        link.stable_action,
        link.deactivate_action,
    ] = actions;
    runner.refresh_active_mapping_config();
}

/// What one scan step plays on the internal lane: `+note` for a note-on, `-note` for a note-off.
fn step(runner: &mut NativeRunner) -> Vec<String> {
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: runner.step_pulses_for_layer(0),
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    routed_events(&messages, false)
        .into_iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => Some(format!("+{note}")),
            MusicalEvent::NoteOff { note, .. } => Some(format!("-{note}")),
            MusicalEvent::Cc { .. } => None,
        })
        .collect()
}

fn steps(runner: &mut NativeRunner, count: usize) -> Vec<Vec<String>> {
    (0..count).map(|_| step(runner)).collect()
}

/// The scan steps (0-based) that played anything, with what they played.
fn sounding(steps: &[Vec<String>]) -> Vec<(usize, String)> {
    steps
        .iter()
        .enumerate()
        .filter(|(_, words)| !words.is_empty())
        .map(|(index, words)| (index, words.join(" ")))
        .collect()
}

const ON_OFF: [TriggerAction; 3] = [
    TriggerAction::NoteOn,
    TriggerAction::None,
    TriggerAction::NoteOff,
];

/// The owner's row: lit at columns 1-3 and 6 on a Hold synth gives On@1, Hold@2-3, Off@4, On@6
/// and Off@7, lap after lap.
#[test]
fn a_hold_scan_plays_on_at_a_run_start_and_off_at_its_first_dark_step() {
    let mut runner = scan_runner("hold", &[1, 2, 3, 6]);
    set_actions(&mut runner, 0, ON_OFF);
    let played = sounding(&steps(&mut runner, 16));
    let note = played[0].1.trim_start_matches('+').to_string();
    let lap = |offset: usize| {
        vec![
            (offset + 1, format!("+{note}")),
            (offset + 4, format!("-{note}")),
            (offset + 6, format!("+{note}")),
            (offset + 7, format!("-{note}")),
        ]
    };
    assert_eq!(played, [lap(0), lap(8)].concat());
}

#[test]
fn a_row_lit_at_its_last_and_first_column_holds_across_the_lap_wrap() {
    let mut runner = scan_runner("hold", &[0, 7]);
    set_actions(&mut runner, 0, ON_OFF);
    let played = sounding(&steps(&mut runner, 18));
    let note = played[0].1.trim_start_matches('+').to_string();
    assert_eq!(
        played,
        [
            (0, format!("+{note}")),
            (1, format!("-{note}")),
            (7, format!("+{note}")),
            (9, format!("-{note}")),
            (15, format!("+{note}")),
            (17, format!("-{note}")),
        ]
    );
}

/// Two lanes: row 2 sits in the bottom lane, which wraps from its column 7 back to its column 0
/// one full lane later, so the hold lasts through the top lane's pass.
#[test]
fn a_sectioned_scan_wraps_each_lane_on_its_own() {
    let mut runner = scan_runner("hold", &[0, 7]);
    runner.layers[0].link.scan_sections = 2;
    runner.refresh_active_interpretation_profile();
    set_actions(&mut runner, 0, ON_OFF);
    let played = sounding(&steps(&mut runner, 18));
    let note = played[0].1.trim_start_matches('+').to_string();
    assert_eq!(
        played,
        [
            (0, format!("+{note}")),
            (1, format!("-{note}")),
            (7, format!("+{note}")),
            (17, format!("-{note}")),
        ]
    );
}

#[test]
fn event_triggers_gate_scan_on_and_off_and_state_notes_gate_its_hold() {
    let all_on = [TriggerAction::NoteOn; 3];
    let mut runner = scan_runner("oneshot", &[1, 2]);
    set_actions(&mut runner, 0, all_on);
    assert_eq!(
        sounding(&steps(&mut runner, 4))
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );

    let mut runner = scan_runner("oneshot", &[1, 2]);
    set_actions(&mut runner, 0, all_on);
    runner.layers[0].link.event_enabled = false;
    runner.refresh_active_interpretation_profile();
    let hold_only = sounding(&steps(&mut runner, 4));
    assert_eq!(hold_only.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [2]);

    let mut runner = scan_runner("oneshot", &[1, 2]);
    set_actions(&mut runner, 0, all_on);
    runner.layers[0].link.state_notes_enabled = false;
    runner.refresh_active_mapping_config();
    let events_only = sounding(&steps(&mut runner, 4));
    assert_eq!(
        events_only.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        [1, 3]
    );
}

#[test]
fn a_one_shot_scan_sounds_every_lit_step_only_with_hold_mapped_to_note_on() {
    let lit_steps = |actions: [TriggerAction; 3]| {
        let mut runner = scan_runner("oneshot", &[1, 2, 3]);
        set_actions(&mut runner, 0, actions);
        sounding(&steps(&mut runner, 8))
            .into_iter()
            .map(|(index, words)| (index, words.starts_with('+')))
            .collect::<Vec<_>>()
    };
    let note_on = TriggerAction::NoteOn;
    let none = TriggerAction::None;

    assert_eq!(lit_steps([note_on, none, none]), [(1, true)]);
    assert_eq!(
        lit_steps([note_on, note_on, none]),
        [(1, true), (2, true), (3, true)]
    );
    assert_eq!(lit_steps([note_on, none, note_on]), [(1, true), (4, true)]);
}

/// Delay and Retrig count the scanning layer's Link ticks, which are its scan steps.
#[test]
fn a_scan_on_with_delay_and_retrig_sounds_two_and_three_scan_steps_later() {
    let mut runner = scan_runner("oneshot", &[1]);
    set_actions(
        &mut runner,
        0,
        [
            TriggerAction::NoteOn,
            TriggerAction::None,
            TriggerAction::None,
        ],
    );
    runner.layers[0].link.activate_timing = LinkEventTiming {
        delay_steps: 2,
        retrigger_count: 1,
    };
    let played = sounding(&steps(&mut runner, 8));
    assert_eq!(
        played.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        [3, 4]
    );
}

/// Layer 0 scans a held note on instrument 1 that its own line never ends; layer 1 ends it.
fn cross_layer(layer_one: impl FnOnce(&mut NativeRunner)) -> Vec<Vec<String>> {
    let mut runner = scan_runner("hold", &[0, 1, 2, 3, 4, 5, 6, 7]);
    set_actions(&mut runner, 0, ON_OFF);
    runner.select_active_layer(1).unwrap();
    layer_one(&mut runner);
    runner.select_active_layer(0).unwrap();
    steps(&mut runner, 8)
}

fn ended_by_layer_one(steps: &[Vec<String>]) {
    let played = sounding(steps);
    let note = played[0].1.trim_start_matches('+').to_string();
    assert_eq!(
        played,
        [(0, format!("+{note}")), (1, format!("-{note}"))],
        "{steps:?}"
    );
}

#[test]
fn another_layers_scan_off_ends_a_scanned_hold_note() {
    ended_by_layer_one(&cross_layer(|runner| {
        runner
            .replace_layer_engine_with_config(1, NativeBehavior::Sequencer, Value::Null, None)
            .unwrap();
        scan_layer(runner, 1);
        set_actions(
            runner,
            1,
            [
                TriggerAction::None,
                TriggerAction::None,
                TriggerAction::NoteOff,
            ],
        );
        grid_tap(runner, 0, 2);
    }));
}

#[test]
fn another_layers_hold_note_off_ends_a_scanned_hold_note_once() {
    ended_by_layer_one(&cross_layer(|runner| {
        runner
            .replace_layer_engine_with_config(1, NativeBehavior::Sequencer, Value::Null, None)
            .unwrap();
        scan_layer(runner, 1);
        set_actions(
            runner,
            1,
            [
                TriggerAction::None,
                TriggerAction::NoteOff,
                TriggerAction::None,
            ],
        );
        for x in 0..8 {
            grid_tap(runner, x, 2);
        }
    }));
}

#[test]
fn another_layers_direct_off_ends_a_scanned_hold_note() {
    let mut runner = scan_runner("hold", &[0, 1, 2, 3, 4, 5, 6, 7]);
    set_actions(&mut runner, 0, ON_OFF);
    runner
        .replace_layer_engine_with_config(1, NativeBehavior::Keys, Value::Null, None)
        .unwrap();
    let link = &mut runner.layers[1].link;
    link.event_enabled = true;
    link.activate_action = TriggerAction::None;
    link.deactivate_action = TriggerAction::NoteOff;
    link.deactivate_slot = 0;
    let first = step(&mut runner);
    assert_eq!(first.len(), 1, "{first:?}");

    runner.select_active_layer(1).unwrap();
    device_input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 2 }));
    let released = device_input(
        &mut runner,
        json!({ "type": "grid_release", "x": 0, "y": 2 }),
    );
    runner.select_active_layer(0).unwrap();

    let ended = routed_events(&released, false)
        .into_iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
        .count();
    assert_eq!(ended, 1, "{released:?}");
    assert!(runner.held_notes[0].is_empty());
    assert!(steps(&mut runner, 9).iter().all(Vec::is_empty));
}

/// Who holds a scan's note changes with these settings, so its notes end on the change.
#[test]
fn scan_setting_changes_end_a_held_scan_note() {
    for (key, delta) in [
        ("layers.0.link.scanMode", -1),
        ("layers.0.link.scanAxis", -1),
        ("layers.0.link.scanDirection", 1),
        ("layers.0.link.scanSections", 1),
    ] {
        let mut runner = scan_runner("hold", &[0, 1, 2, 3, 4, 5, 6, 7]);
        set_actions(&mut runner, 0, ON_OFF);
        let on = step(&mut runner);
        assert_eq!(on.len(), 1, "{key}");
        runner.menu.rebuild(runner.menu_config());
        runner.edit_menu_key(key, delta).unwrap();
        let messages = runner.messages_without_snapshot().unwrap();
        let ended = routed_events(&messages, false)
            .into_iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
            .count();
        assert_eq!(ended, 1, "{key}");
        assert!(runner.held_notes[0].is_empty(), "{key}");
    }
}

#[test]
fn a_row_scan_holds_down_a_column_and_a_reverse_scan_runs_it_backwards() {
    let mut rows = scan_runner("hold", &[]);
    rows.layers[0].link.scan_axis = ScanAxis::Rows;
    rows.refresh_active_interpretation_profile();
    grid_tap(&mut rows, 3, 1);
    grid_tap(&mut rows, 3, 2);
    set_actions(&mut rows, 0, ON_OFF);
    let played = sounding(&steps(&mut rows, 4));
    assert_eq!(
        played.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        [1, 3],
        "On at row 1, Off at row 3: {played:?}"
    );

    let mut reverse = scan_runner("hold", &[1, 2]);
    reverse.layers[0].link.scan_direction = ScanDirection::Reverse;
    reverse.refresh_active_interpretation_profile();
    set_actions(&mut reverse, 0, ON_OFF);
    let played = sounding(&steps(&mut reverse, 8));
    assert_eq!(
        played.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        [5, 7],
        "columns 7 to 0: On at column 2, Off at column 0: {played:?}"
    );
}
