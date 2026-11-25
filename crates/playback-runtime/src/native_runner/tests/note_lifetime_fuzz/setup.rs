//! The fuzz runner's instruments and layers and its random changes.

use super::super::*;
use std::collections::BTreeMap;

pub(super) const KEYS_LAYERS: usize = 4;
pub(super) const LOOPER: usize = 4;
pub(super) const SEQUENCER: usize = 5;
pub(super) const LIFE: usize = 6;
pub(super) const WANDERER: usize = 7;
/// The wanderer's own Hold Synth slot, which no change or Notes In key reaches.
pub(super) const WANDERER_SLOT: usize = 6;

pub(super) type Frames = [[bool; GRID_WIDTH * GRID_HEIGHT]; KEYS_LAYERS];

pub(super) struct Rng(pub(super) u64);

impl Rng {
    pub(super) fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }

    pub(super) fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// Behaviors that animate their tiles on their own; each seed gives the last layer one.
const WANDERERS: [NativeBehavior; 12] = [
    NativeBehavior::Bounce,
    NativeBehavior::Bubbles,
    NativeBehavior::Ant,
    NativeBehavior::Boids,
    NativeBehavior::Orbit,
    NativeBehavior::Gravity,
    NativeBehavior::Twinkle,
    NativeBehavior::Raindrops,
    NativeBehavior::Pong,
    NativeBehavior::ForestFire,
    NativeBehavior::Brain,
    NativeBehavior::Cyclic,
];

/// Slot 1 One-Shot Synth, 2 Hold Synth, 3 Hold Sampler with looped pads, 4 Drum, 5 Hold MIDI
/// Inst. on channel 6, 6 One-Shot FM and 7 a Hold Synth only the wanderer plays; the Sampler and
/// Drum stamp most tiles with one of four pads and leave the rightmost column unstamped.
/// Layers 1-4 play Keys, 5 an overdubbing Looper on slot 2, 6 a two-page Sequencer scanning on
/// slot 3, 7 a dark, unspawning Life on slot 2 with Note Preview off, so its next step plays
/// what a press changed, and 8 a seed's self-animating behavior on slot 7. With `poly` the
/// Sequencer plays slot 2 in poly lanes with no pitch steps, so a scan's Off, from another tile
/// of its lane, ends the notes it started. Every layer is seeded, so each draws its own random
/// stream whichever layer is selected.
pub(super) fn fuzz_runner(poly: bool, seed: u64) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.test_set_midi_enabled(true);
    runner.midi.notes_in = true;
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
        (6, "synth", "hold"),
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
    for layer in &mut runner.layers {
        layer.seeded = true;
    }
    for layer in 0..KEYS_LAYERS {
        let quantize = if layer == 2 { "immediate" } else { "step" };
        let config = json!({ "quantize": quantize });
        new_behavior(&mut runner, layer, NativeBehavior::Keys, config);
        set_rows(&mut runner.layers[layer].link, layer, TriggerAction::NoteOn);
    }
    let looper = json!({ "mode": "overdub", "lengthSteps": 8 });
    new_behavior(&mut runner, LOOPER, NativeBehavior::Looper, looper);
    set_rows(&mut runner.layers[LOOPER].link, 1, TriggerAction::NoteOn);
    let pages = json!({ "pagesX": 2, "advanceSteps": 8 });
    new_behavior(&mut runner, SEQUENCER, NativeBehavior::Sequencer, pages);
    let link = &mut runner.layers[SEQUENCER].link;
    set_rows(link, 2, TriggerAction::NoteOn);
    link.scan_mode = ScanMode::Scanning;
    let dark = json!({ "cells": vec![false; GRID_WIDTH * GRID_HEIGHT], "randomCellsPerTick": 0 });
    new_behavior(&mut runner, LIFE, NativeBehavior::Life, dark);
    set_rows(&mut runner.layers[LIFE].link, 1, TriggerAction::NoteOn);
    let wanderer = WANDERERS[seed as usize % WANDERERS.len()];
    new_behavior(&mut runner, WANDERER, wanderer, json!({}));
    let rows = &mut runner.layers[WANDERER].link;
    set_rows(rows, WANDERER_SLOT, TriggerAction::NoteOn);
    for link in runner
        .layers
        .iter_mut()
        .take(WANDERER + 1)
        .map(|layer| &mut layer.link)
    {
        link.note_preview = true;
        // A poly scan's Off names the dark tile's own pitch and pad, so along a lane with Pitch
        // Steps or other pads it leaves notes sounding by design; mono lanes always close.
        link.scan_lanes = ScanLanes::Mono;
        link.seeded = true;
    }
    runner.layers[LIFE].link.note_preview = false;
    if poly {
        let link = &mut runner.layers[SEQUENCER].link;
        set_rows(link, 1, TriggerAction::NoteOn);
        link.scan_lanes = ScanLanes::Poly;
        link.x_pitch_enabled = false;
        link.y_pitch_enabled = false;
    }
    runner.sync_engine_runtime_config();
    runner.skip_startup_splash();
    runner.send(HostMessage::TransportStop).unwrap();
    runner.menu.rebuild(runner.menu_config());
    runner
}

fn new_behavior(runner: &mut NativeRunner, layer: usize, behavior: NativeBehavior, config: Value) {
    runner.layers[layer].behavior_id = behavior.id().into();
    runner
        .replace_layer_engine_with_config(layer, behavior, config, None)
        .unwrap();
}

fn set_rows(link: &mut NativeLinkLayer, slot: usize, on: TriggerAction) {
    link.activate_slot = slot;
    link.stable_slot = slot;
    link.deactivate_slot = slot;
    link.activate_action = on;
    link.stable_action = TriggerAction::None;
    link.deactivate_action = TriggerAction::NoteOff;
    link.activate_timing.delay_steps = 1;
}

/// Picks option `choice` (wrapped) of the menu row `key` and applies it as a menu edit does.
fn menu_pick(runner: &mut NativeRunner, key: &str, choice: u64) {
    runner.menu.rebuild(runner.menu_config());
    let Some(crate::native_menu::NativeMenuItem {
        value: crate::native_menu::NativeMenuValue::Enum { options, .. },
        ..
    }) = runner.menu.item_for_key(key)
    else {
        return;
    };
    let option = &options[choice as usize % options.len()];
    if runner.menu.set_enum_value_for_key(key, option) {
        runner.apply_or_schedule_menu_key(key).unwrap();
        assert!(runner.pending.pending_menu_apply.is_none(), "{key} waits");
    }
}

fn menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    runner.menu.rebuild(runner.menu_config());
    if runner.menu.set_number_value_for_key(key, value) {
        runner.apply_or_schedule_menu_key(key).unwrap();
        assert!(runner.pending.pending_menu_apply.is_none(), "{key} waits");
    }
}

/// Sets `key` as a CC, Aux or Fader binding does.
fn bind(runner: &mut NativeRunner, key: &str, value: Value) {
    let mut deltas = BTreeMap::new();
    if runner.apply_param_binding_value(key, value, &mut deltas)
        && key.starts_with(&format!("layers.{}.", runner.active_layer_index))
    {
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
    }
}

/// A Link setting through the menu or a binding, picked at random.
fn link_setting(
    runner: &mut NativeRunner,
    rng: &mut Rng,
    layer: usize,
    field: &str,
    options: &[&str],
) {
    let key = format!("layers.{layer}.link.{field}");
    let choice = rng.below(options.len() as u64);
    if rng.chance(50) {
        menu_pick(runner, &key, rng.below(16));
    } else {
        bind(runner, &key, json!(options[choice as usize]));
    }
}

fn assert_slot_released(runner: &NativeRunner, slot: usize, what: &str) {
    let held = runner.held_keys.held_until_released(slot as u8);
    assert_eq!(
        held, 0,
        "slot {slot} keeps {held} keys after a {what} change"
    );
}

/// One random change on one of the first six layers: Link, instrument, gate, behavior, Play
/// Transpose, tempo or One-Shot length; with `poly` the Sequencer keeps its instrument.
pub(super) fn change(runner: &mut NativeRunner, rng: &mut Rng, lit: &mut Frames, poly: bool) {
    let layer = rng.below(SEQUENCER as u64 + 1) as usize;
    let prefix = format!("layers.{layer}.link");
    let case = rng.below(23);
    match case {
        0 if poly && layer == SEQUENCER => {}
        0 => {
            let row = ["activate", "stable", "deactivate"][rng.below(3) as usize];
            let slot = rng.below(6);
            for event in ["activate", "stable", "deactivate"] {
                menu_pick(runner, &format!("{prefix}.mapping.{event}.slot"), slot + 1);
            }
            if row != "deactivate" {
                menu_pick(
                    runner,
                    &format!("{prefix}.mapping.{row}.action"),
                    rng.below(2) * 2,
                );
            }
            menu_number(
                runner,
                &format!("{prefix}.mapping.{row}.delaySteps"),
                rng.below(3) as i32,
            );
        }
        1 => link_setting(
            runner,
            rng,
            layer,
            "pitch.root",
            &["C", "D", "E", "F#", "A"],
        ),
        2 => link_setting(runner, rng, layer, "scanMode", &["none", "scanning"]),
        3 => link_setting(runner, rng, layer, "scanAxis", &["rows", "columns"]),
        4 => link_setting(runner, rng, layer, "scanDirection", &["forward", "reverse"]),
        5 => link_setting(runner, rng, layer, "scanSections", &["1", "2", "4"]),
        6 => link_setting(
            runner,
            rng,
            layer,
            "arp.mode",
            &["none", "up", "down", "random"],
        ),
        7 => link_setting(runner, rng, layer, "arp.source", &["simultaneous", "held"]),
        8 => match rng.below(3) {
            0 => runner.toggle_layer_trigger_gate(layer),
            1 => runner.handle_trigger_gate_grid_press(rng.below(3) as usize, layer),
            _ => link_setting(
                runner,
                rng,
                layer,
                "triggerProbabilityMode",
                &["zero", "full"],
            ),
        },
        9 => {
            let slot = [0, 1, 2, 4, 5][rng.below(5) as usize];
            menu_pick(
                runner,
                &format!("instruments.{slot}.noteBehavior"),
                rng.below(2),
            );
        }
        10 => {
            let slot = [0, 2, 3, 4, 5][rng.below(5) as usize];
            menu_pick(runner, &format!("instruments.{slot}.noteOff"), rng.below(2));
        }
        11 => link_setting(runner, rng, layer, "scanUnit", &["1/16", "1/8", "1/32"]),
        12 => {
            let key = format!("{prefix}.notePreview");
            let on = rng.chance(50);
            if rng.chance(50) {
                runner.menu.rebuild(runner.menu_config());
                if runner.menu.set_bool_value_for_key(&key, on) {
                    runner.apply_or_schedule_menu_key(&key).unwrap();
                }
            } else {
                bind(runner, &key, json!(on));
            }
        }
        13 => bind(
            runner,
            &format!("{prefix}.arp.gatePct"),
            json!(20 + rng.below(80)),
        ),
        // A behavior pick takes its play style's Link setup, and back on Keys the grid starts
        // dark.
        14 if layer < KEYS_LAYERS => {
            runner.select_layer_behavior(layer, "sequencer").unwrap();
            runner.select_layer_behavior(layer, "keys").unwrap();
            lit[layer] = [false; GRID_WIDTH * GRID_HEIGHT];
        }
        // An X binding moving the layer's Root with each step's tiles, or none.
        15 => {
            if rng.chance(50) {
                super::super::key_model_retarget::root_on_x(runner, layer);
            } else {
                runner.layers[layer].param_mods.x[0] = None;
            }
        }
        16 => bind(
            runner,
            &format!("{prefix}.y.pitch.steps"),
            json!(rng.below(3)),
        ),
        // Play Transpose, or a layer's Transpose toggle, with keys down.
        17 => runner.handle_play_transpose_grid_press(rng.below(8) as usize, rng.below(8) as usize),
        // A new instrument type on a slot, or a new MIDI channel on the MIDI Inst.: either ends
        // every key the slot holds.
        18 => {
            let slot = [0, 2, 3, 4, 5][rng.below(5) as usize];
            let before = runner.instruments[slot].kind;
            menu_pick(runner, &format!("instruments.{slot}.type"), rng.below(16));
            if runner.instruments[slot].kind != before {
                assert_slot_released(runner, slot, "type");
            }
        }
        19 => {
            let before = runner.instruments[4].midi_channel;
            menu_number(
                runner,
                "instruments.4.midi.channel",
                1 + rng.below(16) as i32,
            );
            if runner.instruments[4].midi_channel != before {
                assert_slot_released(runner, 4, "channel");
            }
        }
        20 => {
            let row = ["activate", "stable", "deactivate"][rng.below(3) as usize];
            let key = format!("{prefix}.mapping.{row}.retriggerCount");
            menu_number(runner, &key, rng.below(4) as i32);
        }
        21 => {
            let key = ["transport.swingA", "transport.swingB"][rng.below(2) as usize];
            menu_number(runner, key, 1 + rng.below(7) as i32);
        }
        // A new tempo or One-Shot length while One-Shot notes sound.
        _ => {
            if rng.chance(50) {
                menu_number(runner, "transport.bpm", 40 + rng.below(200) as i32);
            } else {
                bind(
                    runner,
                    &format!("{prefix}.oneShotLengthMs"),
                    json!(10 + rng.below(1990)),
                );
            }
        }
    }
}
