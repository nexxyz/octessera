use super::*;
use media_recording::{RecordingStatus, OLED_FRAME_BYTES};
use playback_runtime::RuntimeStoreResult;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn audio_oled_submission_uses_final_tap_cursor_and_rejects_duplicate_revision() {
    let directory = temp_dir();
    let (service, _, _, _) = test_service_with_recording_dir(directory.clone());
    service
        .start_recording_audio_oled_with_seed(1, None)
        .unwrap();
    service
        .submit_accepted_oled_frame(1, &solid_frame(0xf800))
        .unwrap();
    push_frames(&service, 4_410);
    service
        .submit_accepted_oled_frame(1, &solid_frame(0x001f))
        .unwrap();
    service
        .submit_accepted_oled_frame(2, &solid_frame(0x07e0))
        .unwrap();
    push_frames(&service, 4_410);
    service.stop_recording().unwrap();
    assert!(service.poll_recording_status().is_none());

    let path = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "avi"))
        .unwrap();
    let videos = avi_video_payloads(&path);
    assert_eq!(videos.len(), 2);
    assert_ne!(videos[0], videos[1]);

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn audio_oled_start_seeds_the_accepted_frame_before_audio_advances() {
    let directory = temp_dir();
    let (service, _, _, _) = test_service_with_recording_dir(directory.clone());
    let red = solid_frame(0xf800);
    service
        .start_recording_audio_oled_with_seed(1, Some((1, red.clone())))
        .unwrap();
    push_frames(&service, 4_410);
    service.stop_recording().unwrap();

    let seeded_path = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "avi"))
        .unwrap();
    let blank_directory = temp_dir();
    let (blank_service, _, _, _) = test_service_with_recording_dir(blank_directory.clone());
    blank_service
        .start_recording_audio_oled_with_seed(1, None)
        .unwrap();
    push_frames(&blank_service, 4_410);
    blank_service.stop_recording().unwrap();
    let blank_path = fs::read_dir(&blank_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "avi"))
        .unwrap();
    assert_ne!(
        avi_video_payloads(&seeded_path)[0],
        avi_video_payloads(&blank_path)[0]
    );

    let _ = fs::remove_dir_all(directory);
    let _ = fs::remove_dir_all(blank_directory);
}

#[test]
fn audio_and_audio_oled_starts_share_one_active_recording() {
    let directory = temp_dir();
    let (service, _, _, _) = test_service_with_recording_dir(directory.clone());
    service
        .start_recording_audio_oled_with_seed(1, None)
        .unwrap();
    assert!(service
        .start_recording(1)
        .unwrap_err()
        .contains("already active"));
    service.stop_recording().unwrap();

    service.start_recording(1).unwrap();
    assert!(service
        .start_recording_audio_oled_with_seed(1, None)
        .unwrap_err()
        .contains("already active"));
    service.stop_recording().unwrap();
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn recording_completion_uses_existing_status_toast_result() {
    assert!(matches!(
        super::super::audio_recording::recording_status(RecordingStatus::Complete),
        RuntimeStoreResult::RecordingStatus { ok: true, message }
            if message == "Recording saved"
    ));
    assert!(matches!(
        super::super::audio_recording::recording_status(RecordingStatus::Incomplete {
            gap_count: 0,
            gap_frames: 1,
            overflow_count: 0,
            overflow_frames: 0,
        }),
        RuntimeStoreResult::RecordingStatus { ok: false, message }
            if message == "Recording incomplete"
    ));
}

fn push_frames(service: &AudioService, count: usize) {
    let tap = service.recording_tap.read().unwrap().clone().unwrap();
    for index in 0..count {
        tap.push_frame(index as i16, -(index as i16));
    }
}

fn solid_frame(color: u16) -> Vec<u8> {
    let bytes = color.to_be_bytes();
    let mut pixels = vec![0; OLED_FRAME_BYTES];
    let (pixel_pairs, _) = pixels.as_chunks_mut::<2>();
    for pixel in pixel_pairs {
        pixel.copy_from_slice(&bytes);
    }
    pixels
}

fn avi_video_payloads(path: &Path) -> Vec<Vec<u8>> {
    let bytes = fs::read(path).unwrap();
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

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-pi-audio-oled-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
