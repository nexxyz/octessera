//! Seeded note lifetimes: random tile frames on four Keys layers, a Looper and a paged
//! Sequencer, with sparse Link, instrument, behavior and transport changes made through the menu,
//! bindings, the gate page and shortcut, Panic and Notes In. After every step a note held until
//! its release must still have something behind it; at the end every tile goes dark and the
//! queues run out, every key is released, every held note ended once per start, and none of it
//! depends on which layer is selected. Every layer keeps its Off row on note off, on the
//! instrument its other rows play.

mod setup;

use super::*;
use platform_core::DeviceInput;
use setup::{change, fuzz_runner, Frames, Rng, Sinks, KEYS_LAYERS, LOOPER, SEQUENCER};

const STEP: u32 = 24;
const STEPS: usize = 160;
/// Steps a held note may outlast everything that could still end it: a scan's slowest lap with
/// four sections and its Delay.
const GRACE_STEPS: usize = 10;
const SLOTS: [u8; 6] = [0, 1, 2, 3, 4, 5];

struct Run {
    runner: NativeRunner,
    sinks: Sinks,
    rng: Rng,
    lit: Frames,
    /// Notes In keys down, as (channel, note).
    keyboard: Vec<(u8, u8)>,
    /// Steps each slot has held notes with nothing behind them.
    unbacked: [usize; 6],
    clock: std::time::Instant,
}

impl Run {
    fn hear(&mut self, at: &str, messages: &[RunnerMessage]) {
        self.sinks.hear(&self.runner, at, messages);
    }

    fn input(&mut self, at: &str, input: Value) {
        let messages = device_input(&mut self.runner, input);
        self.hear(at, &messages);
    }

    fn press(&mut self, at: &str, layer: usize, input: DeviceInput) {
        let result = self.runner.layer_engine_input_result(layer, input).unwrap();
        let messages = self
            .runner
            .messages_with_layer_input_result(layer, result)
            .unwrap();
        self.hear(at, &messages);
    }

    fn pulse(&mut self, at: &str) {
        self.clock += std::time::Duration::from_micros(20_833);
        self.runner.test_set_display_time(self.clock);
        let messages = self
            .runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        self.hear(at, &messages);
    }

    fn midi_note(&mut self, at: &str, channel: u8, note: u8, velocity: u8) {
        let messages = self
            .runner
            .send_midi_note(crate::runtime::MidiNote {
                channel,
                note,
                velocity,
            })
            .unwrap();
        self.hear(at, &messages);
    }

    /// Moves each Keys layer's held tiles to a new frame now and then, fully dark a third of
    /// the time, on a few columns and rows only, so pitches and pads collide.
    fn frames(&mut self, step: usize) {
        for layer in 0..KEYS_LAYERS {
            if !self.rng.chance(50) {
                continue;
            }
            let dark = self.rng.chance(35);
            for index in 0..GRID_WIDTH * GRID_HEIGHT {
                let (x, y) = (index % GRID_WIDTH, index / GRID_WIDTH);
                let wanted = !dark && x % 3 != 1 && y < 4 && self.rng.chance(20);
                if wanted != self.lit[layer][index] {
                    self.lit[layer][index] = wanted;
                    let input = if wanted {
                        DeviceInput::GridPress { x, y }
                    } else {
                        DeviceInput::GridRelease { x, y }
                    };
                    self.press(&format!("{step}.{layer}"), layer, input);
                }
            }
        }
        for layer in [LOOPER, SEQUENCER] {
            if self.rng.chance(15) {
                let (x, y) = (self.rng.below(8) as usize, self.rng.below(4) as usize);
                let at = format!("{step}.{layer}");
                self.press(&at, layer, DeviceInput::GridPress { x, y });
                self.press(&at, layer, DeviceInput::GridRelease { x, y });
            }
        }
    }

    /// Stop, Pause, Panic, a One-Shot press while stopped, or a Notes In key and an echo.
    fn transport_or_midi(&mut self, step: usize) {
        let at = format!("{step}!");
        match self.rng.below(12) {
            0 => {
                let messages = self.runner.send(HostMessage::TransportStop).unwrap();
                self.hear(&at, &messages);
                let x = self.rng.below(8) as usize;
                self.press(&at, 0, DeviceInput::GridPress { x, y: 0 });
                self.lit[0][x] = true;
                self.clock += std::time::Duration::from_millis(400);
                self.runner.test_set_display_time(self.clock);
                self.input(&at, json!({ "type": "button_s", "pressed": true }));
            }
            1 => {
                self.input(&at, json!({ "type": "button_s", "pressed": true }));
                self.input(&at, json!({ "type": "button_s", "pressed": true }));
            }
            2 => {
                self.runner
                    .execute_platform_action(crate::native_menu::PlatformAction::MidiPanic)
                    .unwrap();
                self.keyboard.clear();
                let messages = self.runner.messages_without_snapshot().unwrap();
                self.hear(&at, &messages);
            }
            3 | 4 => {
                let key = (self.rng.below(6) as u8, 48 + self.rng.below(24) as u8);
                if let Some(index) = self.keyboard.iter().position(|down| *down == key) {
                    self.keyboard.swap_remove(index);
                    self.midi_note(&at, key.0, key.1, 0);
                } else {
                    self.keyboard.push(key);
                    self.midi_note(&at, key.0, key.1, 100);
                }
            }
            5 => {
                // The echo of a note-off this runtime sent: a key-up of a key nobody holds.
                let key = (self.rng.below(6) as u8, 48 + self.rng.below(24) as u8);
                if !self.keyboard.contains(&key) {
                    self.midi_note(&at, key.0, key.1, 0);
                }
            }
            6 => {
                let punch = platform_core::BehaviorActionInput {
                    action_type: "toggleMode".into(),
                };
                self.press(&at, LOOPER, DeviceInput::BehaviorAction(punch));
            }
            _ => {}
        }
    }

    /// Counts the steps each Hold slot keeps a note with nothing left that could end it: no
    /// layer playing it with a lit tile, a queued event, Arp keys or a mono lane note, and no
    /// Notes In key down on it.
    fn check_backing(&mut self, step: usize) {
        for slot in SLOTS {
            let held = self.runner.held_keys.held_until_released(slot);
            let backed = self.keyboard.iter().any(|(channel, _)| *channel == slot)
                || (0..LAYER_COUNT).any(|layer| self.backs(layer, slot));
            let counter = &mut self.unbacked[usize::from(slot)];
            *counter = if held > 0 && !backed { *counter + 1 } else { 0 };
            assert!(
                *counter <= GRACE_STEPS,
                "step {step}: slot {slot} holds {held} notes nothing can end"
            );
        }
    }

    fn backs(&self, index: usize, slot: u8) -> bool {
        let layer = &self.runner.layers[index];
        let link = &layer.link;
        let plays = [link.activate_slot, link.stable_slot, link.deactivate_slot]
            .contains(&usize::from(slot));
        plays
            && (layer.engine.model().unwrap().tiles.iter().any(|tile| *tile)
                || !self.runner.delayed_link_events[index].is_empty()
                || !self.runner.link_arp.keys[index].is_empty()
                || layer.mono_lanes.remembers_a_note())
    }
}

/// Plays `seed`'s script with `selected` as the selected layer and returns what the sinks heard
/// and what is left over.
fn play(seed: u64, selected: usize) -> (Sinks, usize) {
    let mut run = Run {
        runner: fuzz_runner(),
        sinks: Sinks::default(),
        rng: Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1),
        lit: [[false; GRID_WIDTH * GRID_HEIGHT]; KEYS_LAYERS],
        keyboard: Vec::new(),
        unbacked: [0; 6],
        clock: std::time::Instant::now(),
    };
    run.runner.test_set_display_time(run.clock);
    run.runner.select_active_layer(selected).unwrap();
    run.input("play", json!({ "type": "button_s", "pressed": true }));
    for step in 0..STEPS {
        run.frames(step);
        if run.rng.chance(25) {
            change(&mut run.runner, &mut run.rng, &mut run.lit);
            let messages = run.runner.messages_without_snapshot().unwrap();
            run.hear(&format!("{step}*"), &messages);
        }
        if run.rng.chance(8) {
            run.transport_or_midi(step);
        }
        for offset in 0..STEP {
            run.pulse(&format!("{step}+{offset}"));
        }
        run.check_backing(step);
    }
    finish(&mut run);
    let held = run.runner.held_keys.held_count();
    (run.sinks, held)
}

/// Every tile goes dark: the Keys layers let go, the Looper and Sequencer layers end, the
/// keyboard lets up, and the queues run out.
fn finish(run: &mut Run) {
    for layer in 0..KEYS_LAYERS {
        for index in 0..GRID_WIDTH * GRID_HEIGHT {
            if run.lit[layer][index] {
                let (x, y) = (index % GRID_WIDTH, index / GRID_WIDTH);
                run.press("dark", layer, DeviceInput::GridRelease { x, y });
            }
        }
    }
    for (channel, note) in std::mem::take(&mut run.keyboard) {
        run.midi_note("dark", channel, note, 0);
    }
    if run.runner.transport.transport != RuntimeTransportState::Playing {
        run.input("dark", json!({ "type": "button_s", "pressed": true }));
    }
    for layer in [LOOPER, SEQUENCER] {
        run.runner.select_layer_behavior(layer, "none").unwrap();
    }
    for offset in 0..16 * STEP {
        run.pulse(&format!("drain+{offset}"));
    }
}

#[test]
fn every_note_ends_once_its_tiles_go_dark_whichever_layer_is_selected() {
    for seed in 1..=12 {
        let (first, held) = play(seed, 0);
        assert_eq!(held, 0, "seed {seed}: keys still held");
        first.assert_all_ended(seed);
        for selected in 1..LAYER_COUNT.min(6) {
            let (other, _) = play(seed, selected);
            let difference = first
                .lines
                .iter()
                .zip(&other.lines)
                .position(|(a, b)| a != b);
            assert!(
                first.lines == other.lines,
                "seed {seed}: layer {selected} selected plays otherwise from {:?}: {:?} vs {:?}",
                difference,
                difference.map(|line| &first.lines[line]),
                difference.map(|line| &other.lines[line]),
            );
        }
    }
}
