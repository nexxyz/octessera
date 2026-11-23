//! A One-Shot instrument's Note Off: `cut` lets any source's note-off end its note early, `ignore`
//! lets the note play its length. Every instrument starts at `ignore`; a Hold note always ends on
//! a note-off.

use super::*;
use crate::runtime::MidiNote;

/// A Keys layer playing slot 1 as `kind` with tile (2, 0) stamped, On note on, Off note off.
fn tile_runner(kind: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    let instrument = &mut payload["runtimeConfig"]["instruments"][0];
    instrument["type"] = json!(kind);
    if kind == "drum" {
        instrument["drum"]["assignments"] = json!([{ "x": 2, "y": 0, "voice": 0 }]);
    } else {
        instrument["sample"]["assignments"] =
            json!([{ "x": 2, "y": 0, "sampleSlot": 0, "tuneSemis": 0, "tuneCents": 0 }]);
    }
    runner.apply_config_payload(payload).unwrap();
    runner.layers[0].link.deactivate_action = TriggerAction::NoteOff;
    runner.refresh_active_mapping_config();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.menu.rebuild(runner.menu_config());
    runner
}

fn set_note_off(runner: &mut NativeRunner, value: &str) {
    let key = "instruments.0.noteOff";
    runner.menu.rebuild(runner.menu_config());
    runner.menu.set_enum_value_for_key(key, value);
    assert_eq!(
        runner.menu.value_for_key(key).as_deref(),
        Some(value),
        "{key}"
    );
    runner.apply_or_schedule_menu_key(key).unwrap();
}

/// The tile releases (velocity 0) in `messages`.
fn releases(messages: &[RunnerMessage]) -> usize {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::TileHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .filter(|hit| hit.velocity == 0)
        .count()
}

fn tap(runner: &mut NativeRunner) -> (Vec<RunnerMessage>, Vec<RunnerMessage>) {
    let press = device_input(runner, json!({ "type": "grid_press", "x": 2, "y": 0 }));
    let release = device_input(runner, json!({ "type": "grid_release", "x": 2, "y": 0 }));
    (press, release)
}

#[test]
fn a_drum_ignores_note_offs_until_set_to_cut() {
    let mut runner = tile_runner("drum");
    let (press, release) = tap(&mut runner);
    assert_eq!(releases(&press), 0);
    assert_eq!(releases(&release), 0, "the hit plays its full length");
    set_note_off(&mut runner, "cut");
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 1, "the Off fades the hit out");
}

#[test]
fn a_one_shot_sampler_lets_a_sample_play_out_until_set_to_cut_and_a_hold_one_always_cuts() {
    let mut runner = tile_runner("sampler");
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 0, "the sample plays out");
    set_note_off(&mut runner, "cut");
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 1);
    set_note_off(&mut runner, "ignore");
    runner.menu.rebuild(runner.menu_config());
    runner
        .edit_menu_key("instruments.0.noteBehavior", 1)
        .unwrap();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.item_for_key("instruments.0.noteOff").is_none());
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 1, "a held sample ends on its release");
}

#[test]
fn a_notes_in_key_up_follows_the_drum_note_off() {
    let mut runner = tile_runner("drum");
    runner.test_set_midi_enabled(true);
    runner.midi.notes_in = true;
    let note =
        super::super::drum_config::drum_voice_midi_note(&runner.instruments[0].drum_config, 0);
    for (setting, expected) in [("ignore", 0), ("cut", 1)] {
        set_note_off(&mut runner, setting);
        runner
            .send_midi_note(MidiNote {
                channel: 0,
                note,
                velocity: 100,
            })
            .unwrap();
        let key_up = runner
            .send_midi_note(MidiNote {
                channel: 0,
                note,
                velocity: 0,
            })
            .unwrap();
        assert_eq!(releases(&key_up), expected, "{setting}");
    }
}

#[test]
fn the_note_off_setting_saves_with_the_patch() {
    let mut runner = tile_runner("drum");
    set_note_off(&mut runner, "cut");
    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["instruments"][0]["noteOff"], "cut");
    assert_eq!(
        payload["runtimeConfig"]["instruments"][1]["noteOff"],
        "ignore"
    );
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert!(restored.instruments[0].note_off_cuts);
}

/// With nothing playing, a One-Shot key still ends after its length by the host clock, so its
/// later release cuts nothing.
#[test]
fn a_one_shot_key_pressed_while_stopped_ends_after_its_length() {
    let mut runner = tile_runner("sampler");
    set_note_off(&mut runner, "cut");
    runner.transport.transport = RuntimeTransportState::Stopped;
    let start = std::time::Instant::now();
    runner.test_set_display_time(start);
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 1, "a quick release cuts");
    device_input(&mut runner, json!({ "type": "grid_press", "x": 2, "y": 0 }));
    runner.test_set_display_time(start + std::time::Duration::from_secs(1));
    let late = device_input(
        &mut runner,
        json!({ "type": "grid_release", "x": 2, "y": 0 }),
    );
    assert_eq!(releases(&late), 0, "the sample already played its length");
}
