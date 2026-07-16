use super::device_driver::DeviceDriver;

pub(super) struct VisibleMenuDriver<'a> {
    device: &'a mut DeviceDriver,
}

impl<'a> VisibleMenuDriver<'a> {
    pub(super) fn new(device: &'a mut DeviceDriver) -> Self {
        Self { device }
    }

    pub(super) fn open_group(&mut self, label: &str) {
        self.select_visible(label);
        self.device.press_main();
    }

    pub(super) fn open_group_unless_visible(&mut self, group_label: &str, visible_marker: &str) {
        if self.any_line_contains(visible_marker) {
            return;
        }
        self.open_group(group_label);
    }

    pub(super) fn activate_action(&mut self, label: &str) {
        self.select_visible(label);
        self.device.press_main();
    }

    pub(super) fn edit_number_by(&mut self, label: &str, delta: i32) {
        self.select_visible(label);
        self.device.press_main();
        self.device.turn_main(delta);
        self.device.press_main();
        self.ensure_not_editing(label);
    }

    pub(super) fn edit_enum_by(&mut self, label: &str, delta: i32) {
        self.select_visible(label);
        self.device.press_main();
        self.device.turn_main(delta);
        self.device.press_main();
        self.ensure_not_editing(label);
    }

    pub(super) fn edit_bool_to(&mut self, label: &str, value: &str) {
        self.select_visible(label);
        if self.selected_line_contains(value) {
            return;
        }
        self.device.press_main();
        for _ in 0..4 {
            if self
                .lines()
                .iter()
                .any(|line| line.trim_start().starts_with('*') && contains_label(line, value))
            {
                self.device.press_main();
                self.ensure_not_editing(label);
                return;
            }
            self.device.turn_main(1);
        }
        self.device
            .fail(&format!("could not edit `{label}` to `{value}`"));
    }

    pub(super) fn expect_visible_value(&mut self, label: &str, value: &str) {
        self.select_visible(label);
        if !self
            .lines()
            .iter()
            .any(|line| line.starts_with('>') && contains_label(line, value))
        {
            self.device
                .fail(&format!("expected `{label}` row to show `{value}`"));
        }
    }

    pub(super) fn edit_enum_to(&mut self, label: &str, value: &str) {
        self.select_visible(label);
        self.device.press_main();
        for attempt in 0..32 {
            if self
                .lines()
                .iter()
                .any(|line| line.trim_start().starts_with('*') && enum_option_matches(line, value))
            {
                self.device.press_main();
                self.ensure_not_editing(label);
                return;
            }
            self.device.turn_main(if attempt < 16 { -1 } else { 1 });
        }
        self.device
            .fail(&format!("could not edit `{label}` to `{value}`"));
    }

    pub(super) fn back(&mut self) {
        self.device.press_button("back");
    }

    pub(super) fn move_selection(&mut self, delta: i32) {
        self.device.turn_main(delta);
    }

    pub(super) fn edit_selected_enum_to(&mut self, value: &str) {
        self.device.press_main();
        for attempt in 0..32 {
            if self
                .lines()
                .iter()
                .any(|line| line.trim_start().starts_with('*') && enum_option_matches(line, value))
            {
                self.device.press_main();
                self.ensure_not_editing("selected row");
                return;
            }
            self.device.turn_main(if attempt < 16 { -1 } else { 1 });
        }
        self.device
            .fail(&format!("could not edit selected row to `{value}`"));
    }

    pub(super) fn confirm(&mut self, title: &str) {
        if self.device.snapshot()["display"]["title"].as_str() != Some(title) {
            self.device
                .fail(&format!("expected confirm title `{title}`"));
        }
        self.select_visible("Confirm");
        self.device.press_main();
    }

    pub(super) fn back_to_root(&mut self) {
        for _ in 0..8 {
            let lines = self.lines();
            if lines.iter().any(|line| line.contains("Build"))
                && lines.iter().any(|line| line.contains("System"))
            {
                return;
            }
            self.device.press_button("back");
        }
        self.device.fail("could not return to root menu");
    }

    pub(super) fn select_visible(&mut self, label: &str) {
        self.ensure_not_editing(label);
        for search_delta in [1, -1] {
            for _ in 0..96 {
                if self.selected_line_contains(label) {
                    return;
                }
                let delta = self.direction_toward(label).unwrap_or(search_delta);
                self.device.turn_main(delta);
            }
        }
        self.device
            .fail(&format!("could not select visible row `{label}`"));
    }

    fn ensure_not_editing(&mut self, label: &str) {
        for _ in 0..3 {
            if self.device.snapshot()["display"]["editing"] != true {
                return;
            }
            self.device.press_main();
        }
        self.device.fail(&format!(
            "could not exit edit mode before selecting `{label}`"
        ));
    }

    fn selected_line_contains(&self, label: &str) -> bool {
        self.lines()
            .iter()
            .any(|line| line.starts_with('>') && contains_label(&clean_line(line), label))
    }

    fn any_line_contains(&self, label: &str) -> bool {
        self.lines()
            .iter()
            .any(|line| contains_label(&clean_line(line), label))
    }

    fn direction_toward(&self, label: &str) -> Option<i32> {
        let lines = self.lines();
        let selected = lines.iter().position(|line| line.starts_with('>'));
        let target = lines
            .iter()
            .position(|line| contains_label(&clean_line(line), label));
        match (selected, target) {
            (Some(selected), Some(target)) if target < selected => Some(-1),
            (Some(_), Some(_)) => Some(1),
            _ => None,
        }
    }

    fn lines(&self) -> Vec<String> {
        self.device.snapshot()["display"]["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_string))
            .collect()
    }
}

fn clean_line(line: &str) -> String {
    line.trim_start_matches(['>', ' ', '!']).trim().to_string()
}

fn contains_label(line: &str, label: &str) -> bool {
    line.to_ascii_lowercase()
        .contains(&label.to_ascii_lowercase())
}

fn enum_option_matches(line: &str, value: &str) -> bool {
    clean_line(line)
        .trim_start_matches('*')
        .trim()
        .eq_ignore_ascii_case(value)
}
