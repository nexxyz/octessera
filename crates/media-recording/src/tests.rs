use super::*;
use std::fs::{self, File};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{atomic::Ordering, Arc};
use std::time::{Duration, Instant};

#[test]
fn service_writes_finalized_stereo_wav_and_reports_frames() {
    let dir = temp_dir("final");
    let mut service = RecorderService::new(dir.clone());
    let tap = service.start_with_max_frames(4).unwrap();
    for frame in [(1, 2), (3, 4), (5, 6)] {
        tap.push_frame(frame.0, frame.1);
    }
    assert_eq!(file_count_with_suffix(&dir, ".partial.wav"), 1);

    let outcome = service.stop_audio().unwrap().unwrap();
    assert_eq!(outcome.frames_written, 3);
    assert_eq!(outcome.status, RecordingStatus::Complete);
    assert!(outcome.path.extension().is_some_and(|ext| ext == "wav"));
    let bytes = fs::read(&outcome.path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 48);
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        SAMPLE_RATE
    );
    assert_eq!(
        u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
        176_400
    );
    assert_eq!(u16::from_le_bytes(bytes[32..34].try_into().unwrap()), 4);
    assert_eq!(
        u16::from_le_bytes(bytes[34..36].try_into().unwrap()),
        BITS_PER_SAMPLE
    );
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 12);
    assert_eq!(&bytes[44..], &[1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0]);
    assert_eq!(file_count_with_suffix(&dir, ".wav"), 1);
    assert_eq!(file_count_with_suffix(&dir, ".partial.wav"), 0);
    remove_temp_dir(dir);
}

#[test]
fn already_active_start_rejects_without_stopping_current_take() {
    let dir = temp_dir("already-active");
    let mut service = RecorderService::new(dir.clone());
    let tap = service.start_with_max_frames(4).unwrap();
    let error = service.start_with_max_frames(4).unwrap_err();
    assert_eq!(error, RecordingStartError::AlreadyActive);
    let mut chunk = tap.new_chunk();
    assert!(chunk.push_frame(9, 10));
    tap.push_chunk(chunk);
    let outcome = service.stop_audio().unwrap().unwrap();
    assert_eq!(outcome.frames_written, 1);
    remove_temp_dir(dir);
}

#[test]
fn stop_is_idempotent_and_drains_queued_audio() {
    let dir = temp_dir("idempotent-stop");
    let mut service = RecorderService::new(dir.clone());
    let tap = service.start_with_max_frames(4).unwrap();
    let mut chunk = tap.new_chunk();
    assert!(chunk.push_frame(11, 12));
    tap.push_chunk(chunk);
    assert_eq!(service.stop_audio().unwrap().unwrap().frames_written, 1);
    assert!(service.stop_audio().unwrap().is_none());
    remove_temp_dir(dir);
}

#[test]
fn tap_chunks_use_absolute_session_frame_offsets() {
    let (tx, _rx) = mpsc::sync_channel(2);
    let tap = RecordingTap {
        tx,
        state: Arc::new(IngressState::new()),
    };
    let mut chunk = tap.new_chunk();
    for _ in 0..CHUNK_FRAMES {
        assert!(chunk.push_frame(0, 0));
    }
    tap.push_chunk(chunk);
    assert_eq!(tap.new_chunk().frame_offset(), CHUNK_FRAMES as u64);
}

#[test]
fn automatic_duration_completion_clears_active_state() {
    let dir = temp_dir("auto-stop");
    let mut service = RecorderService::new(dir.clone());
    let tap = service.start_with_max_frames(2).unwrap();
    let mut chunk = tap.new_chunk();
    for frame in [(13, 14), (15, 16), (17, 18)] {
        assert!(chunk.push_frame(frame.0, frame.1));
    }
    tap.push_chunk(chunk);
    let deadline = Instant::now() + Duration::from_secs(1);
    while service.is_recording() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(!service.is_recording());
    let outcome = loop {
        if let Some(outcome) = service.poll_completed().unwrap() {
            break outcome;
        }
        std::thread::yield_now();
    };
    assert_eq!(outcome.frames_written, 2);
    assert!(service.stop_audio().unwrap().is_none());
    remove_temp_dir(dir);
}

#[test]
fn gaps_insert_silence_and_finalize_as_incomplete() {
    let dir = temp_dir("gap");
    let (partial, final_path, file) = test_paths(&dir, "gap");
    let (tx, rx) = mpsc::sync_channel(4);
    let state = Arc::new(IngressState::new());
    let tap = RecordingTap {
        tx,
        state: state.clone(),
    };
    let mut first = RecordingChunk::new(0);
    assert!(first.push_frame(1, 2));
    tap.push_chunk(first);
    let mut second = RecordingChunk::new(3);
    assert!(second.push_frame(3, 4));
    tap.push_chunk(second);
    tap.deactivate_and_flush();
    drop(tap);
    let result = wav::write_recording(
        file,
        partial,
        final_path.clone(),
        rx,
        Arc::new(AtomicBool::new(true)),
        state,
        8,
    )
    .unwrap();
    assert_eq!(result.frames_written, 4);
    assert_eq!(
        result.status,
        RecordingStatus::Incomplete {
            gap_count: 1,
            gap_frames: 2,
            overflow_count: 0,
            overflow_frames: 0,
        }
    );
    let bytes = fs::read(&result.path).unwrap();
    assert_eq!(
        &bytes[44..],
        &[1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0]
    );
    assert!(result.path.ends_with("gap.incomplete.wav"));
    assert!(!final_path.exists());
    remove_temp_dir(dir);
}

#[test]
fn overflow_is_counted_and_uses_incomplete_name() {
    let dir = temp_dir("overflow");
    let (partial, final_path, file) = test_paths(&dir, "overflow");
    let (tx, rx) = mpsc::sync_channel(1);
    let state = Arc::new(IngressState::new());
    let tap = RecordingTap {
        tx,
        state: state.clone(),
    };
    for value in [1, 3] {
        let mut chunk = tap.new_chunk();
        assert!(chunk.push_frame(value, value + 1));
        tap.push_chunk(chunk);
    }
    tap.deactivate_and_flush();
    drop(tap);
    let result = wav::write_recording(
        file,
        partial,
        final_path.clone(),
        rx,
        Arc::new(AtomicBool::new(true)),
        state,
        8,
    )
    .unwrap();
    assert!(matches!(
        result.status,
        RecordingStatus::Incomplete {
            overflow_count: 1,
            ..
        }
    ));
    assert_eq!(result.frames_written, 2);
    assert!(result.path.ends_with("overflow.incomplete.wav"));
    remove_temp_dir(dir);
}

#[test]
fn finalization_failure_retains_partial_file() {
    let dir = temp_dir("finalization-failure");
    let (partial, final_path, file) = test_paths(&dir, "failure");
    fs::create_dir(&final_path).unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let state = Arc::new(IngressState::new());
    state.accepting.store(false, Ordering::Release);
    let error = wav::write_recording(
        file,
        partial.clone(),
        final_path,
        rx,
        Arc::new(AtomicBool::new(true)),
        state,
        8,
    )
    .unwrap_err();
    assert!(error.to_string().contains("already exists"));
    assert!(error.partial_path().ends_with("failure.partial.wav"));
    assert!(partial.exists());
    remove_temp_dir(dir);
}

#[test]
fn write_failure_retains_partial_file() {
    let dir = temp_dir("write-failure");
    let partial = dir.join("write.partial.wav");
    let final_path = dir.join("write.wav");
    fs::write(&partial, []).unwrap();
    let file = File::open(&partial).unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let state = Arc::new(IngressState::new());
    state.accepting.store(false, Ordering::Release);
    let error = wav::write_recording(
        file,
        partial.clone(),
        final_path,
        rx,
        Arc::new(AtomicBool::new(true)),
        state,
        8,
    )
    .unwrap_err();
    assert!(error.to_string().contains("audio recording failed"));
    assert!(partial.exists());
    remove_temp_dir(dir);
}

#[test]
fn path_reservation_skips_existing_partial_and_final_takes() {
    let dir = temp_dir("collision");
    fs::write(dir.join("octessera-fixed.wav"), []).unwrap();
    fs::write(dir.join("octessera-fixed.partial.wav"), []).unwrap();
    let (partial, final_path, file) =
        super::service::reserve_paths_for_test(&dir, "octessera-fixed").unwrap();
    assert!(partial.ends_with("octessera-fixed-1.partial.wav"));
    assert!(final_path.ends_with("octessera-fixed-1.wav"));
    drop(file);
    remove_temp_dir(dir);
}

fn test_paths(dir: &Path, name: &str) -> (PathBuf, PathBuf, File) {
    super::service::reserve_paths_for_test(dir, name).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-media-recording-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn file_count_with_suffix(dir: &Path, suffix: &str) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().to_string_lossy().ends_with(suffix))
        .count()
}

fn remove_temp_dir(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}
