use super::oled_output::{render_oled_if_changed_off, OledRenderDevice, OLED_RETRY_INTERVAL};
use super::*;
use std::time::{Duration, Instant};

struct TestOled {
    failed_writes: usize,
    writes: usize,
    frames: Vec<Vec<u8>>,
    operations: Vec<&'static str>,
}

impl OledRenderDevice for TestOled {
    fn display_on(&mut self) -> Result<(), String> {
        self.operations.push("on");
        Ok(())
    }

    fn write_frame(&mut self, _frame: &[u8]) -> Result<(), String> {
        self.operations.push("write");
        self.writes += 1;
        self.frames.push(_frame.to_vec());
        if self.failed_writes == 0 {
            Ok(())
        } else {
            self.failed_writes -= 1;
            Err("test OLED write failed".into())
        }
    }

    fn display_off(&mut self) -> Result<(), String> {
        self.operations.push("off");
        Ok(())
    }
}

#[test]
fn failed_oled_write_does_not_advance_revision_before_retry_success() {
    let now = Instant::now();
    let mut oled = TestOled {
        failed_writes: 1,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();

    let publication = OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]);
    let retry_at =
        render_oled_if_changed_off(&mut oled, true, &publication, &mut cache, now).unwrap();

    assert_eq!(cache.oled_rendered_key, None);
    assert_eq!(oled.writes, 1);
    assert_eq!(retry_at, now + OLED_RETRY_INTERVAL);
    assert_eq!(
        render_oled_if_changed_off(
            &mut oled,
            true,
            &publication,
            &mut cache,
            now + Duration::from_millis(1),
        ),
        Some(now + OLED_RETRY_INTERVAL)
    );
    assert_eq!(oled.writes, 1);

    assert_eq!(
        render_oled_if_changed_off(
            &mut oled,
            true,
            &publication,
            &mut cache,
            now + OLED_RETRY_INTERVAL,
        ),
        None
    );
    assert_eq!(
        cache.oled_rendered_key,
        Some(OledOutputKey::new(publication.revision(), true))
    );
    assert_eq!(oled.writes, 2);
    assert!(oled
        .frames
        .iter()
        .all(|frame| frame.iter().all(|byte| *byte == 0)));
}

#[test]
fn accepted_native_frame_is_written_without_semantic_rendering() {
    let pixels = vec![0x5a; OLED_FRAME_BYTES];
    let publication = OledFramePublication::test_native(7, pixels.clone());
    let mut oled = TestOled {
        failed_writes: 0,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();

    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, Instant::now(),),
        None
    );
    assert_eq!(oled.frames, vec![pixels]);
    assert_eq!(
        cache.oled_rendered_key,
        Some(OledOutputKey::new(publication.revision(), false))
    );
}

#[test]
fn newer_native_frame_supersedes_failed_retry_with_exact_bytes() {
    let old = OledFramePublication::test_native(1, vec![1; OLED_FRAME_BYTES]);
    let newer_pixels = vec![2; OLED_FRAME_BYTES];
    let newer = OledFramePublication::test_native(2, newer_pixels.clone());
    let now = Instant::now();
    let mut oled = TestOled {
        failed_writes: 1,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();
    assert!(render_oled_if_changed_off(&mut oled, false, &old, &mut cache, now).is_some());
    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &newer, &mut cache, now),
        None
    );
    assert_eq!(oled.frames.last(), Some(&newer_pixels));
    assert_eq!(
        cache.oled_rendered_key,
        Some(OledOutputKey::new(newer.revision(), false))
    );
}

#[test]
fn identical_black_frame_revision_reconciles_display_power_state() {
    let publication = OledFramePublication::test_native(7, vec![0; OLED_FRAME_BYTES]);
    let mut oled = TestOled {
        failed_writes: 0,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();
    let now = Instant::now();

    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, now),
        None
    );
    assert_eq!(
        render_oled_if_changed_off(&mut oled, true, &publication, &mut cache, now),
        None
    );
    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, now),
        None
    );

    assert_eq!(oled.operations, vec!["on", "write", "off", "on"]);
    assert_eq!(oled.writes, 1);
}

#[test]
fn retried_bytes_across_display_transitions_stay_immutable() {
    let pixels = vec![0x5a; OLED_FRAME_BYTES];
    let publication = OledFramePublication::test_native(7, pixels.clone());
    let mut oled = TestOled {
        failed_writes: 1,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();
    let now = Instant::now();

    assert!(render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, now).is_some());
    assert_eq!(
        render_oled_if_changed_off(
            &mut oled,
            true,
            &publication,
            &mut cache,
            now + OLED_RETRY_INTERVAL,
        ),
        None
    );
    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, now),
        None
    );

    assert_eq!(oled.operations, vec!["on", "write", "write", "off", "on"]);
    assert_eq!(oled.frames, vec![pixels.clone(), pixels]);
}
