use super::super::led_color::LedColor;
use super::super::{NativeRunner, GRID_HEIGHT, GRID_WIDTH};

const BAR_PULSES: u64 = 96;
const RATE_COLUMNS: [&str; GRID_WIDTH] =
    ["1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2"];

impl NativeRunner {
    pub(in crate::native_runner) fn handle_play_rate_press(&mut self, x: usize, y: usize) {
        let layer = y.min(GRID_HEIGHT - 1);
        if self.layer_is_empty(layer) {
            return;
        }
        if self.display.ui.shift_held {
            self.play_live.rate_resyncs[layer] = true;
            self.show_toast(format!("L{} resync", layer + 1));
            return;
        }
        let rate = RATE_COLUMNS[x.min(GRID_WIDTH - 1)];
        let key = if layer == self.active_layer_index {
            "algorithmStep".to_string()
        } else {
            format!("layers.{layer}.algorithmStep")
        };
        if self.menu.value_for_key(&key).is_none() {
            return;
        }
        self.menu.set_enum_value_for_key(&key, rate);
        if let Err(error) = self.apply_or_schedule_menu_key(&key) {
            self.show_toast(error);
            return;
        }
        self.show_toast(format!("L{} rate {rate}", layer + 1));
    }

    pub(in crate::native_runner) fn apply_play_rate_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for layer in 0..GRID_HEIGHT {
            if self.layer_is_empty(layer) {
                for x in 0..GRID_WIDTH {
                    self.set_display_led(leds, x, layer, LedColor::BLACK);
                }
                continue;
            }
            let current =
                crate::timing_units::note_unit_from_pulses(self.layer_step_rate_pulses(layer));
            for (x, rate) in RATE_COLUMNS.iter().enumerate() {
                let color = if rate.ends_with('T') {
                    LedColor::BLUE
                } else {
                    LedColor::GREEN
                };
                let color = if *rate == current {
                    color
                } else {
                    color.dim(5)
                };
                self.set_display_led(leds, x, layer, color);
            }
        }
    }

    /// Layers the Rate page asked to resync start their next step on the bar this pulse step
    /// crossed.
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
        }
    }

    fn layer_is_empty(&self, layer: usize) -> bool {
        self.layer_behavior_ids
            .get(layer)
            .is_none_or(|behavior| behavior == "none")
    }

    fn layer_step_rate_pulses(&self, layer: usize) -> u32 {
        if layer == self.active_layer_index {
            self.transport.algorithm_step_pulses
        } else {
            self.transport
                .layer_algorithm_step_pulses
                .get(layer)
                .copied()
                .unwrap_or(super::super::DEFAULT_ALGORITHM_STEP_RED)
        }
    }
}
