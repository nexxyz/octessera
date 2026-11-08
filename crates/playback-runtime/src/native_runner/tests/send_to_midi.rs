use super::device_config_apply::{board_runner, reload_system_payload, save_setting_payload};
use super::*;
use crate::native_help::resolve_native_help;
use crate::tests::support::FakeHost;
use crate::{HostMessage, PlaybackRuntime, RuntimeConfig};

const SEND_TO_MIDI: &str = "midi.sendInstruments";
const MIDI_CHANNEL: u8 = 5;

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
        runner.instruments[0].kind = InstrumentType::from_wire_name(kind).unwrap();
        runner.instruments[0].note_behavior = NoteBehavior::Hold;
        runner.instruments[0].midi_enabled = kind == "midi";
        runner.instruments[0].midi_channel = MIDI_CHANNEL + 1;
        runner.link_layers[0].x_filter_cutoff.enabled = true;
        runner.midi_active = true;
        runner.midi_clock_out_enabled = true;
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
        rig.take_midi();
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

    fn press_tile(&mut self) {
        self.input(json!({ "type": "grid_press", "x": 2, "y": 3 }));
    }

    fn release_tile(&mut self) {
        self.input(json!({ "type": "grid_release", "x": 2, "y": 3 }));
    }

    fn toggle_mute(&mut self) {
        self.input(json!({ "type": "button_fn", "pressed": true }));
        self.input(json!({ "type": "grid_press", "x": GRID_WIDTH - 1, "y": 0 }));
        self.input(json!({ "type": "grid_release", "x": GRID_WIDTH - 1, "y": 0 }));
        self.input(json!({ "type": "button_fn", "pressed": false }));
        assert_eq!(
            self.runner.active_play_mode,
            ActivePlayMode::Page(PlayMode::Mix)
        );
        self.input(json!({ "type": "grid_press", "x": 0, "y": 0 }));
        self.input(json!({ "type": "grid_release", "x": 0, "y": 0 }));
        self.input(json!({ "type": "button_fn", "pressed": true }));
        self.input(json!({ "type": "grid_press", "x": 0, "y": 0 }));
        self.input(json!({ "type": "grid_release", "x": 0, "y": 0 }));
        self.input(json!({ "type": "button_fn", "pressed": false }));
        assert_eq!(self.runner.active_play_mode, ActivePlayMode::None);
    }

    fn turn_send_to_midi(&mut self, delta: i8) {
        assert!(self.runner.menu.focus_item_key(SEND_TO_MIDI));
        self.runner.menu.state.editing = true;
        self.input(json!({ "type": "encoder_turn", "delta": delta, "id": "main" }));
    }

    fn take_midi(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.host.midi_messages)
    }

    fn internal_note(&self) -> u8 {
        self.host
            .musical_events
            .iter()
            .find_map(|event| match event {
                MusicalEvent::NoteOn { note, .. } => Some(*note),
                _ => None,
            })
            .expect("the tile should play an internal note")
    }
}

fn notes(messages: &[Vec<u8>]) -> Vec<Vec<u8>> {
    messages
        .iter()
        .filter(|message| matches!(message.first(), Some(status) if status & 0xE0 == 0x80))
        .cloned()
        .collect()
}

#[test]
fn midi_inst_keeps_internal_notes_off_midi_out_but_sends_clock_and_cc() {
    let mut rig = Rig::new("synth");
    assert_eq!(rig.runtime.config().midi_out_internal_channels, 0);

    rig.press_tile();
    rig.release_tile();
    let sent = rig.take_midi();
    assert!(!rig.host.musical_events.is_empty());
    assert_eq!(notes(&sent), Vec::<Vec<u8>>::new());
    assert!(sent.iter().any(|message| message[..2] == [0xB0, 74]));

    rig.runtime
        .advance(500, &mut rig.runner, &mut rig.host)
        .unwrap();
    assert!(rig.take_midi().contains(&vec![0xF8]));
}

#[test]
fn midi_inst_still_sends_midi_instrument_notes() {
    let mut rig = Rig::new("midi");
    rig.press_tile();
    rig.release_tile();
    let sent = notes(&rig.take_midi());
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0][0], 0x90 | MIDI_CHANNEL);
    assert_eq!(sent[1], vec![0x80 | MIDI_CHANNEL, sent[0][1], 0]);
}

#[test]
fn all_inst_mirrors_internal_notes_on_their_slot_channel() {
    let mut rig = Rig::new("synth");
    rig.turn_send_to_midi(1);
    assert_eq!(rig.runner.midi_send_instruments, "all");
    assert_eq!(
        rig.runtime.config().midi_out_internal_channels,
        (1 << INSTRUMENT_COUNT) - 1
    );

    rig.press_tile();
    rig.release_tile();
    let note = rig.internal_note();
    let sent = notes(&rig.take_midi());
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0][..2], [0x90, note]);
    assert_eq!(sent[1], vec![0x80, note, 0]);
}

#[test]
fn switching_to_midi_inst_ends_each_held_mirrored_note_once() {
    let mut rig = Rig::new("synth");
    rig.turn_send_to_midi(1);
    rig.press_tile();
    let note = rig.internal_note();
    assert_eq!(notes(&rig.take_midi()).len(), 1);

    rig.turn_send_to_midi(-1);
    assert_eq!(rig.runner.midi_send_instruments, "midi");
    assert_eq!(notes(&rig.take_midi()), vec![vec![0x80, note, 0]]);

    rig.release_tile();
    assert_eq!(notes(&rig.take_midi()), Vec::<Vec<u8>>::new());
}

#[test]
fn muting_an_internal_slot_ends_its_held_copy_once_and_sends_no_new_ones() {
    let mut rig = Rig::new("synth");
    rig.turn_send_to_midi(1);
    rig.press_tile();
    let note = rig.internal_note();
    assert_eq!(notes(&rig.take_midi()).len(), 1);

    rig.toggle_mute();
    assert!(rig.runner.instrument_muted(0));
    assert_eq!(notes(&rig.take_midi()), vec![vec![0x80, note, 0]]);
    rig.release_tile();
    rig.press_tile();
    rig.release_tile();
    assert!(rig.host.musical_events.len() > 2);
    assert_eq!(notes(&rig.take_midi()), Vec::<Vec<u8>>::new());

    rig.toggle_mute();
    assert!(!rig.runner.instrument_muted(0));
    assert_eq!(notes(&rig.take_midi()), Vec::<Vec<u8>>::new());
    rig.press_tile();
    rig.release_tile();
    let sent = notes(&rig.take_midi());
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0][..2], [0x90, note]);
    assert_eq!(sent[1], vec![0x80, note, 0]);
}

#[test]
fn toggling_send_to_midi_leaves_held_midi_instrument_notes_alone() {
    let mut rig = Rig::new("midi");
    rig.press_tile();
    assert_eq!(notes(&rig.take_midi()).len(), 1);

    rig.turn_send_to_midi(1);
    rig.turn_send_to_midi(-1);
    assert_eq!(notes(&rig.take_midi()), Vec::<Vec<u8>>::new());

    rig.release_tile();
    let sent = notes(&rig.take_midi());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0][0], 0x80 | MIDI_CHANNEL);
}

#[test]
fn send_to_midi_row_shows_its_help_and_survives_a_system_save() {
    let mut runner = board_runner(false);
    assert!(runner.menu.focus_item_key(SEND_TO_MIDI));
    assert_eq!(runner.menu.current_label(), Some("Send MIDI"));
    let help = resolve_native_help(&runner.menu.current_help_target().unwrap()).unwrap();
    assert_eq!(help.title, "Send MIDI");

    let saved = save_setting_payload(&mut runner, SEND_TO_MIDI, 1, "send-to-midi-save");
    assert_eq!(saved["runtimeConfig"]["midi"]["sendInstruments"], "all");

    let reloaded = reload_system_payload(saved);
    assert_eq!(reloaded.midi_send_instruments, "all");
    assert_eq!(
        reloaded.menu.value_for_key(SEND_TO_MIDI).as_deref(),
        Some("all")
    );
}
