use super::*;
use crate::oled_frame_cache::OledFramePublication;
use serde_json::json;

#[cfg(test)]
#[path = "oled_retry_recording_tests.rs"]
mod recording_tests;

struct FakeOled {
    writes: Vec<Vec<u8>>,
    events: Vec<&'static str>,
    fail_writes: usize,
}

impl FakeOled {
    fn new() -> Self {
        Self {
            writes: Vec::new(),
            events: Vec::new(),
            fail_writes: 0,
        }
    }
}

impl OledRenderDevice for FakeOled {
    fn display_on(&mut self) -> Result<(), String> {
        self.events.push("on");
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        if self.fail_writes > 0 {
            self.fail_writes -= 1;
            self.events.push("write_failed");
            return Err("test write failure".into());
        }
        self.events.push("write");
        self.writes.push(frame.to_vec());
        Ok(())
    }

    fn display_off(&mut self) -> Result<(), String> {
        self.events.push("off");
        Ok(())
    }
}

#[test]
fn force_render_writes_cached_revision_again_with_supplied_pixels() {
    let snapshot = json!({"display": { "off": false }, "oledFrameRevision": 7});
    let pixels = vec![0x2a; super::super::OLED_FRAME_BYTES];
    let publication = OledFramePublication::test_native(7, pixels.clone());
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();

    assert_eq!(
        render_oled_if_changed(
            &mut oled,
            &snapshot,
            &publication,
            &mut cache,
            Instant::now(),
        ),
        None
    );
    force_oled_render_with_device(&mut oled, &snapshot, &publication, &mut cache).unwrap();
    assert_eq!(oled.writes, vec![pixels.clone(), pixels]);
}

#[test]
fn typed_off_state_uses_same_frame_order_and_retry_cache_as_snapshot_path() {
    let publication =
        OledFramePublication::test_native(9, vec![0x5a; super::super::OLED_FRAME_BYTES]);
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    assert_eq!(
        render_oled_if_changed_off(&mut oled, true, &publication, &mut cache, Instant::now()),
        None
    );
    assert_eq!(cache.oled_output_state.display_off, Some(true));
    assert_eq!(oled.writes.len(), 1);
    assert_eq!(
        render_oled_if_changed_off(&mut oled, true, &publication, &mut cache, Instant::now()),
        None
    );
    assert_eq!(oled.writes.len(), 1);
    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, Instant::now()),
        None
    );
    assert_eq!(cache.oled_output_state.display_off, Some(false));
    assert_eq!(oled.events, ["write", "off", "on"]);
}

#[test]
fn typed_retry_and_force_preserve_legacy_write_sequence_without_retry_json() {
    let publication =
        OledFramePublication::test_native(12, vec![0x7c; super::super::OLED_FRAME_BYTES]);
    let snapshot = json!({"display": {"off": true}});
    let (mut typed, mut legacy) = (FakeOled::new(), FakeOled::new());
    typed.fail_writes = 1;
    legacy.fail_writes = 1;
    let (mut typed_cache, mut legacy_cache) = (
        HardwareRenderCache::default(),
        HardwareRenderCache::default(),
    );
    let start = Instant::now();
    let due = render_oled_if_changed_off(&mut typed, true, &publication, &mut typed_cache, start)
        .unwrap();
    let old_due = render_oled_if_changed(
        &mut legacy,
        &snapshot,
        &publication,
        &mut legacy_cache,
        start,
    )
    .unwrap();
    assert_eq!(due, old_due);
    assert_eq!(typed.events, legacy.events);
    assert_eq!(
        retry_oled_if_due_with_device(&mut typed, &mut typed_cache, due),
        None
    );
    assert_eq!(
        retry_oled_if_due_with_device(&mut legacy, &mut legacy_cache, due),
        None
    );
    force_oled_render_off(&mut typed, true, &publication, &mut typed_cache).unwrap();
    force_oled_render_with_device(&mut legacy, &snapshot, &publication, &mut legacy_cache).unwrap();
    assert_eq!(typed.events, legacy.events);
    assert_eq!(
        typed.events,
        ["write_failed", "write", "off", "write", "off"]
    );
    assert_eq!(typed.writes, legacy.writes);
}

#[test]
fn colliding_source_revisions_with_different_pixels_write_both_frames() {
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    let first_source =
        OledFramePublication::test_native(2, vec![0x11; super::super::OLED_FRAME_BYTES]);
    let second_source =
        OledFramePublication::test_native(2, vec![0x22; super::super::OLED_FRAME_BYTES]);
    let first = physical_oled_publication(&first_source, &cache).unwrap();

    render_oled_if_changed_off(&mut oled, false, &first, &mut cache, Instant::now());
    let second = physical_oled_publication(&second_source, &cache).unwrap();
    render_oled_if_changed_off(&mut oled, false, &second, &mut cache, Instant::now());

    assert_eq!(
        oled.writes,
        vec![
            vec![0x11; super::super::OLED_FRAME_BYTES],
            vec![0x22; super::super::OLED_FRAME_BYTES]
        ]
    );
    assert!(second.revision().unwrap() > first.revision().unwrap());
}

#[test]
fn changed_source_revision_with_identical_pixels_does_not_rewrite_frame() {
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    let pixels = vec![0x33; super::super::OLED_FRAME_BYTES];
    let first_source = OledFramePublication::test_native(2, pixels.clone());
    let next_source = OledFramePublication::test_native(3, pixels.clone());
    let first = physical_oled_publication(&first_source, &cache).unwrap();

    render_oled_if_changed_off(&mut oled, false, &first, &mut cache, Instant::now());
    let next = physical_oled_publication(&next_source, &cache).unwrap();
    render_oled_if_changed_off(&mut oled, false, &next, &mut cache, Instant::now());

    assert_eq!(oled.writes, vec![pixels]);
    assert_eq!(next.revision(), first.revision());
}

#[test]
fn legacy_native_terminal_source_collisions_advance_physical_frames() {
    let root = std::env::temp_dir().join(format!(
        "octessera-oled-transition-recording-{}",
        std::process::id()
    ));
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.clone());
    audio.start_recording_audio_oled_with_seed(1, None).unwrap();
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    let legacy_source =
        OledFramePublication::test_native(2, vec![0x41; super::super::OLED_FRAME_BYTES]);
    let native_source =
        OledFramePublication::test_native(2, vec![0x52; super::super::OLED_FRAME_BYTES]);
    let terminal_source =
        OledFramePublication::test_native(2, vec![0x63; super::super::OLED_FRAME_BYTES]);
    let legacy = physical_oled_publication(&legacy_source, &cache).unwrap();
    render_oled_if_changed_off(&mut oled, false, &legacy, &mut cache, Instant::now());
    audio
        .submit_accepted_oled_frame(legacy.revision().unwrap(), legacy.pixels().unwrap())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let native = physical_oled_publication(&native_source, &cache).unwrap();
    render_oled_if_changed_off(&mut oled, false, &native, &mut cache, Instant::now());
    audio
        .submit_accepted_oled_frame(native.revision().unwrap(), native.pixels().unwrap())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let terminal = physical_oled_publication(&terminal_source, &cache).unwrap();
    force_oled_render_off(&mut oled, false, &terminal, &mut cache).unwrap();
    audio
        .submit_accepted_oled_frame(terminal.revision().unwrap(), terminal.pixels().unwrap())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let recording = audio.stop_recording_with_outcome().unwrap().unwrap();

    assert!(legacy.revision().unwrap() < native.revision().unwrap());
    assert!(native.revision().unwrap() < terminal.revision().unwrap());
    assert_eq!(
        oled.writes,
        vec![
            vec![0x41; super::super::OLED_FRAME_BYTES],
            vec![0x52; super::super::OLED_FRAME_BYTES],
            vec![0x63; super::super::OLED_FRAME_BYTES],
        ]
    );
    assert!(oled_publication_is_accepted(&terminal, false, &cache));
    assert_eq!(recording.frames_written, 13_230);
    let videos = avi_video_payloads(&recording.path);
    assert_eq!(videos.len(), 3);
    assert_ne!(videos[0], videos[1]);
    assert_ne!(videos[1], videos[2]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unchanged_pixels_keep_one_recording_frame_and_physical_write() {
    let root = std::env::temp_dir().join(format!(
        "octessera-oled-unchanged-recording-{}",
        std::process::id()
    ));
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.clone());
    audio.start_recording_audio_oled_with_seed(1, None).unwrap();
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    let pixels = vec![0x39; super::super::OLED_FRAME_BYTES];
    let first = physical_oled_publication(
        &OledFramePublication::test_native(2, pixels.clone()),
        &cache,
    )
    .unwrap();
    render_oled_if_changed_off(&mut oled, false, &first, &mut cache, Instant::now());
    audio
        .submit_accepted_oled_frame(first.revision().unwrap(), first.pixels().unwrap())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();

    let next = physical_oled_publication(
        &OledFramePublication::test_native(3, pixels.clone()),
        &cache,
    )
    .unwrap();
    render_oled_if_changed_off(&mut oled, false, &next, &mut cache, Instant::now());
    if cache.oled_render_count() > 1 {
        audio
            .submit_accepted_oled_frame(next.revision().unwrap(), next.pixels().unwrap())
            .unwrap();
    }
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let recording = audio.stop_recording_with_outcome().unwrap().unwrap();
    let videos = avi_video_payloads(&recording.path);

    assert_eq!(first.revision(), next.revision());
    assert_eq!(oled.writes, vec![pixels]);
    assert_eq!(cache.oled_render_count(), 1);
    assert!(!videos.is_empty());
    assert!(videos.iter().all(|frame| frame == &videos[0]));
    let _ = std::fs::remove_dir_all(root);
}

fn avi_video_payloads(path: &std::path::Path) -> Vec<Vec<u8>> {
    let bytes = std::fs::read(path).unwrap();
    let list = bytes
        .windows(12)
        .position(|window| &window[..4] == b"LIST" && &window[8..12] == b"movi")
        .unwrap();
    let mut offset = list + 12;
    let end = list + 8 + u32::from_le_bytes(bytes[list + 4..list + 8].try_into().unwrap()) as usize;
    let mut videos = Vec::new();
    while offset + 8 <= end {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let payload_end = offset + 8 + size;
        if &bytes[offset..offset + 4] == b"00dc" {
            videos.push(bytes[offset + 8..payload_end].to_vec());
        }
        offset = payload_end + size % 2;
    }
    videos
}
