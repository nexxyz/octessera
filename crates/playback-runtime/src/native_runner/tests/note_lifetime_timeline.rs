use super::factory_timeline::push_line;
use super::*;

/// Plays scripted presses, menu turns and steps on one runner and writes down every pulse that
/// sounds, in the factory timeline's notation.
struct Script {
    runner: NativeRunner,
    lines: Vec<String>,
    pulse: u32,
}

impl Script {
    fn new(behavior_id: &str, setup: impl FnOnce(&mut NativeRunner)) -> Self {
        Self::with_config(
            NativeRunnerConfig {
                behavior_id: behavior_id.into(),
                ..NativeRunnerConfig::default()
            },
            setup,
        )
    }

    fn with_config(config: NativeRunnerConfig, setup: impl FnOnce(&mut NativeRunner)) -> Self {
        let mut runner = NativeRunner::new(config).unwrap();
        // A new runner gives every layer the first one's behavior; only the first one plays here.
        for layer in 1..LAYER_COUNT {
            runner.layers[layer].behavior_id = "none".into();
            runner
                .replace_layer_engine_with_config(layer, NativeBehavior::None, Value::Null, None)
                .unwrap();
        }
        runner.test_set_midi_enabled(true);
        setup(&mut runner);
        runner.sync_engine_runtime_config();
        runner.skip_startup_splash();
        runner.send(HostMessage::TransportStop).unwrap();
        let mut script = Self {
            runner,
            lines: Vec::new(),
            pulse: 0,
        };
        script.input(json!({ "type": "button_s", "pressed": true }));
        script
    }

    fn input(&mut self, input: Value) {
        let messages = device_input(&mut self.runner, input);
        push_line(&mut self.lines, self.pulse, &messages);
    }

    fn press(&mut self, x: usize, y: usize) {
        self.input(json!({ "type": "grid_press", "x": x, "y": y }));
    }

    fn release(&mut self, x: usize, y: usize) {
        self.input(json!({ "type": "grid_release", "x": x, "y": y }));
    }

    fn turn(&mut self, key: &str, delta: i8) {
        assert!(self.runner.menu.focus_item_key(key), "{key}");
        self.runner.menu.state.editing = true;
        self.input(json!({ "type": "encoder_turn", "delta": delta, "id": "main" }));
        self.runner.menu.state.editing = false;
    }

    fn set(&mut self, key: &str, value: &str) {
        assert!(self.runner.menu.set_enum_value_for_key(key, value), "{key}");
        self.runner.apply_or_schedule_menu_key(key).unwrap();
    }

    /// Turns layer 0's trigger gate with the combined modifier on its leftmost tile.
    fn toggle_gate(&mut self) {
        self.runner.display.ui.combined_modifier_held = true;
        self.press(0, 0);
        self.release(0, 0);
        self.runner.display.ui.combined_modifier_held = false;
    }

    fn pulses(&mut self, count: u32) {
        for _ in 0..count {
            self.pulse += 1;
            let messages = self
                .runner
                .send(HostMessage::TransportPulseStep {
                    pulses: 1,
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: Some(false),
                })
                .unwrap();
            push_line(&mut self.lines, self.pulse, &messages);
        }
    }

    fn finish(self, title: &str) -> String {
        format!("# {title}\n{}\n", self.lines.join("\n"))
    }
}

const STEP: u32 = 24;

/// Two tiles on one pitch, a third tile, a transpose and a Note Mode change while held, then a
/// trigger gate drain, on instrument 1 starting in `note_mode`.
fn presses(script: &mut Script) {
    script.press(2, 3);
    script.pulses(STEP / 2);
    script.press(5, 3);
    script.pulses(STEP);
    script.release(2, 3);
    script.pulses(STEP);
    script.release(5, 3);
    script.press(1, 2);
    script.press(3, 4);
    script.pulses(STEP);
    script.runner.handle_play_transpose_grid_press(1, 5);
    script.pulses(STEP);
    script.release(1, 2);
    script.press(4, 5);
    script.pulses(STEP);
}

fn note_mode_and_gate(script: &mut Script, turn: i8) {
    script.turn("instruments.0.noteBehavior", turn);
    script.pulses(STEP);
    script.release(3, 4);
    script.press(6, 6);
    script.pulses(STEP);
    script.turn("instruments.0.noteBehavior", -turn);
    script.press(2, 3);
    script.pulses(STEP);
    script.toggle_gate();
    script.pulses(STEP);
    script.release(2, 3);
    script.release(4, 5);
    script.release(6, 6);
    script.toggle_gate();
    script.press(2, 3);
    script.pulses(STEP);
    script.release(2, 3);
    script.pulses(STEP);
}

fn keys_on(title: &str, setup: impl FnOnce(&mut NativeRunner), hold: bool) -> String {
    let mut script = Script::new("keys", setup);
    if hold {
        script.turn("instruments.0.noteBehavior", 1);
    }
    presses(&mut script);
    note_mode_and_gate(&mut script, if hold { -1 } else { 1 });
    script.finish(title)
}

fn midi_slot(runner: &mut NativeRunner) {
    let slot = &mut runner.instruments[0];
    slot.kind = InstrumentType::Midi;
    slot.midi_enabled = true;
    slot.midi_channel = 3;
}

fn hold_sampler(runner: &mut NativeRunner) {
    runner.instruments[0].kind = InstrumentType::Sampler;
    runner.instruments[0].note_behavior = NoteBehavior::Hold;
    runner.instruments[0].sample_follow_transpose = true;
    runner.instruments[0].sample_assignments = [(1, 2), (2, 2), (3, 2)]
        .into_iter()
        .map(|(x, sample_slot)| NativeSampleAssignment {
            x,
            y: 0,
            sample_slot,
            level: None,
            tune_semis: x as i8,
            tune_cents: 0,
        })
        .collect();
}

fn sampler_tiles() -> String {
    let mut script = Script::new("keys", hold_sampler);
    script.press(1, 0);
    script.pulses(STEP);
    script.press(2, 0);
    script.pulses(STEP);
    script.release(1, 0);
    script.pulses(STEP);
    script.release(2, 0);
    script.pulses(STEP);
    script.press(3, 0);
    script.press(1, 0);
    script.pulses(STEP);
    script.runner.handle_play_transpose_grid_press(1, 5);
    script.pulses(STEP);
    script.release(3, 0);
    script.release(1, 0);
    script.press(2, 0);
    script.pulses(STEP);
    script.toggle_gate();
    script.pulses(STEP);
    script.finish("hold sampler, two tiles on one pitch")
}

fn held_arp(mode: &str, hold: bool) -> String {
    let mut script = Script::new("keys", |_| {});
    if hold {
        script.turn("instruments.0.noteBehavior", 1);
    }
    script.set("layers.0.link.arp.mode", mode);
    script.set("layers.0.link.arp.source", "held");
    for (x, y) in [(4, 3), (1, 1), (6, 5)] {
        script.press(x, y);
        script.pulses(STEP);
    }
    script.release(1, 1);
    script.pulses(STEP);
    script.press(2, 2);
    script.pulses(2 * STEP);
    script.runner.handle_play_transpose_grid_press(1, 5);
    script.press(3, 4);
    script.pulses(2 * STEP);
    for (x, y) in [(2, 2), (3, 4), (4, 3), (6, 5)] {
        script.release(x, y);
    }
    script.pulses(2 * STEP);
    let note_mode = if hold { "hold" } else { "one-shot" };
    script.finish(&format!("held arp {mode} on {note_mode}"))
}

fn life(title: &str, hold: bool, setup: impl FnOnce(&mut Script)) -> String {
    let mut script = Script::new("life", |_| {});
    if hold {
        script.turn("instruments.0.noteBehavior", 1);
    }
    setup(&mut script);
    script.pulses(16 * STEP);
    script.finish(title)
}

/// On plays the One-Shot instrument 1 and Hold Trig the Hold instrument 2, both into a held
/// Direct Arp, so the keys down on the Arp's list come from both rows.
fn mixed_held_arp() -> String {
    let mut script = Script::new("life", |runner| {
        runner.instruments[1].kind = InstrumentType::Synth;
    });
    script.turn("instruments.1.noteBehavior", 1);
    hold_trig(&mut script);
    script.runner.layers[0].link.stable_slot = 1;
    script.runner.refresh_active_mapping_config();
    script.set("layers.0.link.arp.mode", "direct");
    script.set("layers.0.link.arp.source", "held");
    script.pulses(16 * STEP);
    script.finish("life held arp direct on a one-shot On and a hold Hold Trig")
}

fn hold_trig(script: &mut Script) {
    script.set("layers.0.link.mapping.stable.action", "note_on");
}

fn scanning_held_arp(script: &mut Script) {
    script.set("layers.0.link.scanMode", "scanning");
    script.set("layers.0.link.arp.mode", "chord_strike");
    script.set("layers.0.link.arp.source", "held");
}

fn looper_on_hold() -> String {
    let config = NativeRunnerConfig {
        behavior_id: "looper".into(),
        behavior_config: json!({ "mode": "overdub", "lengthSteps": 4 }),
        ..NativeRunnerConfig::default()
    };
    let mut script = Script::with_config(config, |_| {});
    script.turn("instruments.0.noteBehavior", 1);
    script.press(2, 3);
    script.pulses(STEP);
    script.press(4, 5);
    script.pulses(STEP / 2);
    script.release(2, 3);
    script.pulses(STEP);
    script.release(4, 5);
    script.pulses(12 * STEP);
    script.finish("looper overdub on a hold synth")
}

fn stop_while_held(title: &str, setup: impl FnOnce(&mut NativeRunner)) -> String {
    let mut script = Script::new("keys", setup);
    script.turn("instruments.0.noteBehavior", 1);
    script.press(2, 3);
    script.press(5, 3);
    script.press(4, 5);
    script.pulses(STEP);
    script.input(json!({ "type": "button_s", "pressed": true }));
    script.pulses(STEP);
    script.release(2, 3);
    script.press(1, 1);
    script.input(json!({ "type": "button_s", "pressed": true }));
    script.pulses(STEP);
    script.release(5, 3);
    script.release(4, 5);
    script.release(1, 1);
    script.press(6, 6);
    script.pulses(STEP);
    script.release(6, 6);
    script.pulses(STEP);
    script.finish(title)
}

fn patch_load_while_held() -> String {
    let mut script = Script::new("keys", midi_slot);
    script.turn("instruments.0.noteBehavior", 1);
    script.press(2, 3);
    script.press(4, 5);
    script.pulses(STEP);
    let payload = config_with(&script.runner, json!({}));
    script.runner.apply_config_payload(payload).unwrap();
    script.pulses(STEP);
    script.release(2, 3);
    script.press(1, 1);
    script.pulses(STEP);
    script.release(4, 5);
    script.release(1, 1);
    script.pulses(STEP);
    script.finish("patch load while held on a hold MIDI Inst.")
}

pub(crate) fn note_lifetime_timeline() -> String {
    [
        keys_on("keys on a hold synth", |_| {}, true),
        keys_on("keys on a one-shot synth", |_| {}, false),
        keys_on("keys on a hold MIDI Inst.", midi_slot, true),
        keys_on("keys on a one-shot MIDI Inst.", midi_slot, false),
        sampler_tiles(),
        held_arp("direct", true),
        held_arp("up", true),
        held_arp("direct", false),
        life("life hold trig on hold", true, hold_trig),
        life("life hold trig on one-shot", false, hold_trig),
        life("life scanning held arp on hold", true, scanning_held_arp),
        life(
            "life scanning held arp on one-shot",
            false,
            scanning_held_arp,
        ),
        mixed_held_arp(),
        looper_on_hold(),
        stop_while_held("stop while held on a hold synth", |_| {}),
        stop_while_held("stop while held on a hold MIDI Inst.", midi_slot),
        patch_load_while_held(),
    ]
    .join("")
}

/// Pins when each held and one-shot note starts and ends, on internal and MIDI lanes, through
/// shared pitches, transposes, Note Mode changes, trigger gate drains, held Arps and scans.
#[test]
pub(crate) fn note_lifetimes_play_their_pinned_timeline() {
    let actual = note_lifetime_timeline();
    let expected = include_str!("fixtures/note_lifetime_timeline.txt");
    let first_difference = actual
        .lines()
        .zip(expected.lines())
        .position(|(actual, expected)| actual != expected);
    assert!(
        actual == expected,
        "first differing line {first_difference:?} of {} vs {}",
        actual.lines().count(),
        expected.lines().count()
    );
}
