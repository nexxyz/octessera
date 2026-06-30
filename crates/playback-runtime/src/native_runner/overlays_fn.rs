use super::{
    dance_fx_cell_id, dance_fx_type, momentary_fx_color, pan_marker_left_cell, trigger_gate_color,
    LedColor, NativeRunner, GRID_HEIGHT, GRID_WIDTH, INSTRUMENT_COUNT, TOUCH_FX_MAX_CONCURRENT,
};

impl NativeRunner {
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

    pub(super) fn apply_dance_mix_overlay(&self, leds: &mut [LedColor]) {
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

    pub(super) fn apply_dance_pan_overlay(&self, leds: &mut [LedColor]) {
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

    pub(super) fn apply_dance_fx_overlay(&self, leds: &mut [LedColor]) {
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

    pub(super) fn apply_dance_trigger_gate_overlay(&self, leds: &mut [LedColor]) {
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

    pub(super) fn apply_dance_xy_overlay(&self, leds: &mut [LedColor]) {
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

    pub(super) fn param_mod_overlay_ready(&self) -> bool {
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
