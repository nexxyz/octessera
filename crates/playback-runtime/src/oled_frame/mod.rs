mod error_layout;
mod font;
mod footer;
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
pub use model::{
    OledBarInput, OledBarStyle, OledDisplayInput, OledPresentationInput, OledPresentationMetrics,
    OledRuntimeErrorMetadata, OledSaveFlash, OledScrollInput, OledSplash, OledTransportFlash,
    OledTransportIcon, OledTransportInput,
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
#[path = "error_layout_tests.rs"]
mod error_layout_tests;

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
