use super::super::led_color::LedColor;
use super::super::{NativeRunner, GRID_HEIGHT, GRID_WIDTH};

const BAR_PULSES: u64 = 96;
const RATE_COLUMNS: [&str; GRID_WIDTH] =
    ["1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2"];

impl NativeRunner {
    /// The plain view sets Build Step Rate, the Shift view Link Scan Unit; pressing the lit tile
    /// resyncs the layer instead.
    pub(in crate::native_runner) fn handle_play_rate_press(&mut self, x: usize, y: usize) {
        let layer = y.min(GRID_HEIGHT - 1);
        if self.layer_is_empty(layer) {
            return;
        }
        let scan_view = self.display.ui.shift_held;
        if scan_view && !self.layer_is_scanning(layer) {
            self.show_toast(format!("L{}: not scanning", layer + 1));
            return;
        }
        let rate = RATE_COLUMNS[x.min(GRID_WIDTH - 1)];
        if rate == self.play_rate_current_unit(layer, scan_view) {
            self.play_live.rate_resyncs[layer] = true;
            self.show_toast(format!("L{} resync", layer + 1));
            return;
        }
        let (key, label) = if scan_view {
            (format!("layers.{layer}.link.scanUnit"), "scan")
        } else if layer == self.active_layer_index {
            ("algorithmStep".to_string(), "rate")
        } else {
            (format!("layers.{layer}.algorithmStep"), "rate")
        };
        if self.menu.value_for_key(&key).is_none() {
            return;
        }
        self.menu.set_enum_value_for_key(&key, rate);
        if let Err(error) = self.apply_or_schedule_menu_key(&key) {
            self.show_toast(error);
            return;
        }
        self.show_toast(format!("L{} {label} {rate}", layer + 1));
    }

    pub(in crate::native_runner) fn apply_play_rate_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        let scan_view = self.display.ui.shift_held;
        let straight = if scan_view {
            LedColor::WHITE
        } else {
            LedColor::GREEN
        };
        for layer in 0..GRID_HEIGHT {
            if self.layer_is_empty(layer) {
                for x in 0..GRID_WIDTH {
                    self.set_display_led(leds, x, layer, LedColor::BLACK);
                }
                continue;
            }
            let current = (!scan_view || self.layer_is_scanning(layer))
                .then(|| self.play_rate_current_unit(layer, scan_view));
            for (x, rate) in RATE_COLUMNS.iter().enumerate() {
                let color = if rate.ends_with('T') {
                    LedColor::BLUE
                } else {
                    straight
                };
                let color = if current == Some(*rate) {
                    color
                } else {
                    color.dim(5)
                };
                self.set_display_led(leds, x, layer, color);
            }
        }
    }

    /// Layers the Rate page asked to resync start their next step, and their scan, on the bar this
    /// pulse step crossed.
    pub(in crate::native_runner) fn apply_due_rate_resyncs(&mut self, straight_pulses: u32) {
        if !self.play_live.rate_resyncs.contains(&true) {
            return;
        }
        let current = self.transport.current_ppqn_pulse;
        let previous = current.saturating_sub(u64::from(straight_pulses));
        if previous / BAR_PULSES == current / BAR_PULSES {
            return;
        }
        let since_bar = self
            .transport
            .swung_ppqn_pulse
            .saturating_sub(current / BAR_PULSES * BAR_PULSES);
        for layer in 0..GRID_HEIGHT {
            if !std::mem::take(&mut self.play_live.rate_resyncs[layer]) {
                continue;
            }
            let step = self.step_pulses_for_layer(layer);
            if let Some(accumulator) = self.transport.layer_pulse_accumulators.get_mut(layer) {
                *accumulator = step.saturating_add(u32::try_from(since_bar).unwrap_or(u32::MAX));
            }
            if let Some(tick) = self.transport.layer_ticks.get_mut(layer) {
                *tick = 0;
            }
            if layer == self.active_layer_index {
                self.transport.tick = 0;
                self.engine.set_interpretation_tick(0);
            } else if let Some(Some(engine)) = self.layer_engines.get_mut(layer) {
                engine.set_interpretation_tick(0);
            }
        }
    }

    fn layer_is_empty(&self, layer: usize) -> bool {
        self.layer_behavior_ids
            .get(layer)
            .is_none_or(|behavior| behavior == "none")
    }

    fn layer_is_scanning(&self, layer: usize) -> bool {
        self.link_layers
            .get(layer)
            .is_some_and(|link| link.scan_mode == "scanning")
    }

    fn play_rate_current_unit(&self, layer: usize, scan_view: bool) -> &'static str {
        let pulses = if scan_view {
            self.link_layers.get(layer).map_or(0, |link| {
                crate::timing_units::note_unit_to_pulses(&link.scan_unit)
            })
        } else {
            self.layer_step_rate_pulses(layer)
        };
        crate::timing_units::note_unit_from_pulses(pulses)
    }
}
