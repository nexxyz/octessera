use platform_core::palette;

use super::model::{OledSaveFlash, OledTransportFlash, OledTransportIcon};
use super::pixels::{fill_rect, rgb565, scale};
use super::text::draw_text_clipped;
use super::OledPresentationInput;

pub(super) fn draw_status_indicators(
    frame: &mut [u8],
    input: &OledPresentationInput,
    brightness: f32,
) {
    if input.metrics.cpu_hot {
        draw_cpu_icon(frame, 117, 5, rgb565(scale(palette::RED, brightness)));
    }
    if input.save_flash == OledSaveFlash::Flash {
        draw_save_icon(frame, 107, 5, rgb565(scale(palette::YELLOW, brightness)));
    }
}

pub(super) fn draw_footer(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let text = rgb565(scale(palette::WHITE, brightness));
    if !input.display.toast.is_empty() {
        let background = rgb565(scale(palette::BLACK, brightness));
        fill_rect(frame, 0, 114, 128, 14, background);
        draw_text_clipped(frame, &input.display.toast, 5, 118, 17, text);
        return;
    }
    draw_transport_icon(frame, input, brightness);
    if input.event_dot_on {
        let color = if input.metrics.voice_steal {
            palette::RED
        } else {
            palette::WHITE
        };
        fill_rect(frame, 119, 119, 5, 5, rgb565(scale(color, brightness)));
    }
}

fn draw_transport_icon(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let rgb = match (&input.transport.icon, &input.transport.flash) {
        (OledTransportIcon::Play, OledTransportFlash::Measure) => palette::GREEN,
        (OledTransportIcon::Play, OledTransportFlash::Beat) => palette::YELLOW,
        (OledTransportIcon::Stop, _) => palette::RED,
        (OledTransportIcon::Pause, _) => palette::BLUE,
        _ => palette::WHITE,
    };
    draw_transport_shape(
        frame,
        &input.transport.icon,
        101,
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
            fill_rect(frame, x, y, 2, 9, color);
            fill_rect(frame, x + 2, y + 1, 2, 7, color);
            fill_rect(frame, x + 4, y + 2, 2, 5, color);
            fill_rect(frame, x + 6, y + 3, 2, 3, color);
            fill_rect(frame, x + 8, y + 4, 1, 1, color);
        }
        OledTransportIcon::Pause => {
            fill_rect(frame, x, y, 3, 8, color);
            fill_rect(frame, x + 6, y, 3, 8, color);
        }
        _ => fill_rect(frame, x, y, 8, 8, color),
    }
}
