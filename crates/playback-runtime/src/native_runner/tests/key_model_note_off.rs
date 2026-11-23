//! A Drum or Sampler slot's Note Off: `cut` lets any source's note-off end the tile's sound,
//! `ignore` lets it play out. A Drum starts at `ignore`, a Sampler at `cut`.

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

fn set_note_off(runner: &mut NativeRunner, kind: &str, value: &str) {
    let key = format!(
        "instruments.0.{}.noteOff",
        if kind == "drum" { "drum" } else { "sample" }
    );
    runner.menu.set_enum_value_for_key(&key, value);
    assert_eq!(
        runner.menu.value_for_key(&key).as_deref(),
        Some(value),
        "{key}"
    );
    runner.apply_or_schedule_menu_key(&key).unwrap();
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
    set_note_off(&mut runner, "drum", "cut");
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 1, "the Off fades the hit out");
}

#[test]
fn a_sampler_cuts_on_note_offs_until_set_to_ignore() {
    let mut runner = tile_runner("sampler");
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 1);
    set_note_off(&mut runner, "sampler", "ignore");
    let (_, release) = tap(&mut runner);
    assert_eq!(releases(&release), 0, "the sample plays out");
}

#[test]
fn a_notes_in_key_up_follows_the_drum_note_off() {
    let mut runner = tile_runner("drum");
    runner.test_set_midi_enabled(true);
    runner.midi.notes_in = true;
    let note =
        super::super::drum_config::drum_voice_midi_note(&runner.instruments[0].drum_config, 0);
    for (setting, expected) in [("ignore", 0), ("cut", 1)] {
        set_note_off(&mut runner, "drum", setting);
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
fn the_note_off_settings_save_with_the_patch() {
    let mut runner = tile_runner("drum");
    set_note_off(&mut runner, "drum", "cut");
    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["drum"]["noteOff"],
        "cut"
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][1]["sample"]["noteOff"],
        "cut"
    );
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.instruments[0].drum_config["noteOff"], "cut");
}
