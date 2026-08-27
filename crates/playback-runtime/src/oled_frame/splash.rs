use super::model::OledSplash;
use super::pixels::{rgb565, rgb565_to_rgb, scale};
use super::OLED_FRAME_BYTES;

pub(super) const SPLASH_BOOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/splash_boot.rgb565"));
pub(super) const SPLASH_SLEEP_SHUTDOWN: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/splash_sleep_shutdown.rgb565"));

pub(super) fn render_splash(frame: &mut [u8], splash: &OledSplash, brightness: f32) {
    let source = match splash {
        OledSplash::Sleep | OledSplash::Shutdown => SPLASH_SLEEP_SHUTDOWN,
        OledSplash::None | OledSplash::Boot => SPLASH_BOOT,
    };
    copy_rgb565_scaled(frame, source, brightness);
}

fn copy_rgb565_scaled(frame: &mut [u8], source: &[u8], brightness: f32) {
    debug_assert_eq!(source.len(), OLED_FRAME_BYTES);
    if brightness >= 0.999 {
        frame.copy_from_slice(source);
        return;
    }
    let (chunks, remainder) = source.as_chunks::<2>();
    debug_assert!(remainder.is_empty());
    for (index, chunk) in chunks.iter().enumerate() {
        let color = u16::from_be_bytes([chunk[0], chunk[1]]);
        let scaled = rgb565(scale(rgb565_to_rgb(color), brightness));
        let offset = index * 2;
        frame[offset] = (scaled >> 8) as u8;
        frame[offset + 1] = scaled as u8;
    }
}
