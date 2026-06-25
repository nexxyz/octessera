use std::time::Duration;

#[derive(Default)]
pub struct RenderProfileMetrics {
    pub led_extract: Duration,
    pub led_write: Duration,
    pub neokey_build: Duration,
    pub neokey_write: Duration,
    pub oled_signature: Duration,
    pub oled_frame_build: Duration,
    pub oled_write: Duration,
    pub oled_rendered: bool,
}
