//! Scripted device scenarios for end-to-end tests.
//!
//! A [`ScenarioBuilder`] mirrors the device on its own `NativeRunner` so menu navigation can be
//! written as label paths and resolved into the exact encoder turns and presses a player would
//! make. Setup inputs run before B1, the start of the timed section. Timed inputs are cued either
//! on transport pulses (24 per beat, counted from Play at B1): a `before` input lands once pulse
//! `p - 1` has played, ahead of the step at `p`, an `after` input once pulse `p` has played; or in
//! seconds after B1, for parts played while the transport is stopped or restarted.
mod device_input;

use crate::{CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig};
pub use device_input::{back, button, grid, press_main, turn};
use serde_json::Value;

/// Pulses per 4/4 bar.
pub const BAR: u64 = 96;
/// Pulses a modifier is held ahead of the gesture it modifies, and after it.
const MOD_LEAD: u64 = 6;
const RELEASE_AFTER: u64 = 3;

#[derive(Clone, Debug)]
pub enum SetupStep {
    Input(Value),
    /// The OLED title the device must show at this point.
    Checkpoint {
        name: String,
        title: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    Pulse { pulse: u64, before: bool },
    Seconds(f64),
}

#[derive(Clone, Debug)]
pub struct TimedInput {
    pub cue: Cue,
    pub input: Value,
}

#[derive(Clone, Debug, Default)]
pub struct Scenario {
    pub setup: Vec<SetupStep>,
    pub perf: Vec<TimedInput>,
    /// Press Play at B1 (pulse cues count from it).
    pub play_at_b1: bool,
}

pub struct ScenarioBuilder {
    runner: NativeRunner,
    setup: Vec<SetupStep>,
    perf: Vec<TimedInput>,
    play_at_b1: bool,
}

impl ScenarioBuilder {
    /// Starts from a patch payload the way the desktop does on a fresh store.
    pub fn from_payload(config: NativeRunnerConfig, payload: Value) -> Result<Self, String> {
        let mut runner = NativeRunner::new(config)?;
        runner.apply_config_payload(payload)?;
        runner.skip_startup_splash();
        Ok(Self {
            runner,
            setup: Vec::new(),
            perf: Vec::new(),
            play_at_b1: true,
        })
    }

    pub fn runner(&self) -> &NativeRunner {
        &self.runner
    }

    pub fn input(&mut self, input: Value) {
        self.setup.push(SetupStep::Input(input.clone()));
        let _ = self.runner.send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        });
    }

    pub fn checkpoint(&mut self, name: &str) {
        let title = self.runner.test_display_title();
        self.setup.push(SetupStep::Checkpoint {
            name: name.into(),
            title,
        });
    }

    pub fn back_to_root(&mut self) {
        for _ in 0..8 {
            if self.runner.test_menu_at_root() && self.runner.test_active_play_mode() == "none" {
                return;
            }
            self.input(button("button_a", true));
            self.input(button("button_a", false));
        }
        assert!(
            self.runner.test_menu_at_root(),
            "menu did not return to root"
        );
    }

    /// Moves the cursor to the row labelled `wanted` (or starting with it).
    pub fn select(&mut self, wanted: &str) {
        let labels = self.runner.test_menu_sibling_labels();
        let target = labels
            .iter()
            .position(|label| label == wanted)
            .or_else(|| labels.iter().position(|label| label.starts_with(wanted)))
            .unwrap_or_else(|| panic!("no row {wanted:?} in {labels:?}"));
        for _ in 0..64 {
            let cursor = self.runner.test_menu_cursor();
            if cursor == target {
                return;
            }
            self.input(turn(if target > cursor { 1 } else { -1 }));
        }
        panic!("could not reach row {wanted:?}");
    }

    pub fn enter(&mut self, wanted: &str) {
        self.select(wanted);
        self.input(press_main());
    }

    /// Opens a menu path from the root, one row label (or label prefix) per level.
    pub fn open(&mut self, path: &[&str]) {
        self.back_to_root();
        for label in path {
            self.enter(label);
        }
    }

    /// Edits an enum or bool row until `key` reads `wanted` (or `wanted:` as a label prefix).
    pub fn set_value(&mut self, row: &str, key: &str, wanted: &str) {
        self.select(row);
        self.input(press_main());
        let matches = |value: &str| value == wanted || value.starts_with(&format!("{wanted}:"));
        for direction in [1, -1] {
            for _ in 0..64 {
                let value = self.runner.test_menu_value(key);
                if value.as_deref().is_some_and(matches) {
                    self.input(press_main());
                    return;
                }
                self.input(turn(direction));
                if self.runner.test_menu_value(key) == value {
                    break;
                }
            }
        }
        panic!("{key} never reached {wanted:?}");
    }

    /// Edits a number row to `target`: coarse turns, then Fn + turn for the fine rest, stopping
    /// at the nearest value the row's steps can reach.
    pub fn set_number(&mut self, row: &str, key: &str, target: i32) {
        self.select(row);
        self.input(press_main());
        let number = |builder: &Self| builder.runner.test_menu_number(key).unwrap();
        let direction = if target < number(self) { -1 } else { 1 };
        for _ in 0..200 {
            let value = number(self);
            if (direction < 0 && value <= target) || (direction > 0 && value >= target) {
                break;
            }
            self.input(turn(direction));
        }
        if number(self) != target {
            self.input(button("button_fn", true));
            for _ in 0..40 {
                let value = number(self);
                let step = if value > target { -1 } else { 1 };
                self.input(turn(step));
                let next = number(self);
                if next == target || next == value {
                    break;
                }
                if (next - target).signum() != (value - target).signum() {
                    if (next - target).abs() > (value - target).abs() {
                        self.input(turn(-step));
                    }
                    break;
                }
            }
            self.input(button("button_fn", false));
        }
        self.input(press_main());
    }

    /// Confirms an open confirmation dialog (its `Confirm` option).
    pub fn confirm(&mut self) {
        for _ in 0..4 {
            if self.lines_contain("> Confirm") {
                break;
            }
            self.input(turn(1));
        }
        assert!(self.lines_contain("> Confirm"), "no confirmation dialog");
        self.input(press_main());
    }

    pub fn confirm_if_asked(&mut self) {
        if self.lines_contain("> Cancel") {
            self.confirm();
        }
    }

    fn lines_contain(&self, text: &str) -> bool {
        self.runner
            .test_display_lines()
            .iter()
            .any(|line| line.contains(text))
    }

    /// Runs inputs now, while holding `mods` around them.
    pub fn with_mods_now(&mut self, mods: &[&str], inputs: &[Value]) {
        for kind in mods {
            self.input(button(kind, true));
        }
        for input in inputs {
            self.input(input.clone());
        }
        for kind in mods.iter().rev() {
            self.input(button(kind, false));
        }
    }

    /// Shift + Fn + left-column cell: toggles a layer's mute (trigger gate), now.
    pub fn toggle_mute_now(&mut self, layer: usize) {
        self.with_mods_now(
            &["button_shift", "button_fn"],
            &[grid(0, layer, true), grid(0, layer, false)],
        );
    }

    /// Fn + left-column cell: selects a layer, now.
    pub fn select_layer_now(&mut self, layer: usize) {
        self.with_mods_now(
            &["button_fn"],
            &[grid(0, layer, true), grid(0, layer, false)],
        );
    }

    pub fn tap_now(&mut self, x: usize, y: usize) {
        self.input(grid(x, y, true));
        self.input(grid(x, y, false));
    }

    /// Leaves the transport alone at B1; the scenario presses Play itself.
    pub fn without_play_at_b1(&mut self) {
        self.play_at_b1 = false;
    }

    pub fn at(&mut self, pulse: u64, before: bool, input: Value) {
        self.perf.push(TimedInput {
            cue: Cue::Pulse { pulse, before },
            input,
        });
    }

    pub fn at_seconds(&mut self, seconds: f64, input: Value) {
        self.perf.push(TimedInput {
            cue: Cue::Seconds(seconds),
            input,
        });
    }

    /// A gesture at `seconds` after B1, with `mods` held from a little before until after.
    pub fn gesture_at_seconds(&mut self, mods: &[&str], seconds: f64, inputs: &[Value]) {
        for kind in mods {
            self.at_seconds(seconds - 0.12, button(kind, true));
        }
        for input in inputs {
            self.at_seconds(seconds, input.clone());
        }
        for kind in mods.iter().rev() {
            self.at_seconds(seconds + 0.06, button(kind, false));
        }
    }

    /// A gesture at `pulse` with `mods` held from a little before until a little after.
    pub fn gesture_at(&mut self, mods: &[&str], pulse: u64, before: bool, inputs: &[Value]) {
        let lead = pulse.saturating_sub(MOD_LEAD);
        for kind in mods {
            self.at(lead, true, button(kind, true));
        }
        for input in inputs {
            self.at(pulse, before, input.clone());
        }
        for kind in mods.iter().rev() {
            self.at(pulse + RELEASE_AFTER, false, button(kind, false));
        }
    }

    pub fn toggle_mute_at(&mut self, layer: usize, pulse: u64) {
        self.gesture_at(
            &["button_shift", "button_fn"],
            pulse,
            true,
            &[grid(0, layer, true), grid(0, layer, false)],
        );
    }

    pub fn tap_at(&mut self, x: usize, y: usize, pulse: u64, before: bool) {
        self.at(pulse, before, grid(x, y, true));
        self.at(pulse + RELEASE_AFTER, false, grid(x, y, false));
    }

    /// Menu moves during the performance, two pulses apart; Back is a press and release.
    pub fn nav_at(&mut self, start: u64, inputs: &[Value]) {
        let mut pulse = start;
        for input in inputs {
            self.at(pulse, true, input.clone());
            if input["type"] == "button_a" {
                self.at(pulse, true, button("button_a", false));
            }
            pulse += 2;
        }
    }

    pub fn build(mut self) -> Scenario {
        let order = |cue: &Cue| match *cue {
            Cue::Pulse { pulse, before } => (pulse as f64, !before),
            Cue::Seconds(seconds) => (seconds, false),
        };
        self.perf.sort_by(|a, b| {
            let (a, b) = (order(&a.cue), order(&b.cue));
            a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))
        });
        Scenario {
            setup: self.setup,
            perf: self.perf,
            play_at_b1: self.play_at_b1,
        }
    }
}
