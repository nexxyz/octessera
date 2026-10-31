use super::oled_output::{render_oled_if_changed_off, OledRenderDevice};
use super::*;
use playback_runtime::oled_frame::OLED_FRAME_BYTES;

#[test]
fn native_frame_sleep_wake_rewrites_newer_frame() {
    let initial_pixels = vec![0x11; OLED_FRAME_BYTES];
    let rewritten_pixels = vec![0x22; OLED_FRAME_BYTES];
    let initial = OledFramePublication::test_native(1, initial_pixels.clone());
    let rewritten = OledFramePublication::test_native(2, rewritten_pixels.clone());
    let mut oled = StartupOled::default();
    let mut render_cache = HardwareRenderCache::default();
    let now = std::time::Instant::now();

    assert!(
        render_oled_if_changed_off(&mut oled, false, &initial, &mut render_cache, now).is_none()
    );
    assert!(
        render_oled_if_changed_off(&mut oled, true, &initial, &mut render_cache, now).is_none()
    );
    assert!(
        render_oled_if_changed_off(&mut oled, false, &rewritten, &mut render_cache, now).is_none()
    );

    assert_eq!(
        oled.operations,
        vec![
            "display_on",
            "write_frame",
            "display_off",
            "display_on",
            "write_frame"
        ]
    );
    assert_eq!(oled.frames, vec![initial_pixels, rewritten_pixels]);
}

#[derive(Default)]
struct StartupOled {
    operations: Vec<&'static str>,
    frames: Vec<Vec<u8>>,
}

impl OledRenderDevice for StartupOled {
    fn display_on(&mut self) -> Result<(), String> {
        self.operations.push("display_on");
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.operations.push("write_frame");
        self.frames.push(frame.to_vec());
        Ok(())
    }

    fn display_off(&mut self) -> Result<(), String> {
        self.operations.push("display_off");
        Ok(())
    }

    fn set_master_contrast(&mut self, _step: u8) -> Result<(), String> {
        Ok(())
    }
}
