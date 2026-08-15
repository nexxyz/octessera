use platform_core::palette;

use super::error_layout::runtime_error_rows;
use super::footer::{draw_footer, draw_status_indicators};
use super::model::{OledBarInput, OledBarStyle, OledRuntimeErrorMetadata};
use super::pixels::{dim, fill_rect, rgb565, rgb565_to_rgb, scale};
use super::splash::render_splash;
use super::text::{draw_text, draw_text_clipped, title_text_for_oled};
use super::{OledPresentationInput, OledSplash, OLED_FRAME_BYTES};

pub fn render_oled_frame(input: &OledPresentationInput) -> Vec<u8> {
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    render_oled_frame_into(input, &mut frame);
    frame
}

pub(crate) fn render_oled_frame_into(input: &OledPresentationInput, frame: &mut [u8]) {
    assert_eq!(frame.len(), OLED_FRAME_BYTES);
    frame.fill(0);
    if input.display.off {
        return;
    }
    let brightness = brightness_scale(input.display_brightness);
    if let Some(error) = input.runtime_error.as_ref() {
        if is_concise_runtime_error_presentation(error) {
            render_menu_frame(frame, input, brightness);
        } else {
            render_runtime_error_frame(frame, error, brightness);
        }
        return;
    }
    if input.display.splash != OledSplash::None {
        render_splash(frame, &input.display.splash, brightness);
        overlay_toast(frame, &input.display.toast, brightness);
        return;
    }
    render_menu_frame(frame, input, brightness);
}

fn brightness_scale(value: u8) -> f32 {
    f32::from(value.min(100)) / 100.0
}

fn is_concise_runtime_error_presentation(error: &OledRuntimeErrorMetadata) -> bool {
    error.domain.as_deref() == Some("midi")
        && error.operation.as_deref() == Some("midi_list_inputs")
}

fn render_menu_frame(frame: &mut [u8], input: &OledPresentationInput, brightness: f32) {
    let title = title_text_for_oled(&input.display.title);
    let title_color = rgb565(scale(palette::WHITE, brightness));
    let dim_color = rgb565(scale(dim(palette::GRAY, 4), brightness));
    let text_color = rgb565(scale(palette::WHITE, brightness));
    fill_rect(
        frame,
        0,
        0,
        128,
        16,
        rgb565(scale(palette::BLACK, brightness)),
    );
    draw_text_clipped(frame, &title, 5, 5, 15, title_color);
    draw_status_indicators(frame, input, brightness);

    for (index, line) in input.display.lines.iter().take(7).enumerate() {
        let y = 18 + index * 13;
        let color = input
            .display
            .colors
            .get(index)
            .map(|color| rgb565(scale(rgb565_to_rgb(*color), brightness)))
            .unwrap_or(text_color);
        let selected = input.selected_row == Some(index);
        let bar = input.display.bars.get(index).and_then(Option::as_ref);
        if selected {
            fill_rect(frame, 3, y - 1, 122, 11, color);
        }
        if let Some(bar) = bar {
            draw_bar(frame, 87, y - 1, bar, color, selected);
        }
        let text = if selected {
            rgb565(scale(palette::BLACK, brightness))
        } else {
            color
        };
        let text_x = if line.starts_with("  ") { 4 } else { 6 };
        let text_width = if bar.is_some() { 13 } else { 19 };
        draw_text_clipped(frame, line, text_x, y as i32, text_width, text);
    }
    draw_scrollbar(frame, input, dim_color, text_color);
    draw_footer(frame, input, brightness);
}

fn draw_bar(frame: &mut [u8], x: usize, y: usize, bar: &OledBarInput, fill: u16, selected: bool) {
    let fraction = bar.fraction.clamp(0.0, 1.0);
    let outer_width = 36;
    let outer_height = 9;
    let inner_x = x + 1;
    let inner_y = y + 1;
    let inner_width = outer_width - 2;
    let inner_height = outer_height - 2;
    let outline = if selected {
        rgb565(scale(palette::BLACK, 1.0))
    } else {
        fill
    };
    let track = if selected {
        rgb565(scale(palette::BLACK, 1.0))
    } else {
        rgb565(dim(rgb565_to_rgb(fill), 6))
    };
    fill_rect(frame, x, y, outer_width, outer_height, outline);
    fill_rect(frame, inner_x, inner_y, inner_width, inner_height, track);
    if bar.style == OledBarStyle::Marker {
        let marker_x = inner_x + ((inner_width - 1) as f32 * fraction).round() as usize;
        fill_rect(frame, marker_x, inner_y + 1, 1, inner_height - 2, fill);
        return;
    }
    fill_rect(
        frame,
        inner_x,
        inner_y,
        ((inner_width as f32) * fraction).round() as usize,
        inner_height,
        fill,
    );
}

fn draw_scrollbar(frame: &mut [u8], input: &OledPresentationInput, track: u16, thumb: u16) {
    let Some(scroll) = input.display.scroll.as_ref() else {
        return;
    };
    if scroll.total_rows <= scroll.visible_rows || scroll.visible_rows == 0 {
        return;
    }
    let body_top = 18;
    let body_height = 88;
    let thumb_height = ((scroll.visible_rows * body_height) / scroll.total_rows).max(6);
    let max_offset = scroll.total_rows.saturating_sub(scroll.visible_rows).max(1);
    let max_y = body_top + body_height - thumb_height;
    let y = body_top + (scroll.offset.min(max_offset) * (max_y - body_top)) / max_offset;
    fill_rect(frame, 125, body_top, 2, body_height, track);
    fill_rect(frame, 125, y, 2, thumb_height, thumb);
}

fn render_runtime_error_frame(frame: &mut [u8], error: &OledRuntimeErrorMetadata, brightness: f32) {
    let warning = rgb565(scale(palette::RED, brightness));
    let dim_warning = rgb565(scale(dim(palette::RED, 6), brightness));
    let text = rgb565(scale(palette::GRAY, brightness));
    fill_rect(frame, 0, 0, 128, 128, dim_warning);
    fill_rect(
        frame,
        4,
        4,
        120,
        120,
        rgb565(scale(palette::BLACK, brightness)),
    );
    fill_rect(frame, 8, 8, 112, 18, warning);
    draw_text_clipped(
        frame,
        "RUNTIME ERROR",
        25,
        14,
        13,
        rgb565(scale(palette::BLACK, brightness)),
    );
    let lines = runtime_error_rows(error);
    for (index, line) in lines.iter().enumerate() {
        draw_text_clipped(frame, line, 10, 34 + index as i32 * 12, 18, text);
    }
}

fn overlay_toast(frame: &mut [u8], toast: &str, brightness: f32) {
    if toast.is_empty() {
        return;
    }
    fill_rect(
        frame,
        8,
        100,
        112,
        18,
        rgb565(scale(palette::BLACK, brightness)),
    );
    draw_text(
        frame,
        toast,
        12,
        105,
        1,
        rgb565(scale(palette::GRAY, brightness)),
    );
}
