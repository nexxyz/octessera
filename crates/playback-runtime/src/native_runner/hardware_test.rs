//! System > Maintenance > Hardware Test: a guided check of the grid LEDs and pads, the four keys,
//! the encoders and the audio path, driven entirely by the instrument's own controls.

use super::led_color::LedColor;
use super::snapshot_display::DisplaySnapshot;
use super::{display_index, NativeRunner, RuntimeTransportState, GRID_HEIGHT, GRID_WIDTH};
use crate::oled_frame::OledDisplayLayout;
use crate::protocol::RunnerMessage;
use platform_core::{DeviceInput, MusicalEvent};

const PADS: usize = GRID_WIDTH * GRID_HEIGHT;
const KEY_NAMES: [&str; 4] = ["Back", "Play", "Shift", "Fn"];
const KNOB_NAMES: [&str; 4] = ["Main", "Aux1", "Aux2", "Aux3"];
const KEY_COLORS: [LedColor; 4] = [
    LedColor::MAGENTA,
    LedColor::GREEN,
    LedColor::BLUE,
    LedColor::WHITE,
];
const TONE_NOTE: u8 = 69;
const TONE_MS: u32 = 800;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    GridLeds,
    Pads,
    Keys,
    Knobs,
    Sound,
    Summary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HardwareTestOutcome {
    Stay,
    Exit,
    PlayTone,
}

#[derive(Clone, Debug)]
pub(super) struct NativeHardwareTest {
    step: Step,
    pads: [bool; PADS],
    held_pads: [bool; PADS],
    keys: [bool; 4],
    turns: [bool; 4],
    clicks: [bool; 4],
    tone_slot: Option<u8>,
}

impl NativeHardwareTest {
    pub(super) fn new(tone_slot: Option<u8>) -> Self {
        Self {
            step: Step::GridLeds,
            pads: [false; PADS],
            held_pads: [false; PADS],
            keys: [false; 4],
            turns: [false; 4],
            clicks: [false; 4],
            tone_slot,
        }
    }

    fn handle_input(&mut self, input: &DeviceInput) -> HardwareTestOutcome {
        if let DeviceInput::EncoderPress { id } = input {
            let knob = knob_index(id.as_deref());
            if knob == Some(0) {
                return self.advance();
            }
            if self.step == Step::Knobs {
                if let Some(knob) = knob {
                    self.clicks[knob] = true;
                }
            }
            return HardwareTestOutcome::Stay;
        }
        match self.step {
            Step::Pads => self.record_pad(input),
            Step::Keys => self.record_key(input),
            Step::Knobs => {
                if let DeviceInput::EncoderTurn { delta, id } = input {
                    if let Some(knob) = knob_index(id.as_deref()).filter(|_| *delta != 0) {
                        self.turns[knob] = true;
                    }
                }
            }
            Step::Sound if matches!(input, DeviceInput::ButtonS { pressed } if pressed.unwrap_or(true)) =>
            {
                return HardwareTestOutcome::PlayTone;
            }
            _ => {}
        }
        if self.step != Step::Keys
            && matches!(input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true))
        {
            return HardwareTestOutcome::Exit;
        }
        HardwareTestOutcome::Stay
    }

    fn advance(&mut self) -> HardwareTestOutcome {
        self.step = match self.step {
            Step::GridLeds => Step::Pads,
            Step::Pads => Step::Keys,
            Step::Keys => Step::Knobs,
            Step::Knobs => Step::Sound,
            Step::Sound => Step::Summary,
            Step::Summary => return HardwareTestOutcome::Exit,
        };
        if self.step == Step::Sound {
            HardwareTestOutcome::PlayTone
        } else {
            HardwareTestOutcome::Stay
        }
    }

    fn record_pad(&mut self, input: &DeviceInput) {
        match *input {
            DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
                self.pads[y * GRID_WIDTH + x] = true;
                self.held_pads[y * GRID_WIDTH + x] = true;
            }
            DeviceInput::GridRelease { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
                self.held_pads[y * GRID_WIDTH + x] = false;
            }
            _ => {}
        }
    }

    fn record_key(&mut self, input: &DeviceInput) {
        let pressed = |pressed: &Option<bool>| pressed.unwrap_or(true);
        match input {
            DeviceInput::ButtonA { pressed: p } if pressed(p) => self.keys[0] = true,
            DeviceInput::ButtonS { pressed: p } if pressed(p) => self.keys[1] = true,
            DeviceInput::ButtonShift { pressed: p } if pressed(p) => self.keys[2] = true,
            DeviceInput::ButtonFn { pressed: p } if pressed(p) => self.keys[3] = true,
            DeviceInput::ButtonCombinedModifier { pressed: p } if pressed(p) => {
                self.keys[2] = true;
                self.keys[3] = true;
            }
            _ => {}
        }
    }

    pub(super) fn display(&self) -> DisplaySnapshot {
        let lines = self.lines();
        let line_count = lines.len();
        DisplaySnapshot {
            body_layout: OledDisplayLayout::Rows,
            title: self.title().into(),
            lines,
            colors: vec![platform_core::palette::WHITE_RGB565; line_count],
            bar_values: vec![None; line_count],
            full_lines: vec![None; line_count],
            scroll: None,
            selected_row: None,
        }
    }

    fn title(&self) -> &'static str {
        match self.step {
            Step::GridLeds => "Test 1/5: Grid LEDs",
            Step::Pads => "Test 2/5: Pads",
            Step::Keys => "Test 3/5: Keys",
            Step::Knobs => "Test 4/5: Encoders",
            Step::Sound => "Test 5/5: Sound",
            Step::Summary => "Hardware Test",
        }
    }

    fn lines(&self) -> Vec<String> {
        match self.step {
            Step::GridLeds => vec![
                "Every pad lit?".into(),
                "Corners: bottom-left".into(),
                "magenta, bottom-right".into(),
                "green, top-left blue,".into(),
                "top-right white.".into(),
                "Main: next".into(),
            ],
            Step::Pads => vec![
                "Press every pad.".into(),
                format!("Seen {}/{PADS}", self.pad_count()),
                "Main: next".into(),
            ],
            Step::Keys => {
                let mut lines = vec!["Press each key:".into()];
                lines.extend(
                    KEY_NAMES
                        .iter()
                        .zip(self.keys)
                        .map(|(name, seen)| format!("{name:<6}{}", if seen { "ok" } else { "-" })),
                );
                lines.push("Main: next".into());
                lines
            }
            Step::Knobs => {
                let mut lines = vec!["Turn each, click Aux:".into()];
                lines.extend((0..4).map(|knob| {
                    let click = if knob == 0 {
                        ""
                    } else if self.clicks[knob] {
                        " click ok"
                    } else {
                        " click -"
                    };
                    let turn = if self.turns[knob] { "ok" } else { "-" };
                    format!("{:<5}turn {turn}{click}", KNOB_NAMES[knob])
                }));
                lines.push("Main: next".into());
                lines
            }
            Step::Sound => match self.tone_slot {
                Some(slot) => vec![
                    format!("A440 on I{}.", slot + 1),
                    "Hear it?".into(),
                    "Play: again".into(),
                    "Main: next".into(),
                ],
                None => vec![
                    "No internal synth".into(),
                    "in this patch.".into(),
                    "Main: next".into(),
                ],
            },
            Step::Summary => vec![
                format!("Pads {}/{PADS}", self.pad_count()),
                format!("Keys {}/4", self.keys.iter().filter(|seen| **seen).count()),
                format!(
                    "Encoders {}/7",
                    self.turns
                        .iter()
                        .chain(&self.clicks[1..])
                        .filter(|seen| **seen)
                        .count()
                ),
                "Main: done".into(),
            ],
        }
    }

    fn pad_count(&self) -> usize {
        self.pads.iter().filter(|seen| **seen).count()
    }

    pub(super) fn paint_grid(&self, leds: &mut [LedColor]) {
        leds.fill(LedColor::BLACK);
        match self.step {
            Step::GridLeds => {
                leds.fill(LedColor::WHITE.dim(6));
                let corners = [
                    (0, 0, LedColor::MAGENTA),
                    (GRID_WIDTH - 1, 0, LedColor::GREEN),
                    (0, GRID_HEIGHT - 1, LedColor::BLUE),
                    (GRID_WIDTH - 1, GRID_HEIGHT - 1, LedColor::WHITE),
                ];
                for (x, y, color) in corners {
                    leds[display_index(x, y)] = color;
                }
            }
            Step::Pads => {
                for (index, seen) in self.pads.iter().enumerate() {
                    let color = if self.held_pads[index] {
                        LedColor::WHITE
                    } else if *seen {
                        LedColor::GREEN
                    } else {
                        continue;
                    };
                    leds[display_index(index % GRID_WIDTH, index / GRID_WIDTH)] = color;
                }
            }
            _ => {}
        }
    }

    pub(super) fn key_leds(&self) -> [LedColor; 4] {
        match self.step {
            Step::Keys => std::array::from_fn(|key| {
                if self.keys[key] {
                    LedColor::GREEN
                } else {
                    KEY_COLORS[key].dim(4)
                }
            }),
            _ => KEY_COLORS,
        }
    }
}

fn knob_index(id: Option<&str>) -> Option<usize> {
    match id.unwrap_or("main") {
        "main" => Some(0),
        "aux1" => Some(1),
        "aux2" => Some(2),
        "aux3" => Some(3),
        _ => None,
    }
}

impl NativeRunner {
    pub(super) fn open_hardware_test(&mut self) {
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.display.help_popup = None;
        let tone_slot = self
            .instruments
            .iter()
            .position(|instrument| matches!(instrument.kind.as_str(), "synth" | "fm" | "pluck"))
            .map(|slot| slot as u8);
        self.display.hardware_test = Some(NativeHardwareTest::new(tone_slot));
    }

    pub(super) fn handle_hardware_test_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        let Some(test) = self.display.hardware_test.as_mut() else {
            return self.messages_with_snapshot();
        };
        let outcome = test.handle_input(&input);
        let tone_slot = test.tone_slot;
        if outcome == HardwareTestOutcome::Exit {
            self.display.hardware_test = None;
        }
        let mut messages = self.messages_with_snapshot()?;
        if let (HardwareTestOutcome::PlayTone, Some(slot)) = (outcome, tone_slot) {
            messages.push(RunnerMessage::MusicalEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: slot,
                    note: TONE_NOTE,
                    velocity: 100,
                    duration_ms: Some(TONE_MS),
                }],
            });
        }
        Ok(messages)
    }
}
