use std::time::{Duration, Instant};

use super::super::led_color::LedColor;
use super::super::{
    DisplayTransientPresentation, NativeRunner, RuntimeTransportState, SyncSource, TransportFlash,
    GRID_HEIGHT, GRID_WIDTH,
};

const TAP_RESET: Duration = Duration::from_secs(2);
const TAP_WINDOW: usize = 4;
const TAP_TILE: (usize, usize) = (0, GRID_HEIGHT - 1);
const LOWEST_TEMPO_ROW: usize = 3;
const NUDGE_ROW: usize = 2;
const SWING_A_ROW: usize = 1;
const SWING_B_ROW: usize = 0;
const FIRST_TILE_BPM: i32 = 50;
const TILE_BPM_STEP: i32 = 5;
const TEMPO_TILE_COUNT: i32 = 39;
const NUDGE_COLUMNS: [i32; GRID_WIDTH] = [-4, -3, -2, -1, 1, 2, 3, 4];
const SWING_COLUMNS: [u8; GRID_WIDTH] = [1, 2, 3, 4, 4, 5, 6, 7];

impl NativeRunner {
    pub(in crate::native_runner) fn handle_play_tempo_press(&mut self, x: usize, y: usize) {
        self.handle_play_tempo_press_at(x, y, Instant::now());
    }

    pub(in crate::native_runner) fn handle_play_tempo_press_at(
        &mut self,
        x: usize,
        y: usize,
        now: Instant,
    ) {
        let x = x.min(GRID_WIDTH - 1);
        match y {
            SWING_A_ROW => self.set_swing_from_grid("transport.swingA", SWING_COLUMNS[x]),
            SWING_B_ROW => self.set_swing_from_grid("transport.swingB", SWING_COLUMNS[x]),
            _ if self.transport.sync_source == SyncSource::External => {
                self.show_toast("External sync");
            }
            NUDGE_ROW => {
                let bpm = i32::from(crate::delay_timing::visible_bpm_u16(self.transport.bpm));
                self.set_tempo_from_grid(bpm + NUDGE_COLUMNS[x], "Tempo");
            }
            _ => match tempo_tile_bpm(x, y) {
                Some(bpm) => self.set_tempo_from_grid(bpm, "Tempo"),
                None => self.tap_tempo(now),
            },
        }
    }

    pub(in crate::native_runner) fn apply_play_tempo_overlay(
        &self,
        leds: &mut [LedColor],
        transient: DisplayTransientPresentation,
    ) {
        self.dim_leds(leds, 4);
        let external = self.transport.sync_source == SyncSource::External;
        let tempo_dim = |color: LedColor| if external { color.dim(4) } else { color };
        let bpm = i32::from(crate::delay_timing::visible_bpm_u16(self.transport.bpm));
        let nearest = ((bpm - FIRST_TILE_BPM) as f32 / TILE_BPM_STEP as f32)
            .round()
            .clamp(0.0, (TEMPO_TILE_COUNT - 1) as f32) as i32
            * TILE_BPM_STEP
            + FIRST_TILE_BPM;
        for y in LOWEST_TEMPO_ROW..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let color = match tempo_tile_bpm(x, y) {
                    Some(tile) if tile == bpm => LedColor::GREEN,
                    Some(tile) if tile == nearest => LedColor::YELLOW,
                    Some(_) => LedColor::SYSTEM.dim(4),
                    None => self.tap_tile_color(transient),
                };
                self.set_display_led(leds, x, y, tempo_dim(color));
            }
        }
        for (x, nudge) in NUDGE_COLUMNS.iter().enumerate() {
            let color = if *nudge < 0 {
                LedColor::BLUE
            } else {
                LedColor::MAGENTA
            };
            self.set_display_led(leds, x, NUDGE_ROW, tempo_dim(color.dim(2)));
        }
        for (y, value) in [
            (SWING_A_ROW, self.transport.swing_a),
            (SWING_B_ROW, self.transport.swing_b),
        ] {
            let left = usize::from(value.clamp(1, 7) - 1);
            for x in 0..GRID_WIDTH {
                let color = if x == left || x == left + 1 {
                    LedColor::WHITE
                } else {
                    LedColor::SYSTEM.dim(4)
                };
                self.set_display_led(leds, x, y, color);
            }
        }
    }

    fn tap_tile_color(&self, transient: DisplayTransientPresentation) -> LedColor {
        if self.transport.transport != RuntimeTransportState::Playing {
            return LedColor::GREEN.dim(6);
        }
        match transient.transport_flash {
            TransportFlash::Measure => LedColor::GREEN,
            TransportFlash::Beat => LedColor::GREEN.dim(2),
            TransportFlash::None => LedColor::GREEN.dim(6),
        }
    }

    fn set_tempo_from_grid(&mut self, bpm: i32, label: &str) {
        let bpm = bpm.clamp(40, 240);
        self.menu.set_number_value_for_key("transport.bpm", bpm);
        if !self.dispatch_play_menu_key("transport.bpm") {
            return;
        }
        self.show_toast(format!("{label} {bpm}"));
    }

    fn set_swing_from_grid(&mut self, key: &str, value: u8) {
        self.menu.set_number_value_for_key(key, i32::from(value));
        if !self.dispatch_play_menu_key(key) {
            return;
        }
        self.show_toast(format!(
            "Swing {}:{}",
            self.transport.swing_a, self.transport.swing_b
        ));
    }

    fn tap_tempo(&mut self, now: Instant) {
        let taps = &mut self.play_live.tempo_taps;
        if taps
            .last()
            .is_some_and(|last| now.saturating_duration_since(*last) > TAP_RESET)
        {
            taps.clear();
        }
        taps.push(now);
        if taps.len() > TAP_WINDOW {
            taps.remove(0);
        }
        let intervals = taps.len().saturating_sub(1);
        if intervals == 0 {
            self.show_toast("Tap");
            return;
        }
        let beat = now.saturating_duration_since(taps[0]).as_secs_f64() / intervals as f64;
        let bpm = if beat > 0.0 { 60.0 / beat } else { 240.0 };
        self.set_tempo_from_grid(bpm.round().min(240.0) as i32, "Tap");
    }
}

/// The BPM a tempo tile sets; the top-left tile is Tap and has none.
fn tempo_tile_bpm(x: usize, y: usize) -> Option<i32> {
    if !(LOWEST_TEMPO_ROW..GRID_HEIGHT).contains(&y) || (x, y) == TAP_TILE {
        return None;
    }
    let index = ((GRID_HEIGHT - 1 - y) * GRID_WIDTH + x) as i32 - 1;
    Some(FIRST_TILE_BPM + TILE_BPM_STEP * index)
}

#[cfg(test)]
mod tests {
    use super::tempo_tile_bpm;

    #[test]
    fn tempo_tiles_run_from_50_to_240_in_fives_below_the_tap_tile() {
        assert_eq!(tempo_tile_bpm(0, 7), None);
        assert_eq!(tempo_tile_bpm(1, 7), Some(50));
        assert_eq!(tempo_tile_bpm(7, 7), Some(80));
        assert_eq!(tempo_tile_bpm(0, 6), Some(85));
        assert_eq!(tempo_tile_bpm(7, 6), Some(120));
        assert_eq!(tempo_tile_bpm(0, 3), Some(205));
        assert_eq!(tempo_tile_bpm(7, 3), Some(240));
        assert_eq!(tempo_tile_bpm(0, 2), None);
    }
}
