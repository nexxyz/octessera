//! Scan lanes: `poly` lets every scanned cell play its own note, `mono` gives each lane (a row on
//! a column scan) one note at a time; and what each scan setting change does to them.

use super::scan_events::{scan_runner, set_actions, step};
use super::tempo_modulation::{map_cc, send_cc};
use super::*;

const ON_HOLD_OFF: [TriggerAction; 3] = [
    TriggerAction::NoteOn,
    TriggerAction::NoteOn,
    TriggerAction::NoteOff,
];

/// A Hold Synth row-scanning column 3 lit at `rows`, every row one scale step higher, in
/// `lanes`.
fn melody(rows: &[usize], lanes: ScanLanes) -> NativeRunner {
    let mut runner = scan_runner("hold", &[]);
    runner.layers[0].link.scan_axis = ScanAxis::Rows;
    runner.layers[0].link.scan_lanes = lanes;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    for y in rows {
        grid_tap(&mut runner, 3, *y);
    }
    set_actions(&mut runner, 0, ON_HOLD_OFF);
    runner
}

/// Holds a key on the scan's instrument from another source and returns its note-off.
fn hold_key_on_layer_one(runner: &mut NativeRunner) -> String {
    hold_key(runner, 0, 30);
    "-30".into()
}

#[test]
fn a_mono_lane_plays_a_melody_down_its_column_each_note_replacing_the_last() {
    let mut runner = melody(&[0, 1, 2], ScanLanes::Mono);
    let first = step(&mut runner);
    let [first] = &first[..] else {
        panic!("{first:?}")
    };
    let second = step(&mut runner);
    assert_eq!(second[0], first.replace('+', "-"), "{second:?}");
    assert_eq!(second.len(), 2);
    let third = step(&mut runner);
    assert_eq!(third[0], second[1].replace('+', "-"), "{third:?}");
    let off = step(&mut runner);
    assert_eq!(
        off,
        [third[1].replace('+', "-")],
        "the dark cell's Off ends the lane's last note, not its own pitch"
    );
    assert_eq!(runner.held_keys.held_count(), 0);
}

#[test]
fn poly_lanes_let_every_cell_hold_its_own_note() {
    let mut runner = melody(&[0, 1, 2], ScanLanes::Poly);
    let played = (0..4).flat_map(|_| step(&mut runner)).collect::<Vec<_>>();
    assert_eq!(
        played.iter().filter(|word| word.starts_with('-')).count(),
        0,
        "{played:?}"
    );
    assert_eq!(runner.held_keys.held_count(), 3);
}

/// A column scan's lanes are its rows: rows 2 and 4 play a two-note chord, one note per lane.
#[test]
fn two_mono_lanes_play_a_chord_one_note_each() {
    let mut runner = scan_runner("hold", &[0, 1]);
    runner.layers[0].link.scan_lanes = ScanLanes::Mono;
    set_actions(&mut runner, 0, ON_HOLD_OFF);
    grid_tap(&mut runner, 0, 4);
    grid_tap(&mut runner, 1, 4);
    let chord = step(&mut runner);
    assert_eq!(chord.len(), 2, "{chord:?}");
    let next = step(&mut runner);
    assert_eq!(
        next.len(),
        4,
        "each lane strikes its own note again: {next:?}"
    );
    assert_eq!(runner.held_keys.held_count(), 2);
}

#[test]
fn a_scan_dir_flip_keeps_every_note_and_the_mono_lane_goes_on_replacing() {
    for lanes in [ScanLanes::Mono, ScanLanes::Poly] {
        let mut runner = melody(&[0, 1, 2, 3, 4, 5, 6, 7], lanes);
        step(&mut runner);
        let before = step(&mut runner);
        let last = before.last().unwrap().clone();
        runner.menu.rebuild(runner.menu_config());
        runner
            .edit_menu_key("layers.0.link.scanDirection", 1)
            .unwrap();
        let flip = words_of(&runner.messages_without_snapshot().unwrap());
        assert!(flip.is_empty(), "{lanes:?}: {flip:?}");
        let after = step(&mut runner);
        let expected_off = usize::from(lanes == ScanLanes::Mono);
        assert_eq!(after.len(), 1 + expected_off, "{lanes:?}: {after:?}");
        if lanes == ScanLanes::Mono {
            assert_eq!(after[0], last.replace('+', "-"), "{after:?}");
        }
    }
}

#[test]
fn a_new_scan_axis_releases_everything_and_the_line_starts_fresh() {
    let mut runner = melody(&[0, 1, 2, 3, 4, 5, 6, 7], ScanLanes::Poly);
    let off = hold_key_on_layer_one(&mut runner);
    step(&mut runner);
    step(&mut runner);
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .set_enum_value_for_key("layers.0.link.scanAxis", "columns"));
    runner
        .apply_or_schedule_menu_key("layers.0.link.scanAxis")
        .unwrap();
    let released = words_of(&runner.messages_without_snapshot().unwrap());
    assert!(released.contains(&off), "{released:?}");
    assert_eq!(runner.held_keys.held_count(), 0);
}

#[test]
fn a_sections_change_ends_only_the_mono_lanes_notes() {
    let mut runner = melody(&[0, 1, 2, 3, 4, 5, 6, 7], ScanLanes::Mono);
    let other = hold_key_on_layer_one(&mut runner);
    let played = step(&mut runner);
    runner.menu.rebuild(runner.menu_config());
    runner
        .edit_menu_key("layers.0.link.scanSections", 1)
        .unwrap();
    let ended = words_of(&runner.messages_without_snapshot().unwrap());
    assert_eq!(ended, [played[0].replace('+', "-")]);
    assert!(!ended.contains(&other));
    assert_eq!(runner.held_keys.held_count(), 1);
}

#[test]
fn scan_mode_off_releases_the_layers_notes() {
    let mut runner = melody(&[0, 1, 2, 3, 4, 5, 6, 7], ScanLanes::Mono);
    let played = step(&mut runner);
    runner.menu.rebuild(runner.menu_config());
    runner.edit_menu_key("layers.0.link.scanMode", -1).unwrap();
    let released = words_of(&runner.messages_without_snapshot().unwrap());
    assert_eq!(released, [played[0].replace('+', "-")]);
}

/// A CC on a scan setting acts as the menu edit does: Scan Dir keeps the notes, Scan Axis ends
/// them.
#[test]
fn a_cc_on_a_scan_setting_acts_as_its_menu_edit() {
    for (field, releases) in [("scanDirection", false), ("scanAxis", true)] {
        let mut runner = melody(&[0, 1, 2, 3, 4, 5, 6, 7], ScanLanes::Poly);
        runner.test_set_midi_enabled(true);
        let played = step(&mut runner);
        map_cc(&mut runner, 0, &format!("layers.0.link.{field}"));
        let messages = send_cc(&mut runner, 1, 127);
        let mut released = words_of(&messages);
        released.extend(words_of(&runner.messages_without_snapshot().unwrap()));
        assert_eq!(
            released.contains(&played[0].replace('+', "-")),
            releases,
            "{field}: {released:?}"
        );
    }
}

fn words_of(messages: &[RunnerMessage]) -> Vec<String> {
    super::key_model::words(messages, false)
}
