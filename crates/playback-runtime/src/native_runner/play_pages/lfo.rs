use super::super::led_color::LedColor;
use super::super::{NativeLinkLfo, NativeRunner, GLOBAL_LFO_COUNT, GRID_HEIGHT, GRID_WIDTH};

const PERIODS: [&str; GRID_WIDTH - 1] = ["1/8", "1/4", "1/2", "1/1", "2/1", "4/1", "8/1"];
const DEPTHS: [u8; GRID_WIDTH - 1] = [15, 29, 43, 57, 71, 86, 100];

impl NativeRunner {
    pub(in crate::native_runner) fn handle_play_lfo_press(&mut self, x: usize, y: usize) {
        let index = y.min(GRID_HEIGHT - 1);
        let Some(lfo) = self.link_lfos.get(index) else {
            return;
        };
        let prefix = format!("linkLfos.{index}");
        let column = x.min(GRID_WIDTH - 1);
        if column == 0 {
            if lfo.target.is_none() {
                self.show_toast(format!("LFO{}: no target", index + 1));
                return;
            }
            let enabled = !lfo.enabled;
            let key = format!("{prefix}.enabled");
            self.menu.set_bool_value_for_key(&key, enabled);
            if self.dispatch_play_menu_key(&key) {
                self.show_toast(format!(
                    "LFO{} {}",
                    index + 1,
                    if enabled { "on" } else { "off" }
                ));
            }
        } else if self.display.ui.shift_held {
            let depth = DEPTHS[column - 1];
            let key = format!("{prefix}.depthPct");
            self.menu.set_number_value_for_key(&key, i32::from(depth));
            if self.dispatch_play_menu_key(&key) {
                self.show_toast(format!("LFO{} depth {depth}%", index + 1));
            }
        } else {
            let base = PERIODS[column - 1];
            let period = if lfo.period == base {
                format!("{base}T")
            } else {
                base.to_string()
            };
            let key = format!("{prefix}.period");
            self.menu.set_enum_value_for_key(&key, &period);
            if self.dispatch_play_menu_key(&key) {
                self.show_toast(format!("LFO{} period {period}", index + 1));
            }
        }
    }

    pub(in crate::native_runner) fn apply_play_lfo_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for (y, lfo) in self.link_lfos.iter().enumerate().take(GLOBAL_LFO_COUNT) {
            self.set_display_led(leds, 0, y, lfo_value_color(lfo));
            for x in 1..GRID_WIDTH {
                let color = if self.display.ui.shift_held {
                    let depth_column = DEPTHS
                        .iter()
                        .position(|depth| lfo.depth_pct <= *depth)
                        .unwrap_or(DEPTHS.len() - 1)
                        + 1;
                    if x <= depth_column && lfo.depth_pct > 0 {
                        LedColor::MAGENTA
                    } else {
                        LedColor::MAGENTA.dim(6)
                    }
                } else {
                    let base = PERIODS[x - 1];
                    if lfo.period == base {
                        LedColor::GREEN
                    } else if lfo.period.strip_suffix('T') == Some(base) {
                        LedColor::BLUE
                    } else {
                        LedColor::GREEN.dim(5)
                    }
                };
                self.set_display_led(leds, x, y, color);
            }
        }
    }
}

/// Green that brightens and darkens with the LFO's current value; dark without a target.
fn lfo_value_color(lfo: &NativeLinkLfo) -> LedColor {
    if lfo.target.is_none() {
        return LedColor::BLACK;
    }
    if !lfo.enabled {
        return LedColor::SYSTEM.dim(6);
    }
    let period = crate::timing_units::lfo_period_pulses(&lfo.period).max(1);
    let phase = f64::from(lfo.phase_pulses) / f64::from(period);
    let level = (f64::from(realtime_engine::sine::sin_turns(phase as f32)) + 1.0) / 2.0;
    LedColor::GREEN.dim(1 + ((1.0 - level) * 7.0).round() as u8)
}
