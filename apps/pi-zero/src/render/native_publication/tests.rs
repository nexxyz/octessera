use super::*;
use crate::render::oled_output::OledRenderDevice;
use crate::render::HardwareRenderCache;
use crate::render_loop_queue::NativeSceneCommand;
use playback_runtime::{NativeRunner, NativeRunnerConfig};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct OffRetryOled {
    writes: Vec<Vec<u8>>,
    off_failures: usize,
}

impl OledRenderDevice for OffRetryOled {
    fn display_on(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.writes.push(frame.to_vec());
        Ok(())
    }

    fn display_off(&mut self) -> Result<(), String> {
        if self.off_failures > 0 {
            self.off_failures -= 1;
            Err("injected display-off failure".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn state_only_display_off_retry_completes_generation_once_without_rewriting_pixels() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let scene = runner.capture_display_scene().unwrap();
    let generation = scene.generation();
    let (completion, received) = mpsc::channel();
    let mut latest = prepare_native_scene(
        Box::new(NativeSceneCommand {
            scene,
            metrics: playback_runtime::oled_frame::OledPresentationMetrics::default(),
            error: None,
            completion,
        }),
        &HardwareRenderCache::default(),
    )
    .unwrap();
    let (pixels, off) = match &mut latest {
        LatestPresentation::Native { hardware, oled, .. } => {
            hardware.leds.off = true;
            (oled.clone(), hardware.leds.off)
        }
        LatestPresentation::Legacy { .. } => unreachable!(),
    };
    let mut cache = HardwareRenderCache::default();
    let mut oled = OffRetryOled {
        writes: Vec::new(),
        off_failures: 1,
    };
    let start = Instant::now();

    let due = crate::render::oled_output::render_oled_if_changed_off(
        &mut oled, off, &pixels, &mut cache, start,
    )
    .unwrap();
    assert!(!latest.oled_is_accepted(&cache));
    assert!(received.try_recv().is_err());

    assert_eq!(
        crate::render::oled_output::render_oled_if_changed_off(
            &mut oled,
            off,
            &pixels,
            &mut cache,
            due + Duration::from_millis(1),
        ),
        None
    );
    assert!(latest.oled_is_accepted(&cache));
    latest.complete_native(Ok(()));
    latest.complete_native(Ok(()));

    let completion = received.try_recv().unwrap();
    assert_eq!(completion.generation, generation);
    assert!(completion.result.is_ok());
    assert!(completion.frame_revision.is_some());
    assert!(received.try_recv().is_err());
    assert_eq!(oled.writes.len(), 1);
}
