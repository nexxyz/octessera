//! The sweep's route: a patch with every instrument type and a few behaviors set up through the
//! menu, an Aux bind, then every section, Play page and behavior page, in both glyph sets.

use super::screen_sweep::Sweep;
use super::*;
use crate::native_menu::NativeMenuValue;
use std::collections::BTreeSet;

const DESKTOP_DEFAULT: &str =
    include_str!("../../../../../../config/generated/desktop/default.json");
const PI_DEFAULT: &str = include_str!("../../../../../../config/generated/pi/default.json");

impl Sweep {
    fn select_row(&mut self, prefix: &str) {
        let count = self.runner.menu.current_siblings().len();
        let index = self
            .runner
            .menu
            .current_siblings()
            .iter()
            .position(|item| item.label.starts_with(prefix))
            .unwrap_or_else(|| {
                panic!(
                    "no row `{prefix}` in {:?}",
                    self.runner.test_menu_sibling_labels()
                )
            });
        self.turn(-(count as i32) - 4);
        self.turn(index as i32);
    }

    fn open(&mut self, prefix: &str) {
        self.select_row(prefix);
        self.press();
    }

    fn open_path(&mut self, path: &[&str]) {
        self.back_to_root();
        for prefix in path {
            self.open(prefix);
        }
    }

    fn back_to_root(&mut self) {
        for _ in 0..12 {
            if self.runner.test_menu_at_root() {
                return;
            }
            if self.runner.menu.state.editing {
                self.press();
            } else {
                self.back();
            }
        }
        panic!("menu did not return to the root");
    }

    fn set_enum(&mut self, prefix: &str, option: &str) {
        self.select_row(prefix);
        self.press();
        let NativeMenuValue::Enum { options, selected } = self.cursor_item().value else {
            panic!("`{prefix}` is no enum");
        };
        let target = options.iter().position(|value| value == option).unwrap();
        self.turn(target as i32 - selected as i32);
        self.press();
    }

    fn select_behavior(&mut self, layer: &str, behavior: &str) {
        self.open_path(&["Build", layer, "Beh."]);
        let label = platform_core::behavior_catalog()
            .iter()
            .find(|entry| entry.id == behavior)
            .unwrap()
            .label;
        for category in self.runner.test_menu_sibling_labels() {
            if !category.starts_with('[') {
                continue;
            }
            self.open(&category);
            if self
                .runner
                .test_menu_sibling_labels()
                .iter()
                .any(|row| row == label)
            {
                self.open(label);
                return;
            }
            self.back();
        }
        panic!("behavior {behavior} not found");
    }

    /// A Fn + Aux press binds the selected row; the Aux Mappings pickers must show it.
    fn bind_and_check_aux_mappings(&mut self) {
        self.open_path(&["Shape", "Instruments", "I1", "Synth", "Filter"]);
        self.select_row("Cutoff");
        self.with_fn(json!({ "type": "encoder_press", "id": "aux1" }));
        self.capture();
        self.open_path(&["Build", "L1"]);
        self.select_row("Spawn Random");
        self.with_fn(json!({ "type": "encoder_press", "id": "aux3" }));
        self.capture();
        for (aux, picker, bound) in [("Aux 1", "Trn", "Cutoff"), ("Aux 3", "Clk", "Spawn Random")] {
            self.open_path(&["Link", "Aux Mappings", aux]);
            let shown = self.runner.test_menu_sibling_labels();
            if !shown
                .iter()
                .any(|row| row.starts_with(picker) && row.contains(bound))
            {
                self.fail(format!(
                    "{aux} {picker} does not show the Fn+Aux bind: {shown:?}"
                ));
            }
        }
    }

    fn setup(&mut self) {
        self.open_path(&["Shape", "Instruments"]);
        for (slot, kind) in [("I5", "drum"), ("I6", "midi"), ("I7", "midi_drum")] {
            self.open(slot);
            self.set_enum("Type", kind);
            self.back();
        }
        for (layer, behavior) in [
            ("L5", "fractal_explorer"),
            ("L6", "breaks"),
            ("L7", "physarum"),
        ] {
            self.select_behavior(layer, behavior);
        }
        self.open_path(&["Shape", "FX Buses", "B1", "Slot 1"]);
        self.set_enum("Time Mode", "note");
        self.bind_and_check_aux_mappings();
    }

    fn play_pages(&mut self) {
        for shift in [false, true] {
            for y in 0..8 {
                self.back_to_root();
                if shift {
                    self.button("button_shift", true);
                }
                self.with_fn(json!({ "type": "grid_press", "x": 7, "y": y }));
                self.dev(json!({ "type": "grid_release", "x": 7, "y": y }));
                if shift {
                    self.button("button_shift", false);
                }
                self.capture();
                self.grid(2, 3);
                self.capture();
            }
        }
    }

    fn aux_toasts(&mut self) {
        self.back_to_root();
        for aux in ["aux1", "aux2", "aux3"] {
            self.dev(json!({ "type": "encoder_turn", "id": aux, "delta": 1 }));
            self.capture();
            self.dev(json!({ "type": "encoder_press", "id": aux }));
            self.capture();
        }
        self.with_fn(json!({ "type": "encoder_press", "id": "main" }));
        self.capture();
        self.turn(20);
        self.capture();
        self.press();
    }

    fn walk_sections(&mut self, sections: &[&str]) {
        for section in sections {
            self.open_path(&[section]);
            self.walk();
        }
    }

    fn walk_every_behavior(&mut self) {
        for behavior in platform_core::list_native_behavior_ids() {
            if *behavior == "none" {
                continue;
            }
            self.select_behavior("L8", behavior);
            self.open_path(&["Build", "L8"]);
            self.walk();
        }
    }
}

/// Every problem the walk finds in one glyph set, desktop then board System pages.
fn sweep_font(font: &'static str) -> BTreeSet<String> {
    let mut desktop = Sweep::new(DESKTOP_DEFAULT, false, font);
    desktop.setup();
    desktop.walk_sections(&["Build", "Link", "Shape", "Play", "System"]);
    desktop.play_pages();
    desktop.aux_toasts();
    desktop.walk_every_behavior();
    let mut board = Sweep::new(PI_DEFAULT, true, font);
    board.seen_pages = std::mem::take(&mut desktop.seen_pages);
    board.walk_sections(&["System"]);
    let screens = desktop.seen_screens.len() + board.seen_screens.len();
    assert!(screens > 3000, "the {font} walk saw only {screens} screens");
    desktop.failures.append(&mut board.failures);
    desktop.failures
}

pub(super) fn run() {
    let failures = ["octessera", "original"]
        .into_iter()
        .flat_map(sweep_font)
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "{} screen problems:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
