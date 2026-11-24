//! The fuzz runner's instruments and layers, its random changes, and what its sinks heard.

use super::super::*;
use std::collections::BTreeMap;

pub(super) const KEYS_LAYERS: usize = 4;
pub(super) const LOOPER: usize = 4;
pub(super) const SEQUENCER: usize = 5;

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

/// Slot 1 One-Shot Synth, 2 Hold Synth, 3 Hold Sampler with looped pads, 4 Drum, 5 Hold MIDI
/// Inst. on channel 6 and 6 One-Shot FM; the Sampler and Drum stamp most tiles with one of four
/// pads and leave the rightmost column unstamped. Layers 1-4 play Keys, 5 an overdubbing Looper
/// on slot 2 and 6 a two-page Sequencer scanning on slot 3.
pub(super) fn fuzz_runner() -> NativeRunner {
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
    for link in runner
        .layers
        .iter_mut()
        .take(SEQUENCER + 1)
        .map(|layer| &mut layer.link)
    {
        link.note_preview = true;
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
        if let Some(pending) = runner.pending.pending_menu_apply.as_mut() {
            pending.due_at = std::time::Instant::now();
            runner
                .apply_due_menu_key(std::time::Instant::now())
                .unwrap();
        }
    }
}

fn menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    runner.menu.rebuild(runner.menu_config());
    if runner.menu.set_number_value_for_key(key, value) {
        runner.apply_or_schedule_menu_key(key).unwrap();
        if let Some(pending) = runner.pending.pending_menu_apply.as_mut() {
            pending.due_at = std::time::Instant::now();
            runner
                .apply_due_menu_key(std::time::Instant::now())
                .unwrap();
        }
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

/// One random Link, instrument, gate or behavior change.
pub(super) fn change(runner: &mut NativeRunner, rng: &mut Rng, lit: &mut Frames) {
    let layer = rng.below(SEQUENCER as u64 + 1) as usize;
    let prefix = format!("layers.{layer}.link");
    match rng.below(16) {
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
        11 => link_setting(runner, rng, layer, "scanLanes", &["poly", "mono"]),
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
        _ => bind(
            runner,
            &format!("{prefix}.y.pitch.steps"),
            json!(rng.below(3)),
        ),
    }
}

/// What the sinks heard, one line per call that sounded anything, and how many starts of each
/// held note and looping pad have not ended yet.
#[derive(Default)]
pub(super) struct Sinks {
    pub(super) lines: Vec<String>,
    open: BTreeMap<String, usize>,
}

impl Sinks {
    pub(super) fn hear(&mut self, runner: &NativeRunner, at: &str, messages: &[RunnerMessage]) {
        let mut words = Vec::new();
        for message in messages {
            match message {
                RunnerMessage::MusicalEvents { events } => {
                    words.extend(events.iter().map(|event| self.note(event, "a")));
                }
                RunnerMessage::MidiEvents { events } => {
                    words.extend(events.iter().map(|event| self.note(event, "m")));
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
                            self.end(&voice);
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
                self.end(&format!("{lane}{channel}.{note}"));
                format!("-{lane}{channel}.{note}")
            }
            MusicalEvent::Cc { .. } => String::new(),
        }
    }

    /// One end ends one start; an end with no held start ended a One-Shot note.
    fn end(&mut self, key: &str) {
        if let Some(count) = self.open.get_mut(key) {
            *count = count.saturating_sub(1);
        }
    }

    pub(super) fn assert_all_ended(&self, seed: u64) {
        let open = self
            .open
            .iter()
            .filter(|(_, count)| **count > 0)
            .collect::<Vec<_>>();
        assert!(
            open.is_empty(),
            "seed {seed}: notes still sounding: {open:?}"
        );
    }
}
