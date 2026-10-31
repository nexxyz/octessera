mod aux_cap;
mod body_animation;
mod error_layout;
mod font;
mod font_5x7;
mod font_m5x7;
mod font_tom_thumb;
mod footer;
mod header;
mod menu_rows;
mod model;
mod pixels;
mod presentation_input;
mod render;
mod splash;
mod text;
mod text_layout;

pub const OLED_WIDTH: usize = 128;
pub const OLED_HEIGHT: usize = 128;
pub const OLED_FRAME_BYTES: usize = OLED_WIDTH * OLED_HEIGHT * 2;

pub use error_layout::{runtime_error_rows, ERROR_ROW_COUNT, ERROR_ROW_WIDTH};
pub(crate) use header::breadcrumb_fits;
pub(crate) use menu_rows::menu_row_label_overflows;
pub use model::{
    OledAnimationInput, OledAuxCap, OledAuxHint, OledAuxHints, OledBarInput, OledBarStyle,
    OledBodyAnimation, OledBodyFrom, OledDisplayInput, OledHintLabel, OledHintSource,
    OledMenuRowInput, OledMenuRowKind, OledPresentationInput, OledPresentationMetrics,
    OledRuntimeErrorMetadata, OledSaveFlash, OledScrollInput, OledSplash, OledToastSlide,
    OledTransportFlash, OledTransportIcon, OledTransportInput, INDICATOR_FADE_LEVELS,
};
pub(crate) use presentation_input::presentation_input_from_snapshot;
pub use render::render_oled_frame;
pub(crate) use render::render_oled_frame_into;
pub use text_layout::{
    fit_line_ellipsis, force_line_ellipsis, layout_card_body, layout_rows, normalize_text,
    wrap_text, LaidOutTextRow, OledDisplayLayout, TextLayoutRect, CARD_BODY_RECT, FONT_ADVANCE_X,
    FONT_GLYPH_HEIGHT, FONT_GLYPH_WIDTH, MENU_BODY_RECT, RUNTIME_ERROR_BODY_RECT,
    SPLASH_TOAST_RECT, TOAST_RECT,
};

#[cfg(test)]
mod body_animation_tests;

#[cfg(test)]
mod error_layout_tests;

#[cfg(test)]
mod glyph_placement_tests;

#[cfg(test)]
mod indicator_tests;

#[cfg(test)]
mod menu_frame_tests;

#[cfg(test)]
mod render_bench_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod text_layout_tests;

#[cfg(feature = "test-support")]
pub mod test_support {
    pub const BOOT_SPLASH_RGB565: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/splash_boot.rgb565"));
    pub const SLEEP_SHUTDOWN_SPLASH_RGB565: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/splash_sleep_shutdown.rgb565"));
}
