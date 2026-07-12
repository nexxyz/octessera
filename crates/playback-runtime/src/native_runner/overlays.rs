use super::{
    display_index, native_binding_from_spec, scan_index_for_overlay, scan_section_count, LedColor,
    NativeInstrumentSlot, NativeParamBinding, NativeRunner, GRID_HEIGHT, GRID_WIDTH,
};

impl NativeRunner {
    pub(super) fn apply_sparks_overlay(&self, leds: &mut [LedColor]) {
        match self.active_sparks_mode.as_str() {
            "mix" => self.apply_sparks_mix_overlay(leds),
            "pan" => self.apply_sparks_pan_overlay(leds),
            "fx" => self.apply_sparks_fx_overlay(leds),
            "trigger-gate" => self.apply_sparks_trigger_gate_overlay(leds),
            "transpose" => self.apply_sparks_transpose_overlay(leds),
            "xy" => self.apply_sparks_xy_overlay(leds),
            _ => {}
        }
    }

    pub(super) fn apply_sample_assignment_overlay(&self, leds: &mut [LedColor]) {
        let Some((instrument_slot, selected_sample_slot)) = self.sample_assign else {
            return;
        };
        self.fill_leds(leds, LedColor::BLACK);
        let Some(instrument) = self.instruments.get(instrument_slot) else {
            return;
        };
        for assignment in &instrument.sample_assignments {
            let color = if assignment.sample_slot == selected_sample_slot {
                match assignment.level.as_deref() {
                    Some("high") => LedColor::RED,
                    Some("medium") => LedColor::YELLOW,
                    Some("low") => LedColor::GREEN,
                    _ => LedColor::SYSTEM,
                }
            } else {
                LedColor::SYSTEM.dim(3)
            };
            self.set_display_led(leds, assignment.x, assignment.y, color);
        }
    }

    pub(super) fn apply_trigger_probability_overlay(&self, leds: &mut [LedColor]) {
        let Some(layer_index) = self.trigger_probability_assign else {
            return;
        };
        self.fill_leds(leds, LedColor::BLACK);
        let Some(map) = self.trigger_probability_maps.get(layer_index) else {
            return;
        };
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let color = match map.get(y * GRID_WIDTH + x).map(String::as_str) {
                    Some("low") => LedColor::RED,
                    Some("high") => LedColor::YELLOW,
                    Some("full") => LedColor::GREEN,
                    _ => LedColor::BLACK,
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
            highlighted.key = format!(
                "layers.{}.worlds.behaviorConfig.{field}",
                self.active_layer_index
            );
        }
        let Some(param_mods) = self.param_mods.get(self.active_layer_index) else {
            return;
        };
        let lane = LedColor::SYSTEM.dim(8);
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
        self.set_display_led(leds, 0, 0, LedColor::WHITE);
        self.set_display_led(leds, 1, 1, LedColor::WHITE);
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
            LedColor::RED
        } else {
            LedColor::GREEN
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
        let Some(sense) = self.pulses_layers.get(self.active_layer_index) else {
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

    pub(super) fn dim_leds(&self, leds: &mut [LedColor], divisor: u8) {
        for cell in leds.iter_mut() {
            *cell = cell.dim(divisor);
        }
    }

    pub(super) fn sparks_pan_target(&self, instrument: &NativeInstrumentSlot) -> (u8, LedColor) {
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
                0 => LedColor::RED,
                1 => LedColor::BLUE,
                2 => LedColor::GREEN,
                3 => LedColor::YELLOW,
                _ => LedColor::WHITE,
            };
            return (pan, color);
        }
        (instrument.pan_pos, LedColor::WHITE)
    }
}
