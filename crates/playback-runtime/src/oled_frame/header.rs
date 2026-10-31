use platform_core::palette;

use super::font::TOM_THUMB;
use super::model::OledSaveFlash;
use super::pixels::{dim, draw_mask, fill_rect, rgb565, scale, shade};
use super::text::title_text_for_oled;
use super::OledPresentationInput;

pub(super) const RULE_Y: usize = 8;
/// The breadcrumb ends at or before this column, left of the header icons.
pub(super) const BREADCRUMB_RIGHT: usize = 103;
const ICON_Y: usize = 0;
const MISSED_QUANTUM_X: usize = 106;
const SAVE_X: usize = 113;
const CPU_X: usize = 120;
const SAVE_CHECK: [u8; 5] = [0b00001, 0b00011, 0b10110, 0b11100, 0b01000];
const MISSED_QUANTUM: [u8; 5] = [0b11111, 0b10001, 0b10101, 0b10001, 0b11111];

/// Whether a breadcrumb fits the header left of the icons.
pub(crate) fn breadcrumb_fits(text: &str) -> bool {
    TOM_THUMB.width(text) <= BREADCRUMB_RIGHT + 1
}

pub(super) fn draw_header(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let area = shade(input.display.title_color, brightness);
    let title = title_text_for_oled(&input.display.title);
    TOM_THUMB.draw_fitted(frame, &title, BREADCRUMB_RIGHT + 1, 0, ICON_Y, area);
    fill_rect(frame, 0, RULE_Y, 128, 1, area);
    draw_cpu_meter(frame, input, brightness);
    if input.save_flash == OledSaveFlash::Flash {
        let green = rgb565(scale(palette::GREEN, brightness));
        draw_mask(frame, &SAVE_CHECK, 5, SAVE_X, ICON_Y, green);
    }
    if input.metrics.missed_quantum_flash {
        let red = rgb565(scale(palette::BEHAVIOR_PRIMARY_RED, brightness));
        draw_mask(frame, &MISSED_QUANTUM, 5, MISSED_QUANTUM_X, ICON_Y, red);
    }
}

pub(super) fn cpu_meter_bars(peak_percent: u8, warn_percent: u8) -> usize {
    let moderate_percent = u16::from(warn_percent) * 2 / 3 / 5 * 5;
    if peak_percent >= warn_percent {
        3
    } else if u16::from(peak_percent) >= moderate_percent {
        2
    } else {
        1
    }
}

fn draw_cpu_meter(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let lit = cpu_meter_bars(input.metrics.cpu_load_peak_percent, input.cpu_warn_percent);
    let lit_rgb = match lit {
        3 => palette::BEHAVIOR_PRIMARY_RED,
        2 => palette::YELLOW,
        _ => palette::GREEN,
    };
    for bar in 0..3 {
        let rgb = if bar < lit {
            lit_rgb
        } else {
            dim(palette::GRAY, 4)
        };
        fill_rect(
            frame,
            CPU_X + bar * 3,
            ICON_Y,
            2,
            5,
            rgb565(scale(rgb, brightness)),
        );
    }
}
