use super::*;
use media_recording::{RecordingStatus, OLED_FRAME_BYTES};
use rodio_engine_source::{event_queue, EngineEvent};
use std::fs;
use std::sync::{mpsc, Barrier};
use std::time::Duration;

#[test]
fn final_engine_samples_are_tapped_once_as_stereo_i16() {
    let directory = temp_dir("source");
    let recording = DesktopRecording::new(directory.clone());
    recording.start_audio(1).unwrap();
    let (events, controls) = event_queue();
    events
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 1_000,
        })
        .unwrap();
    let source = EngineSource::with_block_frames(controls, 44_100, 32);
    let mut source = RecordingEngineSource::new(source, recording.tap_state());
    let output: Vec<f32> = (0..64).map(|_| source.next().unwrap()).collect();
    let outcome = recording.stop_audio().unwrap().unwrap();

    assert_eq!(outcome.frames_written, 32);
    assert_eq!(outcome.status, RecordingStatus::Complete);
    let bytes = fs::read(outcome.path).unwrap();
    let recorded: Vec<i16> = bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|sample| i16::from_le_bytes(*sample))
        .collect();
    let expected: Vec<i16> = output
        .as_chunks::<2>()
        .0
        .iter()
        .flat_map(|frame| [float_to_i16(frame[0]), float_to_i16(frame[1])])
        .collect();
    assert_eq!(recorded, expected);
    remove_temp_dir(directory);
}

#[test]
fn stop_is_harmless_when_idle() {
    let directory = temp_dir("idle");
    let recording = DesktopRecording::new(directory.clone());
    assert!(recording.stop_audio().unwrap().is_none());
    remove_temp_dir(directory);
}

#[test]
fn audio_oled_submission_uses_cursor_and_deduplicates_revision() {
    let directory = temp_dir("source-oled");
    let recording = DesktopRecording::new(directory.clone());
    recording.start_audio_oled(1).unwrap();
    let red = solid_frame(0xf800);
    let blue = solid_frame(0x001f);
    let green = solid_frame(0x07e0);
    recording.submit_accepted_oled_frame(1, &red).unwrap();
    let tap = recording.tap.read().unwrap().clone().unwrap();
    for _ in 0..4_410 {
        tap.push_frame(1, 2);
    }
    recording.submit_accepted_oled_frame(1, &blue).unwrap();
    recording.submit_accepted_oled_frame(2, &green).unwrap();
    for _ in 0..4_410 {
        tap.push_frame(3, 4);
    }
    let outcome = recording.stop_audio().unwrap().unwrap();
    let videos = avi_video_payloads(&outcome.path);
    assert_eq!(videos.len(), 2);
    assert_ne!(videos[0], videos[1]);
    remove_temp_dir(directory);
}

#[test]
fn audio_oled_start_seeds_the_retained_accepted_frame_before_audio_advances() {
    let seeded_directory = temp_dir("seeded");
    let seeded = DesktopRecording::new(seeded_directory.clone());
    let red = solid_frame(0xf800);
    seeded.remember_accepted_oled_frame(1, &red).unwrap();
    seeded.start_audio_oled(1).unwrap();
    let tap = seeded.tap.read().unwrap().clone().unwrap();
    for _ in 0..4_410 {
        tap.push_frame(1, 2);
    }
    let seeded_outcome = seeded.stop_audio().unwrap().unwrap();

    let blank_directory = temp_dir("blank");
    let blank = DesktopRecording::new(blank_directory.clone());
    blank.start_audio_oled(1).unwrap();
    let tap = blank.tap.read().unwrap().clone().unwrap();
    for _ in 0..4_410 {
        tap.push_frame(1, 2);
    }
    let blank_outcome = blank.stop_audio().unwrap().unwrap();

    assert_ne!(
        avi_video_payloads(&seeded_outcome.path)[0],
        avi_video_payloads(&blank_outcome.path)[0]
    );
    remove_temp_dir(seeded_directory);
    remove_temp_dir(blank_directory);
}

#[test]
fn repeated_rejected_starts_do_not_interrupt_a_flowing_active_take() {
    let directory = temp_dir("rejected-start");
    let recording = DesktopRecording::new(directory.clone());
    recording.start_audio(1).unwrap();
    let tap_guard = recording.tap.read().unwrap();
    let (_events, controls) = event_queue();
    let source = EngineSource::with_block_frames(controls, 44_100, 32);
    let mut source = RecordingEngineSource::new(source, recording.tap_state());
    let attempts = 8;
    let barrier = Arc::new(Barrier::new(attempts + 1));
    let (result_tx, result_rx) = mpsc::channel();
    let handles = (0..attempts)
        .map(|_| {
            let recording = recording.clone();
            let barrier = barrier.clone();
            let result_tx = result_tx.clone();
            std::thread::spawn(move || {
                barrier.wait();
                result_tx.send(recording.start_audio(1)).unwrap();
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();

    for _ in 0..512 {
        assert!(source.next().is_some());
        assert!(source.next().is_some());
    }

    let mut results = Vec::new();
    let mut timed_out = false;
    for _ in 0..attempts {
        match result_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => results.push(result),
            Err(_) => {
                timed_out = true;
                break;
            }
        }
    }
    drop(tap_guard);
    for handle in handles {
        handle.join().unwrap();
    }
    assert!(!timed_out);
    assert_eq!(results.len(), attempts);
    assert!(results
        .into_iter()
        .all(|result| matches!(result, Err(error) if error.contains("already active"))));

    let outcome = recording.stop_audio().unwrap().unwrap();
    assert_eq!(outcome.frames_written, 512);
    assert_eq!(outcome.status, RecordingStatus::Complete);
    remove_temp_dir(directory);
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-desktop-recording-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn remove_temp_dir(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}

fn solid_frame(color: u16) -> Vec<u8> {
    let bytes = color.to_be_bytes();
    let mut pixels = vec![0; OLED_FRAME_BYTES];
    for pixel in pixels.as_chunks_mut::<2>().0 {
        pixel.copy_from_slice(&bytes);
    }
    pixels
}

fn avi_video_payloads(path: &std::path::Path) -> Vec<Vec<u8>> {
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
