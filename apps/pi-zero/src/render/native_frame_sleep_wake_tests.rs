use super::oled_output::{render_oled_if_changed, OledRenderDevice};
use super::*;
use crate::oled_frame_cache::{OledFrameCache, OledFramePublication};
use playback_runtime::oled_frame::OLED_FRAME_BYTES;
use playback_runtime::RunnerMessage;

#[test]
fn accepted_native_frame_sleep_wake_rewrites_newer_frame() {
    let initial_pixels = vec![0x11; OLED_FRAME_BYTES];
    let rewritten_pixels = vec![0x22; OLED_FRAME_BYTES];
    let mut frame_cache = OledFrameCache::default();
    let initial_snapshot = startup_snapshot(false, 1);
    let initial_publication = accept_native_frame(
        &mut frame_cache,
        &initial_snapshot,
        1,
        initial_pixels.clone(),
    );
    assert_eq!(frame_cache.accepted_frame().unwrap().revision(), 1);
    assert_eq!(initial_publication.revision(), Some(1));
    let mut oled = StartupOled::default();
    let mut render_cache = HardwareRenderCache::default();

    assert!(render_oled_if_changed(
        &mut oled,
        &initial_snapshot,
        &initial_publication,
        &mut render_cache,
        std::time::Instant::now(),
    )
    .is_none());

    let sleeping_snapshot = startup_snapshot(true, 1);
    let sleeping_publication = frame_cache
        .publication_for_snapshot(&sleeping_snapshot, false)
        .unwrap();
    assert_eq!(sleeping_publication.revision(), Some(1));
    assert!(render_oled_if_changed(
        &mut oled,
        &sleeping_snapshot,
        &sleeping_publication,
        &mut render_cache,
        std::time::Instant::now(),
    )
    .is_none());

    let awake_snapshot = startup_snapshot(false, 2);
    let rewritten_publication = accept_native_frame(
        &mut frame_cache,
        &awake_snapshot,
        2,
        rewritten_pixels.clone(),
    );
    assert_eq!(rewritten_publication.revision(), Some(2));
    assert!(render_oled_if_changed(
        &mut oled,
        &awake_snapshot,
        &rewritten_publication,
        &mut render_cache,
        std::time::Instant::now(),
    )
    .is_none());

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
    assert_eq!(frame_cache.accepted_frame().unwrap().revision(), 2);
}

fn startup_snapshot(display_off: bool, revision: u64) -> serde_json::Value {
    serde_json::json!({
        "display": { "off": display_off },
        "oledFrameRevision": revision,
    })
}

fn accept_native_frame(
    cache: &mut OledFrameCache,
    snapshot: &serde_json::Value,
    revision: u64,
    pixels: Vec<u8>,
) -> OledFramePublication {
    cache.ingest(&RunnerMessage::OledFrame {
        revision,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels,
    });
    cache.accept_reference_value(snapshot);
    cache.publication_for_snapshot(snapshot, true).unwrap()
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
}
