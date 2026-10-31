use platform_core::palette;

use super::body_animation::draw_body_animation;
use super::error_layout::runtime_error_rows;
use super::footer::draw_footer;
use super::header::draw_header;
use super::menu_rows::draw_body;
use super::model::OledRuntimeErrorMetadata;
use super::pixels::{dim, fill_rect, rgb565};
use super::splash::render_splash;
use super::text::draw_text_clipped;
use super::{
    fit_line_ellipsis, OledPresentationInput, OledSplash, OLED_FRAME_BYTES,
    RUNTIME_ERROR_BODY_RECT, SPLASH_TOAST_RECT,
};

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
    if let Some(error) = input.runtime_error.as_ref() {
        if is_concise_runtime_error_presentation(error) {
            render_menu_frame(frame, input);
        } else {
            render_runtime_error_frame(frame, error);
        }
        return;
    }
    if input.display.splash != OledSplash::None {
        render_splash(frame, &input.display.splash);
        overlay_toast(frame, &input.display.toast);
        return;
    }
    render_menu_frame(frame, input);
}

fn is_concise_runtime_error_presentation(error: &OledRuntimeErrorMetadata) -> bool {
    error.domain.as_deref() == Some("midi")
        && error.operation.as_deref() == Some("midi_list_inputs")
}

fn render_menu_frame(frame: &mut [u8], input: &OledPresentationInput) {
    draw_header(frame, input);
    draw_body(
        frame,
        &input.display,
        input.selected_row,
        input.display.editing,
    );
    if let Some(animation) = &input.animation.body {
        draw_body_animation(frame, input, animation);
    }
    draw_footer(frame, input);
}

fn render_runtime_error_frame(frame: &mut [u8], error: &OledRuntimeErrorMetadata) {
    let warning = rgb565(palette::MAGENTA);
    let dim_warning = rgb565(dim(palette::MAGENTA, 6));
    let text = rgb565(palette::GRAY);
    fill_rect(frame, 0, 0, 128, 128, dim_warning);
    fill_rect(frame, 4, 4, 120, 120, rgb565(palette::BLACK));
    fill_rect(frame, 8, 8, 112, 18, warning);
    draw_text_clipped(frame, "RUNTIME ERROR", 25, 14, 13, rgb565(palette::BLACK));
    let lines = runtime_error_rows(error);
    for (index, line) in lines.iter().enumerate() {
        draw_text_clipped(
            frame,
            line,
            RUNTIME_ERROR_BODY_RECT.x as i32,
            (RUNTIME_ERROR_BODY_RECT.y + index * RUNTIME_ERROR_BODY_RECT.row_advance) as i32,
            RUNTIME_ERROR_BODY_RECT.columns(),
            text,
        );
    }
    let footer = fit_line_ellipsis("Back/Press: close", RUNTIME_ERROR_BODY_RECT.columns());
    draw_text_clipped(
        frame,
        &footer,
        RUNTIME_ERROR_BODY_RECT.x as i32,
        117,
        RUNTIME_ERROR_BODY_RECT.columns(),
        text,
    );
}

fn overlay_toast(frame: &mut [u8], toast: &str) {
    if toast.is_empty() {
        return;
    }
    fill_rect(frame, 8, 100, 112, 18, rgb565(palette::BLACK));
    let toast = fit_line_ellipsis(toast, SPLASH_TOAST_RECT.columns());
    draw_text_clipped(
        frame,
        &toast,
        SPLASH_TOAST_RECT.x as i32,
        105,
        SPLASH_TOAST_RECT.columns(),
        rgb565(palette::GRAY),
    );
}
