use super::model::OledSplash;
use super::OLED_FRAME_BYTES;

pub(super) const SPLASH_BOOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/splash_boot.rgb565"));
pub(super) const SPLASH_SLEEP_SHUTDOWN: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/splash_sleep_shutdown.rgb565"));

pub(super) fn render_splash(frame: &mut [u8], splash: &OledSplash) {
    let source = match splash {
        OledSplash::Sleep | OledSplash::Shutdown => SPLASH_SLEEP_SHUTDOWN,
        OledSplash::None | OledSplash::Boot => SPLASH_BOOT,
    };
    debug_assert_eq!(source.len(), OLED_FRAME_BYTES);
    frame.copy_from_slice(source);
}
