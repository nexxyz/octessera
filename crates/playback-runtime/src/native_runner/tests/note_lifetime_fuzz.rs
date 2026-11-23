//! Seeded note lifetimes: random tile frames on four layers and random Link, instrument and
//! behavior changes, then every tile dark and the queues run out. Every key is released by then,
//! every held note that reached a sink has ended, and none of it depends on which layer is
//! selected. Every layer keeps its Off row on note off, on the instrument its other rows play.

use super::*;
use platform_core::DeviceInput;
use std::collections::BTreeMap;

const LAYERS: usize = 4;
const STEP: u32 = 24;

struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// Slot 1 One-Shot Synth, 2 Hold Synth, 3 Hold Sampler with looped pads, 4 Drum, 5 Hold MIDI
/// Inst. on channel 6 and 6 One-Shot FM; the Sampler and Drum stamp most tiles with one of four
/// pads, and leave the rightmost column unstamped.
fn fuzz_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.test_set_midi_enabled(true);
    let mut payload = runner.config_payload();
    let instruments = &mut payload["runtimeConfig"]["instruments"];
    let stamps = |pad: &str| {
        (0..GRID_HEIGHT)
            .flat_map(|y| (0..GRID_WIDTH - 1).map(move |x| (x, y)))
            .map(|(x, y)| match pad {
                "voice" => json!({ "x": x, "y": y, "voice": (x + y) % 4 }),
                _ => json!({
                    "x": x, "y": y, "sampleSlot": (x * 3 + y) % 4, "tuneSemis": 0, "tuneCents": 0
                }),
            })
            .collect::<Vec<_>>()
    };
    for (slot, kind, mode) in [
        (0, "synth", "oneshot"),
        (1, "synth", "hold"),
        (2, "sampler", "hold"),
        (3, "drum", "oneshot"),
        (4, "midi", "hold"),
        (5, "fm", "oneshot"),
    ] {
        instruments[slot]["type"] = json!(kind);
        instruments[slot]["noteBehavior"] = json!(mode);
    }
    instruments[2]["sample"]["assignments"] = json!(stamps("sampleSlot"));
    for pad in 0..4 {
        instruments[2]["sample"]["slots"][pad]["loop"] = json!("forward");
    }
    instruments[3]["drum"]["assignments"] = json!(stamps("voice"));
    instruments[4]["midi"] = json!({ "channel": 5, "enabled": true, "velocity": 100 });
    runner.apply_config_payload(payload).unwrap();
    for layer in 0..LAYERS {
        let quantize = if layer == 2 { "immediate" } else { "step" };
        runner
            .replace_layer_engine_with_config(
                layer,
                NativeBehavior::Keys,
                json!({ "quantize": quantize }),
                None,
            )
            .unwrap();
        let link = &mut runner.layers[layer].link;
        set_rows(link, layer, TriggerAction::NoteOn, TriggerAction::None);
        link.note_preview = true;
    }
    runner.sync_engine_runtime_config();
    runner.skip_startup_splash();
    runner.send(HostMessage::TransportStop).unwrap();
    runner
}

fn set_rows(link: &mut NativeLinkLayer, slot: usize, on: TriggerAction, hold: TriggerAction) {
    link.activate_slot = slot;
    link.stable_slot = slot;
    link.deactivate_slot = slot;
    link.activate_action = on;
    link.stable_action = hold;
    link.deactivate_action = TriggerAction::NoteOff;
}

/// What the sinks heard, one line per call that sounded anything.
#[derive(Default)]
struct Sinks {
    lines: Vec<String>,
    /// Held notes and looping pads that started and have not ended yet.
    open: BTreeMap<String, usize>,
}

impl Sinks {
    fn hear(&mut self, runner: &NativeRunner, at: &str, messages: &[RunnerMessage]) {
        let mut words = Vec::new();
        for message in messages {
            match message {
                RunnerMessage::MusicalEvents { events } => {
                    for event in events {
                        words.push(self.note(event, "a"));
                    }
                }
                RunnerMessage::MidiEvents { events } => {
                    for event in events {
                        words.push(self.note(event, "m"));
                    }
                }
                RunnerMessage::TileHits { hits } => {
                    for hit in hits {
                        let voice = format!("t{}.{}.{}", hit.instrument_slot, hit.voice, hit.tile);
                        let holds = runner
                            .instruments
                            .get(usize::from(hit.instrument_slot))
                            .is_some_and(|slot| {
                                slot.kind == InstrumentType::Sampler
                                    && slot.note_behavior == NoteBehavior::Hold
                            });
                        if hit.velocity == 0 {
                            self.open.remove(&voice);
                        } else if holds {
                            *self.open.entry(voice.clone()).or_default() += 1;
                        }
                        words.push(format!("{voice}.{}", hit.velocity));
                    }
                }
                RunnerMessage::SequencedOffset { nanos } => words.push(format!("@{nanos}")),
                _ => {}
            }
        }
        if !words.is_empty() {
            self.lines.push(format!("{at}: {}", words.join(" ")));
        }
    }

    fn note(&mut self, event: &MusicalEvent, lane: &str) -> String {
        match *event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                if duration_ms.is_none() {
                    *self
                        .open
                        .entry(format!("{lane}{channel}.{note}"))
                        .or_default() += 1;
                }
                let length = duration_ms.map_or("h".to_string(), |ms| ms.to_string());
                format!("+{lane}{channel}.{note}.{velocity}.{length}")
            }
            MusicalEvent::NoteOff { channel, note } => {
                self.open.remove(&format!("{lane}{channel}.{note}"));
                format!("-{lane}{channel}.{note}")
            }
            MusicalEvent::Cc { .. } => String::new(),
        }
    }
}

/// Changes one Link setting of `layer` as the menu or a binding would.
fn edit_link(runner: &mut NativeRunner, layer: usize, edit: impl FnOnce(&mut NativeLinkLayer)) {
    let before = runner.layers[layer].link.clone();
    edit(&mut runner.layers[layer].link);
    runner.apply_link_change(layer, &before);
    if layer == runner.active_layer_index {
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
    }
}

type Frames = [[bool; GRID_WIDTH * GRID_HEIGHT]; LAYERS];

/// One random Link, instrument or behavior change.
fn change(runner: &mut NativeRunner, rng: &mut Rng, lit: &mut Frames) {
    let layer = rng.below(LAYERS as u64) as usize;
    match rng.below(15) {
        0 => {
            let slot = rng.below(6) as usize;
            let on = if rng.chance(80) {
                TriggerAction::NoteOn
            } else {
                TriggerAction::None
            };
            let hold = if rng.chance(30) {
                TriggerAction::NoteOn
            } else {
                TriggerAction::None
            };
            let delays = [rng.below(3), rng.below(3), rng.below(3), rng.below(2)];
            edit_link(runner, layer, |link| {
                set_rows(link, slot, on, hold);
                link.activate_timing.delay_steps = delays[0] as u8;
                link.stable_timing.delay_steps = delays[1] as u8;
                link.deactivate_timing.delay_steps = delays[2] as u8;
                link.activate_timing.retrigger_count = delays[3] as u8;
            });
        }
        1 => {
            let root = ["C", "D", "E", "F#", "A"][rng.below(5) as usize];
            edit_link(runner, layer, |link| link.root = root.into());
        }
        2 => edit_link(runner, layer, |link| {
            link.scan_mode = match link.scan_mode {
                ScanMode::None => ScanMode::Scanning,
                ScanMode::Scanning => ScanMode::None,
            };
        }),
        3 => edit_link(runner, layer, |link| {
            link.scan_axis = match link.scan_axis {
                ScanAxis::Columns => ScanAxis::Rows,
                ScanAxis::Rows => ScanAxis::Columns,
            };
        }),
        4 => edit_link(runner, layer, |link| {
            link.scan_direction = match link.scan_direction {
                ScanDirection::Forward => ScanDirection::Reverse,
                ScanDirection::Reverse => ScanDirection::Forward,
            };
        }),
        5 => {
            let sections = [1, 2, 4][rng.below(3) as usize];
            edit_link(runner, layer, |link| link.scan_sections = sections);
        }
        6 => {
            let mode =
                [ArpMode::None, ArpMode::Up, ArpMode::Down, ArpMode::Random][rng.below(4) as usize];
            let source = ArpSource::ALL[rng.below(2) as usize];
            edit_link(runner, layer, |link| {
                link.arp.mode = mode;
                link.arp.source = source;
            });
        }
        7 => {
            let gate = 20 + rng.below(80) as u8;
            edit_link(runner, layer, |link| link.arp.gate_pct = gate);
        }
        8 => edit_link(runner, layer, |link| {
            link.trigger_probability_mode = match link.trigger_probability_mode {
                TriggerProbabilityMode::Zero => TriggerProbabilityMode::Full,
                _ => TriggerProbabilityMode::Zero,
            };
        }),
        9 => {
            let slot = [0, 1, 2, 4, 5][rng.below(5) as usize];
            let key = format!("instruments.{slot}.noteBehavior");
            let mode = if runner.instruments[slot].note_behavior == NoteBehavior::Hold {
                "oneshot"
            } else {
                "hold"
            };
            runner.menu.rebuild(runner.menu_config());
            runner.menu.set_enum_value_for_key(&key, mode);
            assert_eq!(
                runner.menu.value_for_key(&key).as_deref(),
                Some(mode),
                "{key}"
            );
            runner.apply_or_schedule_menu_key(&key).unwrap();
        }
        10 => {
            let slot = [0, 2, 3, 4, 5][rng.below(5) as usize];
            let key = format!("instruments.{slot}.noteOff");
            let value = ["cut", "ignore"][rng.below(2) as usize];
            runner.menu.rebuild(runner.menu_config());
            // Hold instruments show no Note Off: their notes always end on a note-off.
            if runner.menu.set_enum_value_for_key(&key, value) {
                runner.apply_or_schedule_menu_key(&key).unwrap();
            }
        }
        11 => edit_link(runner, layer, |link| {
            link.scan_lanes = match link.scan_lanes {
                ScanLanes::Poly => ScanLanes::Mono,
                ScanLanes::Mono => ScanLanes::Poly,
            };
        }),
        12 => edit_link(runner, layer, |link| {
            link.note_preview = !link.note_preview;
        }),
        // A behavior pick takes its play style's Link setup, and back on Keys the grid starts
        // dark.
        14 => {
            runner.select_layer_behavior(layer, "sequencer").unwrap();
            runner.select_layer_behavior(layer, "keys").unwrap();
            lit[layer] = [false; GRID_WIDTH * GRID_HEIGHT];
        }
        _ => {
            let steps = rng.below(3) as i32;
            edit_link(runner, layer, |link| link.y_pitch_steps = steps);
        }
    }
}

fn press(runner: &mut NativeRunner, sinks: &mut Sinks, at: &str, layer: usize, input: DeviceInput) {
    let result = runner.layer_engine_input_result(layer, input).unwrap();
    let messages = runner
        .messages_with_layer_input_result(layer, result)
        .unwrap();
    sinks.hear(runner, at, &messages);
}

fn pulse(runner: &mut NativeRunner, sinks: &mut Sinks, at: &str) {
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    sinks.hear(runner, at, &messages);
}

/// Plays `seed`'s script with `selected` as the selected layer and returns what the sinks heard
/// and what is left over.
fn play(seed: u64, selected: usize) -> (Sinks, usize) {
    let mut runner = fuzz_runner();
    runner.select_active_layer(selected).unwrap();
    let mut sinks = Sinks::default();
    let play = device_input(&mut runner, json!({ "type": "button_s", "pressed": true }));
    sinks.hear(&runner, "play", &play);
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut lit: Frames = [[false; GRID_WIDTH * GRID_HEIGHT]; LAYERS];
    for step in 0..96 {
        for (layer, tiles) in lit.iter_mut().enumerate() {
            if !rng.chance(60) {
                continue;
            }
            for (index, tile) in tiles.iter_mut().enumerate() {
                // A few columns and rows only, so pitches and pads collide.
                let (x, y) = (index % GRID_WIDTH, index / GRID_WIDTH);
                let wanted = x % 3 != 1 && y < 4 && rng.chance(25);
                if wanted != *tile {
                    *tile = wanted;
                    let input = if wanted {
                        DeviceInput::GridPress { x, y }
                    } else {
                        DeviceInput::GridRelease { x, y }
                    };
                    press(
                        &mut runner,
                        &mut sinks,
                        &format!("{step}.{layer}"),
                        layer,
                        input,
                    );
                }
            }
        }
        if rng.chance(35) {
            change(&mut runner, &mut rng, &mut lit);
        }
        for offset in 0..STEP {
            if rng.chance(2) {
                change(&mut runner, &mut rng, &mut lit);
            }
            pulse(&mut runner, &mut sinks, &format!("{step}+{offset}"));
        }
    }
    for (layer, tiles) in lit.iter().enumerate() {
        for (index, _) in tiles.iter().enumerate().filter(|(_, lit)| **lit) {
            let (x, y) = (index % GRID_WIDTH, index / GRID_WIDTH);
            let input = DeviceInput::GridRelease { x, y };
            press(
                &mut runner,
                &mut sinks,
                &format!("dark.{layer}"),
                layer,
                input,
            );
        }
    }
    for offset in 0..16 * STEP {
        pulse(&mut runner, &mut sinks, &format!("drain+{offset}"));
    }
    let held = runner.held_keys.held_count();
    (sinks, held)
}

#[test]
fn every_note_ends_once_its_tiles_go_dark_whichever_layer_is_selected() {
    for seed in 1..=12 {
        let (first, held) = play(seed, 0);
        assert_eq!(held, 0, "seed {seed}: keys still held");
        assert!(
            first.open.is_empty(),
            "seed {seed}: notes still sounding: {:?}",
            first.open
        );
        for selected in 1..LAYERS {
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
