//! Walks every reachable menu screen with real device input in both glyph sets and collects
//! what does not fit, has no specific help or shows a raw value.

use super::screen_sweep_checks::{
    clip_failures, help_failure, selected_row_overflows, value_failure, walks_into, FIXTURE_NAMES,
};
use super::*;
use crate::native_menu::{NativeMenuAction, NativeMenuItem, NativeMenuValue};
use crate::oled_frame::{
    clip_log, render_oled_frame, OledAnimationInput, OledPresentationInput, OledPresentationMetrics,
};
use crate::{MidiPort, SampleEntry};
use std::collections::{BTreeSet, HashSet, VecDeque};

pub(super) struct Sweep {
    pub(super) runner: NativeRunner,
    font: &'static str,
    pub(super) seen_pages: HashSet<String>,
    pub(super) seen_screens: HashSet<String>,
    pub(super) failures: BTreeSet<String>,
}

fn entry(dir: &str, name: &str, is_dir: bool) -> SampleEntry {
    SampleEntry {
        name: name.into(),
        path: if dir.is_empty() {
            name.into()
        } else {
            format!("{dir}/{name}")
        },
        is_dir,
    }
}

fn sample_entries(dir: &str) -> Vec<SampleEntry> {
    if dir.ends_with("Empty") {
        return Vec::new();
    }
    if dir.is_empty() {
        return vec![
            entry(dir, "Drum", true),
            entry(dir, FIXTURE_NAMES[1], true),
            entry(dir, "Empty", true),
            entry(dir, "Kick2.wav", false),
            entry(dir, &format!("{}.wav", FIXTURE_NAMES[2]), false),
        ];
    }
    vec![
        entry(dir, "snare.wav", false),
        entry(dir, &format!("{}.wav", FIXTURE_NAMES[3]), false),
    ]
}

fn preset_names() -> RuntimeStoreResult {
    RuntimeStoreResult::ListPresetsResult {
        names: vec!["test.json".into(), format!("{}.json", FIXTURE_NAMES[0])],
    }
}

impl Sweep {
    pub(super) fn new(payload: &str, board: bool, font: &'static str) -> Self {
        let mut payload: Value = serde_json::from_str(payload).unwrap();
        payload["runtimeConfig"]["mainFont"] = json!(font);
        payload["runtimeConfig"]["smallFont"] = json!(font);
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            jack_audio_required: board,
            usb_data_role_available: board,
            ..NativeRunnerConfig::default()
        })
        .unwrap();
        runner.midi.enabled = true;
        runner.apply_config_payload(payload).unwrap();
        runner.skip_startup_splash();
        let mut sweep = Self {
            runner,
            font,
            seen_pages: HashSet::new(),
            seen_screens: HashSet::new(),
            failures: BTreeSet::new(),
        };
        sweep.send(HostMessage::RuntimeResult {
            result: preset_names(),
        });
        let ports = FIXTURE_NAMES[4..]
            .iter()
            .enumerate()
            .map(|(index, name)| MidiPort {
                id: format!("p{index}"),
                name: (*name).into(),
            })
            .collect::<Vec<_>>();
        sweep.send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: ports.clone(),
            },
        });
        sweep.send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListInputsResult { inputs: ports },
        });
        sweep.dev(json!({ "type": "other" }));
        sweep
    }

    fn send(&mut self, message: HostMessage) {
        let mut queue = VecDeque::from([message]);
        while let Some(message) = queue.pop_front() {
            let messages = self.runner.send(message).unwrap();
            for message in messages {
                let RunnerMessage::PlatformEffects { effects } = message else {
                    continue;
                };
                for effect in effects {
                    match effect {
                        RuntimePlatformEffect::SampleListRequest {
                            instrument_slot,
                            sample_slot,
                            dir,
                        } => queue.push_back(HostMessage::RuntimeResult {
                            result: RuntimeStoreResult::SampleListResult {
                                instrument_slot,
                                sample_slot,
                                entries: sample_entries(&dir),
                                dir,
                            },
                        }),
                        RuntimePlatformEffect::StoreListPresets => {
                            queue.push_back(HostMessage::RuntimeResult {
                                result: preset_names(),
                            })
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub(super) fn dev(&mut self, input: Value) {
        self.send(HostMessage::DeviceInput {
            input,
            request_snapshot: Some(true),
        });
    }

    pub(super) fn turn(&mut self, delta: i32) {
        self.dev(json!({ "type": "encoder_turn", "id": "main", "delta": delta }));
    }

    pub(super) fn press(&mut self) {
        self.dev(json!({ "type": "encoder_press", "id": "main" }));
    }

    pub(super) fn button(&mut self, kind: &str, pressed: bool) {
        self.dev(json!({ "type": kind, "pressed": pressed }));
    }

    pub(super) fn back(&mut self) {
        self.button("button_a", true);
        self.button("button_a", false);
    }

    pub(super) fn grid(&mut self, x: usize, y: usize) {
        self.dev(json!({ "type": "grid_press", "x": x, "y": y }));
        self.dev(json!({ "type": "grid_release", "x": x, "y": y }));
    }

    pub(super) fn with_fn(&mut self, input: Value) {
        self.button("button_fn", true);
        self.dev(input);
        self.button("button_fn", false);
    }

    pub(super) fn fail(&mut self, problem: String) {
        let title = self.presentation().display.title;
        self.failures
            .insert(format!("[{}] {title}: {problem}", self.font));
    }

    pub(super) fn cursor_item(&self) -> NativeMenuItem {
        let siblings = self.runner.menu.current_siblings();
        siblings[self.runner.menu.state.cursor.min(siblings.len() - 1)].clone()
    }

    fn presentation(&self) -> OledPresentationInput {
        let mut input = self
            .runner
            .capture_presentation_scene(false)
            .unwrap()
            .oled_presentation_input(OledPresentationMetrics::from_status(Some(0.1), false), None);
        input.animation = OledAnimationInput::default();
        input
    }

    /// The menu item behind each body row of a plain menu page.
    fn row_items(&self, input: &OledPresentationInput) -> Vec<Option<NativeMenuItem>> {
        let overlay = self.runner.display.help_popup.is_some()
            || self.runner.display.confirm_dialog.is_some();
        let offset = input
            .display
            .scroll
            .as_ref()
            .map_or(0, |scroll| scroll.offset);
        let siblings = self.runner.menu.current_siblings();
        (0..input.display.rows.len())
            .map(|row| {
                (!overlay)
                    .then(|| siblings.get(offset + row).cloned())
                    .flatten()
            })
            .collect()
    }

    /// Renders the screen once, records what does not fit and, when the selected row clips a
    /// name, checks that it scrolls.
    pub(super) fn capture(&mut self) {
        self.runner.reset_menu_scroll();
        let input = self.presentation();
        let key = format!("{:?}|{:?}", input.display, input.selected_row);
        if !self.seen_screens.insert(key) {
            return;
        }
        clip_log::start();
        render_oled_frame(&input);
        let clipped = clip_log::take();
        let items = self.row_items(&input);
        let refs = items.iter().map(Option::as_ref).collect::<Vec<_>>();
        for failure in clip_failures(&input, &refs, &clipped) {
            self.fail(failure);
        }
        if !selected_row_overflows(&input) {
            return;
        }
        if self.runner.menu.state.editing {
            self.fail("the row being edited does not fit".into());
        } else {
            self.check_scroll(&input);
        }
    }

    fn check_scroll(&mut self, before: &OledPresentationInput) {
        let row = |input: &OledPresentationInput| {
            input
                .selected_row
                .and_then(|row| input.display.rows.get(row).cloned().flatten())
                .map(|row| (row.label, row.value))
        };
        for _ in 0..12 {
            self.runner.advance_toast_state();
        }
        let after = self.presentation();
        let moved = row(before) != row(&after);
        self.runner.reset_menu_scroll();
        if !moved {
            self.fail(format!("selected name does not scroll: {:?}", row(before)));
        }
    }

    fn check_selected_row(&mut self, item: &NativeMenuItem) {
        let input = self.presentation();
        let row = input
            .selected_row
            .and_then(|row| input.display.rows.get(row).cloned().flatten());
        if let Some(problem) = row.and_then(|row| value_failure(item, &row)) {
            self.fail(problem);
        }
    }

    fn open_help(&mut self) {
        self.button("button_shift", true);
        self.with_fn(json!({ "type": "encoder_press", "id": "main" }));
        self.button("button_shift", false);
    }

    fn check_help(&mut self) {
        let target = self.runner.menu.current_help_target();
        if let Some(problem) = help_failure(target.as_ref()) {
            self.fail(problem);
            return;
        }
        self.open_help();
        if self.runner.display.help_popup.is_none() {
            self.fail(format!(
                "help did not open on `{}`",
                self.cursor_item().label
            ));
            return;
        }
        self.capture();
        self.turn(20);
        self.capture();
        self.press();
    }

    fn restore(&mut self, stack: &[usize], cursor: usize) {
        if self.runner.display.help_popup.is_some()
            || self.runner.display.confirm_dialog.is_some()
            || self.runner.display.full_screen_signal.is_some()
            || self.runner.display.system_info_modal.is_some()
        {
            self.back();
        }
        if self.runner.menu.state.editing {
            self.press();
        }
        if self.runner.menu.state.editing {
            self.back();
        }
        if self.runner.menu.state.stack != stack || self.runner.menu.state.cursor != cursor {
            self.runner.menu.state.stack = stack.to_vec();
            self.runner.menu.state.cursor = cursor;
            self.runner.menu.state.editing = false;
            self.dev(json!({ "type": "other" }));
        }
    }

    fn page_signature(&self) -> String {
        let labels = self
            .runner
            .menu
            .current_siblings()
            .iter()
            .map(|item| format!("{}~{}", item.label, item.value == NativeMenuValue::Group))
            .collect::<Vec<_>>();
        format!(
            "{:?}|{}",
            self.presentation().display.body_layout,
            labels.join("\u{1f}")
        )
    }

    pub(super) fn walk(&mut self) {
        if !self.seen_pages.insert(self.page_signature()) {
            return;
        }
        let count = self.runner.menu.current_siblings().len();
        self.turn(-(count as i32) - 4);
        let stack = self.runner.menu.state.stack.clone();
        for index in 0..count {
            self.runner.menu.state.cursor = index;
            self.dev(json!({ "type": "other" }));
            let item = self.cursor_item();
            if item.label.is_empty() {
                continue;
            }
            self.capture();
            self.check_selected_row(&item);
            self.check_help();
            self.restore(&stack, index);
            self.visit(&item, &stack, index);
            self.restore(&stack, index);
        }
    }

    fn visit(&mut self, item: &NativeMenuItem, stack: &[usize], index: usize) {
        let enters = item.value == NativeMenuValue::Group
            || matches!(item.value, NativeMenuValue::Enum { .. }) && !item.children.is_empty();
        match &item.value {
            _ if enters => {
                self.press();
                if self.runner.menu.state.stack.len() == stack.len() + 1 {
                    self.walk();
                    self.restore(stack, index);
                }
            }
            NativeMenuValue::Action(action) => self.action(action, &item.label, stack.len()),
            _ => {
                self.press();
                self.capture();
                self.check_selected_row(item);
            }
        }
    }

    fn action(&mut self, action: &NativeMenuAction, label: &str, depth: usize) {
        if self.runner.confirmation_for_action(action).is_some() {
            self.press();
            self.capture();
            self.turn(1);
            self.capture();
            return;
        }
        if walks_into(action, label) {
            self.press();
            self.capture();
            if self.runner.menu.state.stack.len() == depth + 1 {
                self.walk();
            } else if self.runner.display.help_popup.is_some() {
                self.turn(20);
                self.capture();
            }
        }
    }
}
