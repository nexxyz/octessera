use super::{
    dance_fx_cell_id, dance_fx_type, display_index, momentary_fx_color, native_binding_from_spec,
    pan_marker_left_cell, scan_index_for_overlay, scan_section_count, trigger_gate_color, LedColor,
    NativeInstrumentSlot, NativeParamBinding, NativeRunner, GRID_HEIGHT, GRID_WIDTH,
    INSTRUMENT_COUNT, TOUCH_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    pub(super) fn apply_dance_overlay(&self, leds: &mut [LedColor]) {
        match self.active_dance_mode.as_str() {
            "mix" => self.apply_dance_mix_overlay(leds),
            "pan" => self.apply_dance_pan_overlay(leds),
            "fx" => self.apply_dance_fx_overlay(leds),
            "trigger-gate" => self.apply_dance_trigger_gate_overlay(leds),
            "xy" => self.apply_dance_xy_overlay(leds),
            _ => {}
        }
    }

    pub(super) fn apply_sample_assignment_overlay(&self, leds: &mut [LedColor]) {
        let Some((instrument_slot, selected_sample_slot)) = self.sample_assign else {
            return;
        };
        self.fill_leds(leds, LedColor::rgb(0, 0, 0));
        let Some(instrument) = self.instruments.get(instrument_slot) else {
            return;
        };
        for assignment in &instrument.sample_assignments {
            let color = if assignment.sample_slot == selected_sample_slot {
                match assignment.level.as_deref() {
                    Some("high") => LedColor::rgb(220, 0, 0),
                    Some("medium") => LedColor::rgb(220, 180, 0),
                    Some("low") => LedColor::rgb(0, 220, 0),
                    _ => LedColor::rgb(220, 220, 220),
                }
            } else {
                LedColor::rgb(70, 70, 70)
            };
            self.set_display_led(leds, assignment.x, assignment.y, color);
        }
    }

    pub(super) fn apply_trigger_probability_overlay(&self, leds: &mut [LedColor]) {
        let Some(part_index) = self.trigger_probability_assign else {
            return;
        };
        self.fill_leds(leds, LedColor::rgb(0, 0, 0));
        let Some(map) = self.trigger_probability_maps.get(part_index) else {
            return;
        };
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let color = match map.get(y * GRID_WIDTH + x).map(String::as_str) {
                    Some("low") => LedColor::rgb(220, 0, 0),
                    Some("high") => LedColor::rgb(220, 180, 0),
                    Some("full") => LedColor::rgb(0, 220, 0),
                    _ => LedColor::rgb(0, 0, 0),
                };
                self.set_display_led(leds, x, y, color);
            }
        }
    }

    pub(super) fn apply_param_mod_overlay(&self, leds: &mut [LedColor]) {
        if !self.param_mod_overlay_ready() {
            return;
        }
        let Some(mut highlighted) = self
            .menu
            .current_param_binding()
            .map(native_binding_from_spec)
        else {
            return;
        };
        if let Some(field) = highlighted.key.strip_prefix("behavior.") {
            highlighted.key = format!("parts.{}.l1.behaviorConfig.{field}", self.active_part_index);
        }
        let Some(param_mods) = self.param_mods.get(self.active_part_index) else {
            return;
        };
        let lane = LedColor::rgb(18, 18, 24);
        for x in 0..GRID_WIDTH {
            self.set_display_led(leds, x, 0, lane);
            self.set_display_led(leds, x, 1, lane);
        }
        for y in 0..GRID_HEIGHT {
            self.set_display_led(leds, 0, y, lane);
            self.set_display_led(leds, 1, y, lane);
        }
        if let Some(binding) = param_mods.x.first().and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "x", 0);
        }
        if let Some(binding) = param_mods.x.get(1).and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "x", 1);
        }
        if let Some(binding) = param_mods.y.first().and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "y", 0);
        }
        if let Some(binding) = param_mods.y.get(1).and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "y", 1);
        }
        self.set_display_led(leds, 0, 0, LedColor::rgb(255, 255, 255));
        self.set_display_led(leds, 1, 1, LedColor::rgb(255, 255, 255));
    }

    fn paint_param_mod_axis_slot(
        &self,
        leds: &mut [LedColor],
        binding: &NativeParamBinding,
        highlighted_key: &str,
        axis: &str,
        slot: usize,
    ) {
        let color = if binding.invert {
            LedColor::rgb(255, 0, 90)
        } else {
            LedColor::rgb(0, 255, 120)
        };
        let color = if binding.key == highlighted_key {
            color
        } else {
            color.dim(3)
        };
        if axis == "x" {
            for x in 0..GRID_WIDTH {
                self.set_display_led(leds, x, slot, color);
            }
        } else {
            for y in 0..GRID_HEIGHT {
                self.set_display_led(leds, slot, y, color);
            }
        }
    }

    pub(super) fn apply_scan_progress_overlay(&self, leds: &mut [LedColor]) {
        let Some(sense) = self.sense_parts.get(self.active_part_index) else {
            return;
        };
        if sense.scan_mode != "scanning" {
            return;
        }
        let reverse = sense.scan_direction == "reverse";
        if sense.scan_axis == "columns" {
            let sections = scan_section_count(sense.scan_sections, GRID_HEIGHT);
            if sections > 1 {
                let section_height = (GRID_HEIGHT / sections).max(1);
                let step =
                    scan_index_for_overlay(self.tick as usize, GRID_WIDTH * sections, reverse);
                let section = step / GRID_WIDTH;
                let x = step % GRID_WIDTH;
                let first_y = section * section_height;
                for dy in 0..section_height {
                    self.add_scan_overlay_led(leds, x, first_y + dy);
                }
            } else {
                let x = scan_index_for_overlay(self.tick as usize, GRID_WIDTH, reverse);
                for y in 0..GRID_HEIGHT {
                    self.add_scan_overlay_led(leds, x, y);
                }
            }
        } else {
            let sections = scan_section_count(sense.scan_sections, GRID_WIDTH);
            if sections > 1 {
                let section_width = (GRID_WIDTH / sections).max(1);
                let step =
                    scan_index_for_overlay(self.tick as usize, GRID_HEIGHT * sections, reverse);
                let section = step / GRID_HEIGHT;
                let y = step % GRID_HEIGHT;
                let first_x = section * section_width;
                for dx in 0..section_width {
                    self.add_scan_overlay_led(leds, first_x + dx, y);
                }
            } else {
                let y = scan_index_for_overlay(self.tick as usize, GRID_HEIGHT, reverse);
                for x in 0..GRID_WIDTH {
                    self.add_scan_overlay_led(leds, x, y);
                }
            }
        }
    }

    fn add_scan_overlay_led(&self, leds: &mut [LedColor], x: usize, y: usize) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let index = display_index(x, y);
        if let Some(cell) = leds.get_mut(index) {
            *cell = cell.add_dim_white(24);
        }
    }

    pub(super) fn set_display_led(
        &self,
        leds: &mut [LedColor],
        x: usize,
        y: usize,
        value: LedColor,
    ) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let index = display_index(x, y);
        if let Some(cell) = leds.get_mut(index) {
            *cell = value;
        }
    }

    fn fill_leds(&self, leds: &mut [LedColor], value: LedColor) {
        for cell in leds.iter_mut() {
            *cell = value;
        }
    }

    fn dim_leds(&self, leds: &mut [LedColor], divisor: u8) {
        for cell in leds.iter_mut() {
            *cell = cell.dim(divisor);
        }
    }

    fn dance_pan_target(&self, instrument: &NativeInstrumentSlot) -> (u8, LedColor) {
        if let Some(bus_index) = instrument
            .route
            .strip_prefix("fx_bus_")
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|value| value.checked_sub(1))
        {
            let pan = self
                .fx_buses
                .get(bus_index)
                .map(|bus| bus.pan_pos)
                .unwrap_or(instrument.pan_pos);
            let color = match bus_index {
                0 => LedColor::rgb(190, 80, 255),
                1 => LedColor::rgb(0, 210, 255),
                2 => LedColor::rgb(0, 230, 120),
                3 => LedColor::rgb(255, 160, 0),
                _ => LedColor::rgb(255, 255, 255),
            };
            return (pan, color);
        }
        (instrument.pan_pos, LedColor::rgb(255, 255, 255))
    }

    pub(super) fn apply_fn_overlay(&self, leds: &mut [LedColor]) {
        if !self.ui.fn_held {
            return;
        }
        if self.active_dance_mode != "none" {
            self.dim_dance_fn_overlay(leds);
        }
        self.paint_fn_part_column(leds);
        self.paint_fn_page_column(leds);
    }

    fn apply_dance_mix_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for x in 0..INSTRUMENT_COUNT.min(GRID_WIDTH) {
            let instrument = self.instruments.get(x);
            let volume = instrument.map(|inst| inst.volume).unwrap_or(0).min(100);
            let y = ((f32::from(volume) / 100.0) * (GRID_HEIGHT - 1) as f32).round() as usize;
            let color = if instrument.map(|inst| inst.kind.as_str()) == Some("none") {
                LedColor::rgb(0, 55, 22)
            } else {
                LedColor::rgb(0, 220, 90)
            };
            self.set_display_led(leds, x, y, color);
        }
    }

    fn apply_dance_pan_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for y in 0..INSTRUMENT_COUNT.min(GRID_HEIGHT) {
            let Some(instrument) = self.instruments.get(y) else {
                continue;
            };
            let (pan_pos, color) = self.dance_pan_target(instrument);
            let left = pan_marker_left_cell(pan_pos);
            let value = if instrument.kind == "none" {
                color.dim(4)
            } else {
                color
            };
            self.set_display_led(leds, left, y, value);
            self.set_display_led(leds, left + 1, y, value);
        }
    }

    fn apply_dance_fx_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for assignment in &self.dance_fx_assignments {
            let id = dance_fx_cell_id(assignment.x, assignment.y);
            let active = self
                .active_dance_fx
                .iter()
                .any(|(active_id, _)| active_id == &id);
            let fx_type = dance_fx_type(&assignment.config);
            let same_type_active = self
                .active_dance_fx
                .iter()
                .any(|(_, active_type)| active_type == fx_type);
            let limited = !active
                && (self.active_dance_fx.len() >= TOUCH_FX_MAX_CONCURRENT || same_type_active);
            let color = momentary_fx_color(fx_type);
            self.set_display_led(
                leds,
                assignment.x,
                assignment.y,
                if active {
                    color.add_dim_white(70)
                } else if limited {
                    color.dim(5)
                } else {
                    color
                },
            );
        }
    }

    fn apply_dance_trigger_gate_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for (row, mode) in self.trigger_gate_modes.iter().enumerate().take(GRID_HEIGHT) {
            for (x, candidate) in [(0, "zero"), (1, "custom"), (2, "full")] {
                let color = trigger_gate_color(candidate);
                self.set_display_led(
                    leds,
                    x,
                    row,
                    if mode == candidate {
                        color
                    } else {
                        color.dim(4)
                    },
                );
            }
        }
        self.set_display_led(leds, 5, 0, trigger_gate_color("zero"));
        self.set_display_led(leds, 6, 0, trigger_gate_color("custom"));
        self.set_display_led(leds, 7, 0, trigger_gate_color("full"));
    }

    fn apply_dance_xy_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        let x =
            (self.xy_touch.display_x.clamp(0.0, 1.0) * (GRID_WIDTH - 1) as f32).round() as usize;
        let y =
            (self.xy_touch.display_y.clamp(0.0, 1.0) * (GRID_HEIGHT - 1) as f32).round() as usize;
        let color = if self.xy_touch.active {
            LedColor::rgb(255, 255, 255)
        } else if self.xy_release == "sample-hold" {
            LedColor::rgb(80, 80, 80)
        } else {
            LedColor::rgb(48, 48, 48)
        };
        self.set_display_led(leds, x, y, color);
    }

    fn param_mod_overlay_ready(&self) -> bool {
        self.ui.shift_held
            && !self.ui.fn_held
            && self.active_dance_mode == "none"
            && self.sample_assign.is_none()
            && self.trigger_probability_assign.is_none()
            && self.dance_fx_assign.is_none()
    }

    fn dim_dance_fn_overlay(&self, leds: &mut [LedColor]) {
        for cell in leds.iter_mut() {
            *cell = cell.dim(4);
        }
    }

    fn paint_fn_part_column(&self, leds: &mut [LedColor]) {
        for row in 0..GRID_HEIGHT {
            let configured = self
                .part_behavior_ids
                .get(row)
                .map(|id| id != "none")
                .unwrap_or(false);
            let color = fn_part_color(
                configured,
                row == self.active_part_index,
                self.active_dance_mode != "none",
            );
            self.set_display_led(leds, 0, row, color);
        }
    }

    fn paint_fn_page_column(&self, leds: &mut [LedColor]) {
        let page_options = ["mix", "pan", "fx", "trigger-gate", "xy"];
        for (row, mode) in page_options.iter().enumerate() {
            let selected = self.active_dance_mode != "none" && self.active_dance_mode == *mode;
            let color = if selected {
                LedColor::rgb(0, 158, 158)
            } else {
                LedColor::rgb(0, 60, 60)
            };
            self.set_display_led(leds, GRID_WIDTH - 1, row, color);
        }
    }
}

fn fn_part_color(configured: bool, active: bool, dance_active: bool) -> LedColor {
    if dance_active {
        if configured {
            LedColor::rgb(0, 120, 0)
        } else {
            LedColor::rgb(0, 48, 23)
        }
    } else if active {
        LedColor::rgb(0, 191, 191)
    } else if configured {
        LedColor::rgb(0, 120, 0)
    } else {
        LedColor::rgb(0, 48, 23)
    }
}
