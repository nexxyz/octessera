use super::*;
use crate::tests::support::FakeHost;
use crate::{HostMessage, PlaybackRuntime, RuntimeConfig};

const STEP: u32 = 24;

/// A Hold Synth on a Keys layer behind the runtime with Send MIDI `all`, writing down per action
/// the internal notes and their MIDI out copies.
struct Copies {
    runtime: PlaybackRuntime,
    runner: NativeRunner,
    host: FakeHost,
    lines: Vec<String>,
    pulse: u32,
}

impl Copies {
    fn new() -> Self {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: "keys".into(),
            ..NativeRunnerConfig::default()
        })
        .unwrap();
        runner.instruments[0].note_behavior = NoteBehavior::Hold;
        runner.test_set_midi_enabled(true);
        runner.midi.output_id = Some("out-1".into());
        runner.transport.transport = RuntimeTransportState::Playing;
        runner.menu.rebuild(runner.menu_config());
        runner.sync_engine_runtime_config();
        let mut copies = Self {
            runtime: PlaybackRuntime::new(RuntimeConfig::default()),
            runner,
            host: FakeHost::default(),
            lines: Vec::new(),
            pulse: 0,
        };
        copies.input(json!({ "type": "other" }));
        assert!(copies.runner.menu.focus_item_key("midi.sendInstruments"));
        copies.runner.menu.state.editing = true;
        copies.input(json!({ "type": "encoder_turn", "delta": 1, "id": "main" }));
        copies.runner.menu.state.editing = false;
        copies.lines.clear();
        copies
    }

    fn dispatch(&mut self, message: HostMessage) {
        self.runtime
            .dispatch_host_message(message, &mut self.runner, &mut self.host)
            .unwrap();
        let mut words = Vec::new();
        for event in self.host.musical_events.drain(..) {
            match event {
                MusicalEvent::NoteOn { note, .. } => words.push(format!("+{note}")),
                MusicalEvent::NoteOff { note, .. } => words.push(format!("-{note}")),
                MusicalEvent::Cc { .. } => {}
            }
        }
        for bytes in self.host.midi_messages.drain(..) {
            if matches!(bytes.first(), Some(status) if status & 0xE0 == 0x80) {
                let hex = bytes.iter().map(|byte| format!("{byte:02x}"));
                words.push(format!("m{}", hex.collect::<Vec<_>>().join("")));
            }
        }
        if !words.is_empty() {
            self.lines
                .push(format!("{}: {}", self.pulse, words.join(" ")));
        }
    }

    fn input(&mut self, input: Value) {
        self.dispatch(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        });
    }

    fn press(&mut self, x: usize, y: usize) {
        self.input(json!({ "type": "grid_press", "x": x, "y": y }));
    }

    fn release(&mut self, x: usize, y: usize) {
        self.input(json!({ "type": "grid_release", "x": x, "y": y }));
    }

    fn pulses(&mut self, count: u32) {
        for _ in 0..count {
            self.pulse += 1;
            self.dispatch(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            });
        }
    }
}

fn held_note_copies_timeline() -> String {
    let mut copies = Copies::new();
    copies.press(2, 3);
    copies.pulses(STEP);
    copies.press(5, 3);
    copies.press(4, 5);
    copies.pulses(STEP);
    copies.release(2, 3);
    copies.pulses(STEP);
    copies.runner.handle_play_transpose_grid_press(1, 5);
    copies.pulses(STEP);
    copies.release(5, 3);
    copies.press(1, 1);
    copies.pulses(STEP);
    copies.input(json!({ "type": "button_s", "pressed": true }));
    copies.pulses(STEP);
    copies.release(4, 5);
    copies.release(1, 1);
    copies.pulses(STEP);
    copies.lines.join("\n") + "\n"
}

/// Pins the MIDI out copies Send MIDI `all` makes of a Hold Synth's notes through a struck-again
/// key, a transpose and Stop: each copy starts and ends with its internal note.
#[test]
pub(crate) fn held_note_copies_play_their_pinned_timeline() {
    let actual = held_note_copies_timeline();
    let expected = include_str!("fixtures/held_note_copies_timeline.txt");
    assert!(actual == expected, "{actual}");
}
