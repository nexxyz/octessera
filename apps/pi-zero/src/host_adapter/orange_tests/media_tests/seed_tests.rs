use super::*;
use crate::audio::test_service_with_recording_dir;
use media_recording::OLED_FRAME_BYTES;
use playback_runtime::RunnerMessage;
use std::sync::Arc;

#[test]
fn orange_audio_oled_start_seeds_latest_physical_frame_not_legacy_cache() {
    let (store, samples) = directories();
    let actual_dir = store.parent().unwrap().join("actual-recording");
    let reference_dir = store.parent().unwrap().join("reference-recording");
    let (audio, _, _, _) = test_service_with_recording_dir(actual_dir.clone());
    let stale = vec![0x11; OLED_FRAME_BYTES];
    let current = vec![0x66; OLED_FRAME_BYTES];
    audio
        .submit_accepted_oled_frame_shared(42, Arc::from(current.clone()))
        .unwrap();
    let mut adapter = PiHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    adapter.core.ingest_oled_frame(&RunnerMessage::OledFrame {
        revision: 1,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels: stale,
    });
    adapter
        .core
        .accept_oled_frame_reference(&serde_json::json!({"oledFrameRevision": 1}));

    let started = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::RecordingStartAudioOled { max_minutes: 1 },
            "orange-oled-seed",
        ))
        .unwrap();
    assert!(matches!(
        started.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus { ok: true, .. }
        }]
    ));
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let actual = audio.stop_recording_with_outcome().unwrap().unwrap();

    let (reference, _, _, _) = test_service_with_recording_dir(reference_dir.clone());
    reference
        .start_recording_audio_oled_with_seed(1, Some((42, current)))
        .unwrap();
    reference
        .test_push_recording_samples(&vec![0; 8_820])
        .unwrap();
    let expected = reference.stop_recording_with_outcome().unwrap().unwrap();
    assert_eq!(
        avi_video_payloads(&actual.path)[0],
        avi_video_payloads(&expected.path)[0]
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
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
