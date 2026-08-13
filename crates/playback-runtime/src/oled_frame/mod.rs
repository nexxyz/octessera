mod font;
mod footer;
mod model;
mod pixels;
mod presentation_input;
mod render;
mod splash;
mod text;

pub const OLED_WIDTH: usize = 128;
pub const OLED_HEIGHT: usize = 128;
pub const OLED_FRAME_BYTES: usize = OLED_WIDTH * OLED_HEIGHT * 2;

pub use model::{
    OledBarInput, OledBarStyle, OledDisplayInput, OledPresentationInput, OledPresentationMetrics,
    OledRuntimeErrorMetadata, OledSaveFlash, OledScrollInput, OledSplash, OledTransportFlash,
    OledTransportIcon, OledTransportInput,
};
pub(crate) use presentation_input::presentation_input_from_snapshot;
pub use render::render_oled_frame;
pub(crate) use render::render_oled_frame_into;

#[cfg(test)]
mod tests;

#[cfg(feature = "test-support")]
pub mod test_support {
    pub const BOOT_SPLASH_RGB565: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/splash_boot.rgb565"));
    pub const SLEEP_SHUTDOWN_SPLASH_RGB565: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/splash_sleep_shutdown.rgb565"));
}
