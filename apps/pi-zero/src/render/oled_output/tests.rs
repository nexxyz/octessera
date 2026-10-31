use super::*;

#[cfg(test)]
mod recording_tests;

struct FakeOled {
    writes: Vec<Vec<u8>>,
    events: Vec<&'static str>,
    contrasts: Vec<u8>,
    fail_writes: usize,
}

impl FakeOled {
    fn new() -> Self {
        Self {
            writes: Vec::new(),
            events: Vec::new(),
            contrasts: Vec::new(),
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

    fn set_master_contrast(&mut self, step: u8) -> Result<(), String> {
        self.contrasts.push(step);
        Ok(())
    }
}

#[test]
fn force_render_writes_cached_revision_again_with_supplied_pixels() {
    let pixels = vec![0x2a; super::super::OLED_FRAME_BYTES];
    let publication = OledFramePublication::test_native(7, pixels.clone());
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();

    assert_eq!(
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, Instant::now()),
        None
    );
    force_oled_render_off(&mut oled, false, &publication, &mut cache).unwrap();
    assert_eq!(oled.writes, vec![pixels.clone(), pixels]);
}

#[test]
fn off_state_writes_frame_before_display_off_and_back_on() {
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
fn retry_and_force_keep_the_off_write_sequence() {
    let publication =
        OledFramePublication::test_native(12, vec![0x7c; super::super::OLED_FRAME_BYTES]);
    let mut oled = FakeOled::new();
    oled.fail_writes = 1;
    let mut cache = HardwareRenderCache::default();
    let due = render_oled_if_changed_off(&mut oled, true, &publication, &mut cache, Instant::now())
        .unwrap();
    assert_eq!(
        retry_oled_if_due_with_device(&mut oled, &mut cache, due),
        None
    );
    force_oled_render_off(&mut oled, true, &publication, &mut cache).unwrap();
    assert_eq!(
        oled.events,
        ["write_failed", "write", "off", "write", "off"]
    );
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
    assert!(second.revision() > first.revision());
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

static RECORDING_WRITER_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn wait_for_recording_writer_gate() {
    drop(RECORDING_WRITER_GATE.lock());
}

#[test]
fn colliding_source_revisions_through_terminal_advance_physical_frames() {
    let root = std::env::temp_dir().join(format!(
        "octessera-oled-transition-recording-{}",
        std::process::id()
    ));
    let writer_gate = RECORDING_WRITER_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (audio, _, _, _) = crate::audio::test_service_with_recorder(
        crate::audio_recording::RecordingServices::new(root.clone(), root.clone())
            .with_writer_start(wait_for_recording_writer_gate),
    );
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
        .submit_accepted_oled_frame(legacy.revision(), legacy.pixels())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 4_410]).unwrap();
    let native = physical_oled_publication(&native_source, &cache).unwrap();
    render_oled_if_changed_off(&mut oled, false, &native, &mut cache, Instant::now());
    audio
        .submit_accepted_oled_frame(native.revision(), native.pixels())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 4_410]).unwrap();
    let terminal = physical_oled_publication(&terminal_source, &cache).unwrap();
    force_oled_render_off(&mut oled, false, &terminal, &mut cache).unwrap();
    audio
        .submit_accepted_oled_frame(terminal.revision(), terminal.pixels())
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 4_410]).unwrap();
    drop(writer_gate);
    let recording = audio.stop_recording_with_outcome().unwrap().unwrap();

    assert!(legacy.revision() < native.revision());
    assert!(native.revision() < terminal.revision());
    assert_eq!(
        oled.writes,
        vec![
            vec![0x41; super::super::OLED_FRAME_BYTES],
            vec![0x52; super::super::OLED_FRAME_BYTES],
            vec![0x63; super::super::OLED_FRAME_BYTES],
        ]
    );
    assert!(oled_publication_is_accepted(&terminal, false, &cache));
    assert_eq!(recording.frames_written, 6_615);
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
        .submit_accepted_oled_frame(first.revision(), first.pixels())
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
            .submit_accepted_oled_frame(next.revision(), next.pixels())
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

#[test]
fn master_contrast_is_sent_once_per_change_without_rewriting_unchanged_pixels() {
    let pixels = vec![0x2a; super::super::OLED_FRAME_BYTES];
    let bright = OledFramePublication::test_native_with_contrast(7, pixels.clone(), 11);
    let dim = OledFramePublication::test_native_with_contrast(7, pixels.clone(), 5);
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    let now = Instant::now();

    for publication in [&bright, &bright, &dim, &dim] {
        assert_eq!(
            render_oled_if_changed_off(&mut oled, false, publication, &mut cache, now),
            None
        );
    }
    assert_eq!(oled.contrasts, vec![11, 5]);
    assert_eq!(oled.writes, vec![pixels]);
    assert!(oled_publication_is_accepted(&dim, false, &cache));
    assert!(!oled_publication_is_accepted(&bright, false, &cache));
}

#[test]
fn forced_restore_sends_the_master_contrast_again() {
    let publication = OledFramePublication::test_native_with_contrast(
        7,
        vec![0x2a; super::super::OLED_FRAME_BYTES],
        9,
    );
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();

    render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, Instant::now());
    force_oled_render_off(&mut oled, false, &publication, &mut cache).unwrap();
    assert_eq!(oled.contrasts, vec![9, 9]);
}
