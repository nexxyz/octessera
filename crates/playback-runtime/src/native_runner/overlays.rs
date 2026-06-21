use super::{
    add_dim_white_overlay, dance_fx_cell_id, dance_fx_type, dim_color, display_index, json,
    momentary_fx_color, native_binding_from_spec, pan_marker_left_cell, scan_index_for_overlay,
    scan_section_count, trigger_gate_color, NativeInstrumentSlot, NativeParamBinding, NativeRunner,
    Value, GRID_HEIGHT, GRID_WIDTH, INSTRUMENT_COUNT, TOUCH_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    pub(super) fn apply_dance_overlay(&self, leds: &mut [Value]) {
        match self.active_dance_mode.as_str() {
            "mix" => self.apply_dance_mix_overlay(leds),
            "pan" => self.apply_dance_pan_overlay(leds),
            "fx" => self.apply_dance_fx_overlay(leds),
            "trigger-gate" => self.apply_dance_trigger_gate_overlay(leds),
            "xy" => self.apply_dance_xy_overlay(leds),
            _ => {}
        }
    }

    pub(super) fn apply_sample_assignment_overlay(&self, leds: &mut [Value]) {
        let Some((instrument_slot, selected_sample_slot)) = self.sample_assign else {
            return;
        };
        self.fill_leds(leds, json!({ "r": 0, "g": 0, "b": 0 }));
        let Some(instrument) = self.instruments.get(instrument_slot) else {
            return;
        };
        for assignment in &instrument.sample_assignments {
            let color = if assignment.sample_slot == selected_sample_slot {
                match assignment.level.as_deref() {
                    Some("high") => json!({ "r": 220, "g": 0, "b": 0 }),
                    Some("medium") => json!({ "r": 220, "g": 180, "b": 0 }),
                    Some("low") => json!({ "r": 0, "g": 220, "b": 0 }),
                    _ => json!({ "r": 220, "g": 220, "b": 220 }),
                }
            } else {
                json!({ "r": 70, "g": 70, "b": 70 })
            };
            self.set_display_led(leds, assignment.x, assignment.y, color);
        }
    }

    pub(super) fn apply_trigger_probability_overlay(&self, leds: &mut [Value]) {
        let Some(part_index) = self.trigger_probability_assign else {
            return;
        };
        self.fill_leds(leds, json!({ "r": 0, "g": 0, "b": 0 }));
        let Some(map) = self.trigger_probability_maps.get(part_index) else {
            return;
        };
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let color = match map.get(y * GRID_WIDTH + x).map(String::as_str) {
                    Some("low") => json!({ "r": 220, "g": 0, "b": 0 }),
                    Some("high") => json!({ "r": 220, "g": 180, "b": 0 }),
                    Some("full") => json!({ "r": 0, "g": 220, "b": 0 }),
                    _ => json!({ "r": 0, "g": 0, "b": 0 }),
                };
                self.set_display_led(leds, x, y, color);
            }
        }
    }

    pub(super) fn apply_param_mod_overlay(&self, leds: &mut [Value]) {
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
        let lane = json!({ "r": 18, "g": 18, "b": 24 });
        for x in 0..GRID_WIDTH {
            self.set_display_led(leds, x, 0, lane.clone());
            self.set_display_led(leds, x, 1, lane.clone());
        }
        for y in 0..GRID_HEIGHT {
            self.set_display_led(leds, 0, y, lane.clone());
            self.set_display_led(leds, 1, y, lane.clone());
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
        self.set_display_led(leds, 0, 0, json!({ "r": 255, "g": 255, "b": 255 }));
        self.set_display_led(leds, 1, 1, json!({ "r": 255, "g": 255, "b": 255 }));
    }

    fn paint_param_mod_axis_slot(
        &self,
        leds: &mut [Value],
        binding: &NativeParamBinding,
        highlighted_key: &str,
        axis: &str,
        slot: usize,
    ) {
        let color = if binding.invert {
            json!({ "r": 255, "g": 0, "b": 90 })
        } else {
            json!({ "r": 0, "g": 255, "b": 120 })
        };
        let color = if binding.key == highlighted_key {
            color
        } else {
            dim_color(color, 3)
        };
        if axis == "x" {
            for x in 0..GRID_WIDTH {
                self.set_display_led(leds, x, slot, color.clone());
            }
        } else {
            for y in 0..GRID_HEIGHT {
                self.set_display_led(leds, slot, y, color.clone());
            }
        }
    }

    pub(super) fn apply_scan_progress_overlay(&self, leds: &mut [Value]) {
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

    fn add_scan_overlay_led(&self, leds: &mut [Value], x: usize, y: usize) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let index = display_index(x, y);
        if let Some(cell) = leds.get_mut(index) {
            *cell = add_dim_white_overlay(cell, 24);
        }
    }

    pub(super) fn set_display_led(&self, leds: &mut [Value], x: usize, y: usize, value: Value) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let index = display_index(x, y);
        if let Some(cell) = leds.get_mut(index) {
            *cell = value;
        }
    }

    fn fill_leds(&self, leds: &mut [Value], value: Value) {
        for cell in leds.iter_mut() {
            *cell = value.clone();
        }
    }

    fn dim_leds(&self, leds: &mut [Value], divisor: i64) {
        for cell in leds.iter_mut() {
            *cell = dim_color(cell.clone(), divisor);
        }
    }

    fn dance_pan_target(&self, instrument: &NativeInstrumentSlot) -> (u8, Value) {
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
                0 => json!({ "r": 190, "g": 80, "b": 255 }),
                1 => json!({ "r": 0, "g": 210, "b": 255 }),
                2 => json!({ "r": 0, "g": 230, "b": 120 }),
                3 => json!({ "r": 255, "g": 160, "b": 0 }),
                _ => json!({ "r": 255, "g": 255, "b": 255 }),
            };
            return (pan, color);
        }
        (instrument.pan_pos, json!({ "r": 255, "g": 255, "b": 255 }))
    }

    pub(super) fn apply_fn_overlay(&self, leds: &mut [Value]) {
        if !self.ui.fn_held {
            return;
        }
        if self.active_dance_mode != "none" {
            self.dim_dance_fn_overlay(leds);
        }
        self.paint_fn_part_column(leds);
        self.paint_fn_page_column(leds);
    }

    fn apply_dance_mix_overlay(&self, leds: &mut [Value]) {
        self.dim_leds(leds, 4);
        for x in 0..INSTRUMENT_COUNT.min(GRID_WIDTH) {
            let instrument = self.instruments.get(x);
            let volume = instrument.map(|inst| inst.volume).unwrap_or(0).min(100);
            let y = ((f32::from(volume) / 100.0) * (GRID_HEIGHT - 1) as f32).round() as usize;
            let color = if instrument.map(|inst| inst.kind.as_str()) == Some("none") {
                json!({ "r": 0, "g": 55, "b": 22 })
            } else {
                json!({ "r": 0, "g": 220, "b": 90 })
            };
            self.set_display_led(leds, x, y, color);
        }
    }

    fn apply_dance_pan_overlay(&self, leds: &mut [Value]) {
        self.dim_leds(leds, 4);
        for y in 0..INSTRUMENT_COUNT.min(GRID_HEIGHT) {
            let Some(instrument) = self.instruments.get(y) else {
                continue;
            };
            let (pan_pos, color) = self.dance_pan_target(instrument);
            let left = pan_marker_left_cell(pan_pos);
            let value = if instrument.kind == "none" {
                dim_color(color, 4)
            } else {
                color
            };
            self.set_display_led(leds, left, y, value.clone());
            self.set_display_led(leds, left + 1, y, value);
        }
    }

    fn apply_dance_fx_overlay(&self, leds: &mut [Value]) {
        self.dim_leds(leds, 4);
        for assignment in &self.dance_fx_assignments {
            let id = dance_fx_cell_id(assignment.x, assignment.y);
            let active = self.active_dance_fx.iter().any(|(active_id, _)| active_id == &id);
            let limited = !active && self.active_dance_fx.len() >= TOUCH_FX_MAX_CONCURRENT;
            let color = momentary_fx_color(dance_fx_type(&assignment.config));
            self.set_display_led(
                leds,
                assignment.x,
                assignment.y,
                if active {
                    add_dim_white_overlay(&color, 70)
                } else if limited {
                    dim_color(color, 5)
                } else {
                    color
                },
            );
        }
    }

    fn apply_dance_trigger_gate_overlay(&self, leds: &mut [Value]) {
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
                        dim_color(color, 4)
                    },
                );
            }
        }
        self.set_display_led(leds, 5, 0, trigger_gate_color("zero"));
        self.set_display_led(leds, 6, 0, trigger_gate_color("custom"));
        self.set_display_led(leds, 7, 0, trigger_gate_color("full"));
    }

    fn apply_dance_xy_overlay(&self, leds: &mut [Value]) {
        self.dim_leds(leds, 4);
        self.set_display_led(leds, 4, 4, json!({ "r": 180, "g": 180, "b": 180 }));
    }

    fn param_mod_overlay_ready(&self) -> bool {
        self.ui.shift_held
            && !self.ui.fn_held
            && self.active_dance_mode == "none"
            && self.sample_assign.is_none()
            && self.trigger_probability_assign.is_none()
            && self.dance_fx_assign.is_none()
    }

    fn dim_dance_fn_overlay(&self, leds: &mut [Value]) {
        for cell in leds.iter_mut() {
            if let Some(object) = cell.as_object_mut() {
                let r = object.get("r").and_then(|v| v.as_i64()).unwrap_or(0);
                let g = object.get("g").and_then(|v| v.as_i64()).unwrap_or(0);
                let b = object.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
                object.insert("r".into(), Value::from(r / 4));
                object.insert("g".into(), Value::from(g / 4));
                object.insert("b".into(), Value::from(b / 4));
            }
        }
    }

    fn paint_fn_part_column(&self, leds: &mut [Value]) {
        for row in 0..GRID_HEIGHT {
            let configured = self
                .part_behavior_ids
                .get(row)
                .map(|id| id != "none")
                .unwrap_or(false);
            let color = fn_part_color(configured, row == self.active_part_index, self.active_dance_mode != "none");
            self.set_display_led(leds, 0, row, color);
        }
    }

    fn paint_fn_page_column(&self, leds: &mut [Value]) {
        let page_options = ["mix", "pan", "fx", "trigger-gate", "xy"];
        for (row, mode) in page_options.iter().enumerate() {
            let selected = self.active_dance_mode != "none" && self.active_dance_mode == *mode;
            let color = if selected {
                json!({ "r": 0, "g": 158, "b": 158 })
            } else {
                json!({ "r": 0, "g": 60, "b": 60 })
            };
            self.set_display_led(leds, GRID_WIDTH - 1, row, color);
        }
    }
}

fn fn_part_color(configured: bool, active: bool, dance_active: bool) -> Value {
    if dance_active {
        if configured {
            json!({ "r": 0, "g": 120, "b": 0 })
        } else {
            json!({ "r": 0, "g": 48, "b": 23 })
        }
    } else if active {
        json!({ "r": 0, "g": 191, "b": 191 })
    } else if configured {
        json!({ "r": 0, "g": 120, "b": 0 })
    } else {
        json!({ "r": 0, "g": 48, "b": 23 })
    }
}
