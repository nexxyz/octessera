use super::{
    momentary_fx_color, pan_marker_left_cell, sparks_fx_cell_id, sparks_fx_type,
    trigger_gate_color, LedColor, NativeRunner, GRID_HEIGHT, GRID_WIDTH, INSTRUMENT_COUNT,
    SPARKS_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    pub(super) fn apply_fn_overlay(&self, leds: &mut [LedColor]) {
        if !self.ui.fn_held && !self.ui.combined_modifier_held {
            return;
        }
        if self.sample_assign.is_some()
            || self.trigger_probability_assign.is_some()
            || self.sparks_fx_assign.is_some()
        {
            return;
        }
        self.dim_fn_overlay(leds);
        self.paint_fn_layer_column(leds);
        if !self.ui.combined_modifier_held {
            self.paint_fn_page_column(leds);
        }
    }

    pub(super) fn apply_sparks_mix_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for x in 0..INSTRUMENT_COUNT.min(GRID_WIDTH) {
            let instrument = self.instruments.get(x);
            let volume = instrument.map(|inst| inst.volume).unwrap_or(0).min(100);
            let y = ((f32::from(volume) / 100.0) * (GRID_HEIGHT - 1) as f32).round() as usize;
            let color = if instrument.map(|inst| inst.kind.as_str()) == Some("none") {
                LedColor::SYSTEM.dim(4)
            } else {
                LedColor::GREEN
            };
            self.set_display_led(leds, x, y, color);
        }
    }

    pub(super) fn apply_sparks_pan_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for y in 0..INSTRUMENT_COUNT.min(GRID_HEIGHT) {
            let Some(instrument) = self.instruments.get(y) else {
                continue;
            };
            let (pan_pos, color) = self.sparks_pan_target(instrument);
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

    pub(super) fn apply_sparks_fx_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for assignment in &self.sparks_fx_assignments {
            let id = sparks_fx_cell_id(assignment.x, assignment.y);
            let active = self
                .active_sparks_fx
                .iter()
                .any(|(active_id, _)| active_id == &id);
            let fx_type = sparks_fx_type(&assignment.config);
            let same_type_active = self
                .active_sparks_fx
                .iter()
                .any(|(_, active_type)| active_type == fx_type);
            let limited = !active
                && (self.active_sparks_fx.len() >= SPARKS_FX_MAX_CONCURRENT || same_type_active);
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

    pub(super) fn apply_sparks_trigger_gate_overlay(&self, leds: &mut [LedColor]) {
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

    pub(super) fn apply_sparks_xy_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        let x =
            (self.xy_touch.display_x.clamp(0.0, 1.0) * (GRID_WIDTH - 1) as f32).round() as usize;
        let y =
            (self.xy_touch.display_y.clamp(0.0, 1.0) * (GRID_HEIGHT - 1) as f32).round() as usize;
        let color = if self.xy_touch.active {
            LedColor::WHITE
        } else if self.xy_release == "sample-hold" {
            LedColor::SYSTEM.dim(2)
        } else {
            LedColor::SYSTEM.dim(4)
        };
        self.set_display_led(leds, x, y, color);
    }

    pub(super) fn param_mod_overlay_ready(&self) -> bool {
        self.ui.shift_held
            && !self.ui.fn_held
            && self.active_sparks_mode == "none"
            && self.sample_assign.is_none()
            && self.trigger_probability_assign.is_none()
            && self.sparks_fx_assign.is_none()
    }

    fn dim_fn_overlay(&self, leds: &mut [LedColor]) {
        for cell in leds.iter_mut() {
            *cell = LedColor::BLACK;
        }
    }

    fn paint_fn_layer_column(&self, leds: &mut [LedColor]) {
        for row in 0..GRID_HEIGHT {
            let configured = self
                .layer_behavior_ids
                .get(row)
                .map(|id| id != "none")
                .unwrap_or(false);
            let color = fn_layer_color(
                configured,
                row == self.active_layer_index,
                self.active_sparks_mode != "none",
            );
            self.set_display_led(leds, 0, row, color);
        }
    }

    fn paint_fn_page_column(&self, leds: &mut [LedColor]) {
        let page_options = ["mix", "pan", "fx", "trigger-gate", "transpose", "xy"];
        for (row, mode) in page_options.iter().enumerate() {
            let selected = self.active_sparks_mode != "none" && self.active_sparks_mode == *mode;
            let color = if selected {
                LedColor::GREEN
            } else {
                LedColor::YELLOW.dim(4)
            };
            self.set_display_led(leds, GRID_WIDTH - 1, row, color);
        }
        for row in page_options.len()..GRID_HEIGHT {
            self.set_display_led(leds, GRID_WIDTH - 1, row, LedColor::SYSTEM.dim(8));
        }
    }
}

fn fn_layer_color(configured: bool, active: bool, sparks_active: bool) -> LedColor {
    if sparks_active {
        if configured {
            LedColor::BLUE
        } else {
            LedColor::SYSTEM.dim(8)
        }
    } else if active {
        LedColor::BLUE
    } else if configured {
        LedColor::GREEN
    } else {
        LedColor::SYSTEM.dim(8)
    }
}
