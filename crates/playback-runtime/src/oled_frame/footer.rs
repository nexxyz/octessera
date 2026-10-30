use platform_core::palette;

use super::model::{OledSaveFlash, OledTransportFlash, OledTransportIcon, INDICATOR_FADE_LEVELS};
use super::pixels::{dim, fill_rect, rgb565, scale};
use super::text::draw_text_clipped;
use super::text_layout::fit_line_ellipsis;
use super::OledPresentationInput;

pub(super) fn draw_status_indicators(
    frame: &mut [u8],
    input: &OledPresentationInput,
    brightness: f32,
) {
    draw_cpu_meter(frame, input, brightness);
    if input.metrics.missed_quantum_flash {
        draw_missed_quantum_icon(frame, 107, 5, brightness);
    } else if input.metrics.high_cpu_steady {
        draw_cpu_icon(frame, 107, 5, rgb565(scale(palette::MAGENTA, brightness)));
    }
    if input.save_flash == OledSaveFlash::Flash {
        draw_save_icon(frame, 97, 4, rgb565(scale(palette::YELLOW, brightness)));
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
        let height = 3 + bar * 2;
        let rgb = if bar < lit {
            lit_rgb
        } else {
            dim(palette::GRAY, 4)
        };
        fill_rect(
            frame,
            116 + bar * 3,
            12 - height,
            2,
            height,
            rgb565(scale(rgb, brightness)),
        );
    }
}

pub(super) fn draw_footer(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let text = rgb565(scale(palette::WHITE, brightness));
    if !input.display.toast.is_empty() {
        let toast = fit_line_ellipsis(&input.display.toast, super::TOAST_RECT.columns());
        draw_text_clipped(
            frame,
            &toast,
            super::TOAST_RECT.x as i32,
            super::TOAST_RECT.y as i32,
            super::TOAST_RECT.columns(),
            text,
        );
    }
    draw_transport_icon(frame, input, brightness);
    let voice_steal = input.metrics.voice_steal_level;
    let level = input
        .event_dot_level
        .max(voice_steal)
        .min(INDICATOR_FADE_LEVELS);
    if level > 0 {
        let color = if voice_steal > 0 {
            palette::MAGENTA
        } else {
            palette::WHITE
        };
        let fade = 1.0 / f32::from(1_u8 << (INDICATOR_FADE_LEVELS - level));
        fill_rect(
            frame,
            119,
            119,
            5,
            5,
            rgb565(scale(color, brightness * fade)),
        );
    }
}

fn draw_transport_icon(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let rgb = match (&input.transport.icon, &input.transport.flash) {
        (OledTransportIcon::Play, OledTransportFlash::Measure) => palette::GREEN,
        (OledTransportIcon::Play, OledTransportFlash::Beat) => palette::YELLOW,
        (OledTransportIcon::Stop, _) => palette::MAGENTA,
        (OledTransportIcon::Pause, _) => palette::BLUE,
        _ => palette::WHITE,
    };
    draw_transport_shape(
        frame,
        &input.transport.icon,
        107,
        118,
        rgb565(scale(rgb, brightness)),
    );
}

fn draw_cpu_icon(frame: &mut [u8], x: usize, y: usize, color: u16) {
    fill_rect(frame, x + 1, y + 1, 6, 6, color);
    fill_rect(frame, x + 3, y + 3, 2, 2, 0);
    fill_rect(frame, x, y + 2, 1, 1, color);
    fill_rect(frame, x, y + 5, 1, 1, color);
}

fn draw_missed_quantum_icon(frame: &mut [u8], x: usize, y: usize, brightness: f32) {
    let white = rgb565(scale(palette::WHITE, brightness));
    let black = rgb565(scale(palette::BLACK, brightness));
    fill_rect(frame, x + 1, y + 1, 6, 6, black);
    fill_rect(frame, x + 3, y + 3, 2, 2, white);
    fill_rect(frame, x, y + 2, 1, 1, white);
    fill_rect(frame, x, y + 5, 1, 1, white);
}

fn draw_save_icon(frame: &mut [u8], x: usize, y: usize, color: u16) {
    fill_rect(frame, x, y, 8, 8, color);
    fill_rect(frame, x + 1, y + 1, 6, 2, 0);
    fill_rect(frame, x + 5, y + 1, 1, 2, color);
    fill_rect(frame, x + 2, y + 5, 4, 2, 0);
}

fn draw_transport_shape(
    frame: &mut [u8],
    icon: &OledTransportIcon,
    x: usize,
    y: usize,
    color: u16,
) {
    match icon {
        OledTransportIcon::Play => {
            fill_rect(frame, x, y, 2, 7, color);
            fill_rect(frame, x + 2, y + 1, 1, 5, color);
            fill_rect(frame, x + 3, y + 2, 1, 3, color);
            fill_rect(frame, x + 4, y + 3, 1, 1, color);
        }
        OledTransportIcon::Pause => {
            fill_rect(frame, x, y, 3, 7, color);
            fill_rect(frame, x + 4, y, 3, 7, color);
        }
        _ => fill_rect(frame, x, y, 7, 7, color),
    }
}
