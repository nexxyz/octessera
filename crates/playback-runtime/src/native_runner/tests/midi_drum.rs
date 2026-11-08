use super::*;
use crate::native_runner::led_color::LedColor;
use crate::native_runner::{
    compose_local_system_patch_documents, split_local_system_patch_documents,
};
use crate::tests::support::FakeHost;
use crate::{HostMessage, PlaybackRuntime, RuntimeConfig};

const SNARE: u8 = 38;

struct Rig {
    runtime: PlaybackRuntime,
    runner: NativeRunner,
    host: FakeHost,
}

impl Rig {
    fn new(kind: &str) -> Self {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: "keys".into(),
            ..NativeRunnerConfig::default()
        })
        .unwrap();
        runner.midi_active = true;
        runner.selected_midi_output_id = Some("out-1".into());
        runner.transport.transport = RuntimeTransportState::Playing;
        runner.menu.rebuild(runner.menu_config());
        runner.refresh_active_mapping_config();
        runner.sync_engine_runtime_config();
        let mut rig = Self {
            runtime: PlaybackRuntime::new(RuntimeConfig::default()),
            runner,
            host: FakeHost::default(),
        };
        rig.input(json!({ "type": "other" }));
        let turns = [
            "synth",
            "sampler",
            "midi",
            "fm",
            "pluck",
            "drum",
            "midi_drum",
        ]
        .iter()
        .position(|option| *option == kind)
        .unwrap();
        if turns > 0 {
            rig.turn("instruments.0.type", turns as i32);
        }
        assert_eq!(rig.runner.instruments[0].kind.wire_name(), kind);
        rig.take_notes();
        rig
    }

    fn midi_drum() -> Self {
        let mut rig = Self::new("midi_drum");
        rig.turn("instruments.0.midiDrum.enabled", 1);
        assert!(rig.runner.instruments[0].midi_drum.settings.enabled);
        rig
    }

    fn input(&mut self, input: Value) {
        self.runtime
            .dispatch_host_message(
                HostMessage::DeviceInput {
                    input,
                    request_snapshot: None,
                },
                &mut self.runner,
                &mut self.host,
            )
            .unwrap();
    }

    /// Edits the row by `detents` fine steps: Main turns while Fn is held.
    fn turn(&mut self, key: &str, detents: i32) {
        assert!(self.runner.menu.focus_item_key(key), "{key}");
        self.runner.menu.state.editing = true;
        self.input(json!({ "type": "button_fn", "pressed": true }));
        self.input(json!({ "type": "encoder_turn", "delta": detents, "id": "main" }));
        self.input(json!({ "type": "button_fn", "pressed": false }));
        self.runner.menu.state.editing = false;
    }

    fn press_main_on(&mut self, key: &str) {
        assert!(self.runner.menu.focus_item_key(key), "{key}");
        self.input(json!({ "type": "encoder_press", "id": "main" }));
    }

    fn press(&mut self, x: usize, y: usize) {
        self.input(json!({ "type": "grid_press", "x": x, "y": y }));
        self.input(json!({ "type": "grid_release", "x": x, "y": y }));
    }

    fn press_with(&mut self, modifiers: &[&str], x: usize, y: usize) {
        for modifier in modifiers {
            self.input(json!({ "type": modifier, "pressed": true }));
        }
        self.press(x, y);
        for modifier in modifiers.iter().rev() {
            self.input(json!({ "type": modifier, "pressed": false }));
        }
    }

    fn toggle_mute(&mut self) {
        self.press_with(&["button_fn"], GRID_WIDTH - 1, 0);
        assert_eq!(
            self.runner.active_play_mode,
            ActivePlayMode::Page(PlayMode::Mix)
        );
        self.press(0, 0);
        self.press_with(&["button_fn"], 0, 0);
        assert_eq!(self.runner.active_play_mode, ActivePlayMode::None);
    }

    fn back(&mut self) {
        self.input(json!({ "type": "button_a", "pressed": true }));
        self.input(json!({ "type": "button_a", "pressed": false }));
    }

    fn assign(&mut self, note_turns: i32, velocity_turns: i32) {
        self.turn("instruments.0.midiDrum.tileNote", note_turns);
        self.turn("instruments.0.midiDrum.tileVelocity", velocity_turns);
        self.press_main_on("midiDrum.assign.0");
    }

    fn toast(&self) -> String {
        self.runner.display.toast.as_ref().unwrap().message.clone()
    }

    fn stamp(&self, x: usize, y: usize) -> Option<(u8, u8)> {
        self.runner.instruments[0]
            .midi_drum
            .assignments
            .iter()
            .find(|tile| (tile.x, tile.y) == (x, y))
            .map(|tile| (tile.note, tile.velocity))
    }

    fn take_notes(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.host.midi_messages)
            .into_iter()
            .filter(|message| matches!(message.first(), Some(status) if status & 0xE0 == 0x80))
            .collect()
    }

    fn advance(&mut self, ms: u64) {
        self.runtime
            .advance(ms, &mut self.runner, &mut self.host)
            .unwrap();
    }

    fn led(&self, x: usize, y: usize) -> Value {
        led_tiles(&self.runner.snapshot().unwrap())[display_index(x, y)].clone()
    }
}

fn rgb(color: LedColor) -> Value {
    let mut bytes = Vec::new();
    color.append_rgb(&mut bytes);
    json!({ "r": bytes[0], "g": bytes[1], "b": bytes[2] })
}

#[test]
fn type_picker_names_midi_inst_and_midi_drum_and_auto_labels_follow() {
    let mut rig = Rig::new("midi");
    assert!(rig.runner.menu.focus_item_key("instruments.0.type"));
    assert!(rig
        .runner
        .menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "  Type MIDI Inst."));
    assert_eq!(rig.runner.instruments[0].name, "MIDI Inst.");

    rig.turn("instruments.0.type", 4);
    assert_eq!(rig.runner.instruments[0].kind, InstrumentType::MidiDrum);
    assert!(rig.runner.menu.focus_item_key("instruments.0.type"));
    assert!(rig
        .runner
        .menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "  Type MIDI Drum"));
    assert_eq!(rig.runner.instruments[0].name, "MIDI Drum");
    let shown = |rig: &Rig, value: &str| {
        let snapshot = rig.runner.menu.snapshot();
        let row = snapshot.rows[snapshot.selected_row.unwrap()]
            .clone()
            .unwrap();
        assert_eq!((row.label.as_str(), row.value.as_str()), ("Note", value));
    };
    assert!(rig
        .runner
        .menu
        .focus_item_key("instruments.0.midiDrum.tileNote"));
    shown(&rig, "36 C2 Kick");
    rig.turn("instruments.0.midiDrum.tileNote", 6);
    shown(&rig, "42 F#2 Clsd Hat");
    rig.turn("instruments.0.midiDrum.tileNote", 48);
    shown(&rig, "90 F#6");
    assert!(!rig.runner.menu.focus_item_key("instruments.0.noteBehavior"));
}

#[test]
fn assign_stamps_tiles_rows_and_columns_and_the_same_stamp_clears() {
    let mut rig = Rig::midi_drum();
    rig.assign(2, -10);
    assert_eq!(rig.toast(), "N38 v90: press tiles to assign");

    rig.press(2, 3);
    assert_eq!(rig.stamp(2, 3), Some((SNARE, 90)));
    assert_eq!(rig.toast(), "N38 v90");
    rig.press(2, 3);
    assert_eq!(rig.stamp(2, 3), None);
    assert_eq!(rig.toast(), "Cleared");

    rig.press_with(&["button_shift"], 4, 1);
    for x in 0..GRID_WIDTH {
        assert_eq!(rig.stamp(x, 1), Some((SNARE, 90)), "row x={x}");
    }
    assert_eq!(rig.toast(), "row N38 v90");
    rig.press_with(&["button_shift"], 0, 1);
    assert!((0..GRID_WIDTH).all(|x| rig.stamp(x, 1).is_none()));
    assert_eq!(rig.toast(), "Cleared row");

    rig.press_with(&["button_shift", "button_fn"], 6, 5);
    for y in 0..GRID_HEIGHT {
        assert_eq!(rig.stamp(6, y), Some((SNARE, 90)), "col y={y}");
    }
    assert_eq!(rig.toast(), "col N38 v90");
    assert!(rig.runner.grid_assign.is_some());
    assert!(rig.take_notes().is_empty());

    rig.back();
    assert!(rig.runner.grid_assign.is_none());
    rig.press(6, 2);
    assert_eq!(rig.take_notes()[0], vec![0x99, SNARE, 90]);
}

#[test]
fn assign_mode_lights_the_current_stamp_bright_and_other_stamps_dim() {
    let mut rig = Rig::midi_drum();
    rig.assign(0, 0);
    rig.press(1, 1);
    rig.turn("instruments.0.midiDrum.tileNote", 6);
    rig.press(5, 6);

    assert_eq!(rig.led(5, 6), rgb(LedColor::WHITE));
    assert_eq!(rig.led(1, 1), rgb(LedColor::WHITE.dim(3)));
    assert_eq!(rig.led(3, 3), rgb(LedColor::BLACK));
}

#[test]
fn a_struck_tile_sends_its_stamp_on_the_channel_and_ends_it_after_duration() {
    let mut rig = Rig::midi_drum();
    rig.turn("instruments.0.midiDrum.channel", -7);
    rig.turn("instruments.0.midiDrum.durationMs", 3);
    let duration = rig.runner.instruments[0].midi_drum.settings.duration_ms;
    assert!(duration > 100);
    rig.assign(2, -10);
    rig.press(2, 3);
    rig.back();

    rig.press(2, 3);
    assert_eq!(rig.take_notes(), vec![vec![0x92, SNARE, 90]]);
    assert!(rig.host.musical_events.is_empty() && rig.host.tile_hits.is_empty());
    rig.advance(u64::from(duration) - 1);
    assert!(rig.take_notes().is_empty());
    rig.advance(1);
    assert_eq!(rig.take_notes(), vec![vec![0x82, SNARE, 0]]);

    rig.press(4, 4);
    rig.advance(u64::from(duration));
    assert!(
        rig.take_notes().is_empty(),
        "an unstamped tile sends nothing"
    );
}

#[test]
fn mute_disabled_output_and_a_closed_trigger_gate_block_midi_drum_notes() {
    let mut rig = Rig::midi_drum();
    rig.assign(2, -10);
    rig.press(2, 3);
    rig.back();

    rig.toggle_mute();
    assert!(rig.runner.instrument_muted(0));
    rig.press(2, 3);
    assert!(rig.take_notes().is_empty(), "muted");
    rig.toggle_mute();
    rig.press(2, 3);
    assert_eq!(rig.take_notes().len(), 1, "unmuted");

    rig.press_with(&["button_shift", "button_fn"], 0, 0);
    rig.press(2, 3);
    assert!(rig.take_notes().is_empty(), "trigger gate closed");
    rig.press_with(&["button_shift", "button_fn"], 0, 0);
    rig.press(2, 3);
    assert_eq!(rig.take_notes().len(), 1, "trigger gate open");

    let mode = std::mem::replace(
        &mut rig.runner.link_layers[0].trigger_probability_mode,
        TriggerProbabilityMode::Zero,
    );
    rig.press(2, 3);
    assert!(rig.take_notes().is_empty(), "zero probability");
    rig.runner.link_layers[0].trigger_probability_mode = mode;

    rig.turn("instruments.0.midiDrum.enabled", -1);
    rig.press(2, 3);
    assert!(rig.take_notes().is_empty(), "disabled");
}

#[test]
fn an_octave_spread_arp_never_re_pitches_or_re_lengthens_a_midi_drum_note() {
    let mut rig = Rig::midi_drum();
    rig.assign(2, -10);
    rig.press(2, 3);
    rig.back();
    let arp = &mut rig.runner.link_layers[0].arp;
    arp.mode = ArpMode::OctaveSpread;
    arp.octave_spread = 2;
    arp.note_length_ms = 400;

    rig.press(2, 3);
    for _ in 0..40 {
        rig.advance(50);
    }
    assert_eq!(
        rig.take_notes(),
        vec![vec![0x99, SNARE, 90], vec![0x89, SNARE, 0]]
    );
}

#[test]
fn changing_the_type_leaves_midi_drum_assign_mode() {
    let mut rig = Rig::midi_drum();
    rig.press_main_on("midiDrum.assign.0");
    assert!(rig.runner.grid_assign.is_some());
    rig.turn("instruments.0.type", -1);
    assert_eq!(rig.runner.instruments[0].kind, InstrumentType::Drum);
    assert!(rig.runner.grid_assign.is_none());
}

#[test]
fn midi_drum_stamps_and_settings_survive_a_patch_save_and_load() {
    let mut rig = Rig::midi_drum();
    rig.turn("instruments.0.midiDrum.channel", -2);
    rig.assign(2, -10);
    rig.press(2, 3);
    rig.press_with(&["button_shift"], 0, 7);
    let saved = rig.runner.instruments[0].midi_drum.clone();

    let documents = split_local_system_patch_documents(&rig.runner.config_payload()).unwrap();
    let block = &documents.patch["runtimeConfig"]["instruments"][0]["midiDrum"];
    assert_eq!(block["channel"], 8);
    assert_eq!(block["tileNote"], SNARE);
    assert_eq!(block["assignments"].as_array().unwrap().len(), 9);
    assert_eq!(
        block["assignments"][0],
        json!({ "x": 2, "y": 3, "note": SNARE, "velocity": 90 })
    );
    compose_local_system_patch_documents(&documents.system, &documents.patch).unwrap();

    let mut reloaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    reloaded
        .apply_store_result(RuntimeStoreResult::LoadDefaultResult {
            payload: Some(documents.patch.clone()),
        })
        .unwrap();
    assert_eq!(reloaded.instruments[0].kind, InstrumentType::MidiDrum);
    assert_eq!(reloaded.instruments[0].midi_drum, saved);

    let mut old_patch = documents.patch;
    old_patch["runtimeConfig"]["instruments"][3]
        .as_object_mut()
        .unwrap()
        .remove("midiDrum");
    let error = compose_local_system_patch_documents(&documents.system, &old_patch).unwrap_err();
    assert!(error.contains("instruments[3].midiDrum"), "{error}");
}

fn mirror_rig(kind: &str) -> Rig {
    let mut rig = Rig::new(kind);
    let instrument = &mut rig.runner.instruments[0];
    if kind == "drum" {
        instrument.drum_config["assignments"] = json!([{ "x": 2, "y": 3, "voice": 6 }]);
    } else {
        instrument.sample_assignments = vec![NativeSampleAssignment {
            x: 2,
            y: 3,
            sample_slot: 4,
            level: None,
            tune_semis: 7,
            tune_cents: 0,
        }];
    }
    rig
}

#[test]
fn drum_and_sampler_hits_go_to_midi_only_under_send_midi_all() {
    let struck = |rig: &Rig| {
        rig.host
            .tile_hits
            .iter()
            .filter(|hit| hit.velocity > 0)
            .map(|hit| hit.velocity)
            .collect::<Vec<_>>()
    };
    for (kind, note) in [("drum", 39), ("sampler", 40)] {
        let mut rig = mirror_rig(kind);
        rig.press(2, 3);
        assert_eq!(struck(&rig).len(), 1, "{kind}");
        assert!(rig.take_notes().is_empty(), "{kind} under MIDI inst.");

        rig.turn("midi.sendInstruments", 1);
        rig.press(2, 3);
        let velocity = struck(&rig)[1];
        assert_eq!(rig.take_notes(), vec![vec![0x90, note, velocity]], "{kind}");
        rig.advance(100);
        assert_eq!(rig.take_notes(), vec![vec![0x80, note, 0]], "{kind}");

        rig.toggle_mute();
        rig.press(2, 3);
        rig.advance(100);
        assert_eq!(struck(&rig).len(), 3, "{kind}");
        assert!(rig.take_notes().is_empty(), "{kind} muted");
        rig.toggle_mute();

        rig.turn("midi.sendInstruments", -1);
        rig.press(2, 3);
        rig.advance(100);
        assert_eq!(struck(&rig).len(), 4, "{kind}");
        assert!(rig.take_notes().is_empty(), "{kind} back on MIDI inst.");
    }
}

#[test]
fn a_drum_voice_sends_the_gm_note_of_its_current_sound() {
    let mut rig = mirror_rig("drum");
    rig.turn("midi.sendInstruments", 1);
    rig.turn("instruments.0.drum.voice", 6);
    rig.turn("instruments.0.drum.voices.6.sound", -6);
    assert_eq!(
        rig.runner.instruments[0].drum_config["voices"][6]["sound"],
        "kick"
    );
    rig.press(2, 3);
    assert_eq!(rig.take_notes()[0][..2], [0x90, 36]);
}
