use super::*;
use crate::render::OLED_FRAME_BYTES;
use std::sync::Arc;

#[test]
fn failed_frame_is_not_recorded_and_retry_records_one_physical_frame() {
    let root = std::env::temp_dir().join(format!(
        "octessera-oled-retry-recording-{}",
        std::process::id()
    ));
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(root.clone());
    audio.start_recording_audio_oled_with_seed(1, None).unwrap();
    let mut cache = HardwareRenderCache::default();
    let mut oled = FakeOled::new();
    oled.fail_writes = 1;
    let pixels = vec![0x74; OLED_FRAME_BYTES];
    let publication = physical_oled_publication(
        &OledFramePublication::test_native(2, pixels.clone()),
        &cache,
    )
    .unwrap();
    let now = Instant::now();
    let retry_at =
        render_oled_if_changed_off(&mut oled, false, &publication, &mut cache, now).unwrap();

    assert_eq!(cache.oled_render_count(), 0);
    assert!(audio.latest_physical_oled_frame().is_none());
    assert!(oled.writes.is_empty());

    assert_eq!(
        retry_oled_if_due_with_device(&mut oled, &mut cache, retry_at),
        None
    );
    assert_eq!(cache.oled_render_count(), 1);
    let accepted = (
        publication.revision().unwrap(),
        publication.shared_pixels().unwrap(),
    );
    audio
        .submit_accepted_oled_frame_shared(accepted.0, Arc::clone(&accepted.1))
        .unwrap();
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let recording = audio.stop_recording_with_outcome().unwrap().unwrap();
    let (latest_revision, latest_pixels) = audio.latest_physical_oled_frame().unwrap();

    assert_eq!(latest_revision, accepted.0);
    assert_eq!(latest_pixels.as_ref(), pixels);
    assert_eq!(oled.writes, vec![pixels]);
    assert!(!avi_video_payloads(&recording.path).is_empty());
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
