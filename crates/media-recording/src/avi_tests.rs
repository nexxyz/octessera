use super::*;
use image::GenericImageView;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

#[test]
fn avi_headers_chunks_index_and_jpeg_are_valid() {
    let dir = temp_dir("avi-format");
    let mut service = RecorderService::new(dir.clone());
    let recording = service.start_oled_with_max_frames(8_820).unwrap();
    assert!(
        recording
            .oled
            .try_submit(solid_frame(1, 0, 0xf800))
            .accepted
    );
    assert!(
        recording
            .oled
            .try_submit(solid_frame(2, 4_410, 0x07e0))
            .accepted
    );
    push_audio(&recording.tap, 8_820);
    let outcome = service.stop_audio().unwrap().unwrap();
    assert_eq!(outcome.frames_written, 8_820);
    assert_eq!(outcome.status, RecordingStatus::Complete);
    assert!(outcome
        .path
        .extension()
        .is_some_and(|extension| extension == "avi"));
    assert!(!outcome.path.to_string_lossy().contains(".incomplete."));

    let bytes = fs::read(&outcome.path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(read_u32(&bytes, 4) as usize, bytes.len() - 8);
    assert_eq!(&bytes[8..12], b"AVI ");
    assert!(!bytes
        .windows(4)
        .any(|chunk| chunk == b"odml" || chunk == b"dmlh"));
    let avih = find_chunks(&bytes, *b"avih").next().unwrap();
    assert_eq!(read_u32(avih.payload, 0), 100_000);
    assert_eq!(read_u32(avih.payload, 12), 0x110);
    assert_eq!(read_u32(avih.payload, 16), 2);
    assert_eq!(read_u32(avih.payload, 24), 2);
    assert_eq!(read_u32(avih.payload, 32), 128);
    assert_eq!(read_u32(avih.payload, 36), 128);

    let stream_headers: Vec<_> = find_chunks(&bytes, *b"strh").collect();
    assert_eq!(stream_headers.len(), 2);
    assert_eq!(&stream_headers[0].payload[..4], b"vids");
    assert_eq!(&stream_headers[0].payload[4..8], b"MJPG");
    assert_eq!(read_u32(stream_headers[0].payload, 20), 1);
    assert_eq!(read_u32(stream_headers[0].payload, 24), 10);
    assert_eq!(read_u32(stream_headers[0].payload, 32), 2);
    assert_eq!(&stream_headers[1].payload[..4], b"auds");
    assert_eq!(read_u32(stream_headers[1].payload, 20), 1);
    assert_eq!(read_u32(stream_headers[1].payload, 24), SAMPLE_RATE);
    assert_eq!(read_u32(stream_headers[1].payload, 32), 8_820);

    let formats: Vec<_> = find_chunks(&bytes, *b"strf").collect();
    assert_eq!(formats.len(), 2);
    assert_eq!(read_u32(formats[0].payload, 4), 128);
    assert_eq!(read_u32(formats[0].payload, 8), 128);
    assert_eq!(&formats[0].payload[16..20], b"MJPG");
    assert_eq!(read_u16(formats[1].payload, 0), 1);
    assert_eq!(read_u16(formats[1].payload, 2), 2);
    assert_eq!(read_u32(formats[1].payload, 4), SAMPLE_RATE);
    assert_eq!(read_u16(formats[1].payload, 14), 16);

    let movi = find_list(&bytes, *b"movi");
    let movi_chunks: Vec<_> = chunks_in(&bytes, movi.data_start, movi.end).collect();
    assert_eq!(movi_chunks.len(), 4);
    assert_eq!(&movi_chunks[0].id, b"00dc");
    assert_eq!(&movi_chunks[1].id, b"01wb");
    assert_eq!(&movi_chunks[2].id, b"00dc");
    assert_eq!(&movi_chunks[3].id, b"01wb");
    assert_eq!(movi_chunks[1].payload.len(), 17_640);
    let decoded = image::load_from_memory(movi_chunks[0].payload).unwrap();
    assert_eq!(decoded.dimensions(), (128, 128));
    assert!(decoded.get_pixel(64, 64).0[0] > 150);

    let index = find_chunks(&bytes, *b"idx1").next().unwrap();
    assert_eq!(index.payload.len(), movi_chunks.len() * 16);
    for (entry, chunk) in index.payload.chunks_exact(16).zip(&movi_chunks) {
        assert_eq!(&entry[..4], &chunk.id);
        assert_eq!(
            read_u32(entry, 8),
            (chunk.offset - movi.list_start - 8) as u32
        );
        assert_eq!(read_u32(entry, 12), chunk.payload.len() as u32);
    }
    assert_eq!(movi.end, index.offset);
    assert!(bytes[index.offset + 8 + index.payload.len()..]
        .iter()
        .all(|byte| *byte == 0));
    remove_temp_dir(dir);
}

#[test]
fn audio_is_master_clock_and_frames_hold_until_changed() {
    let dir = temp_dir("avi-timing");
    let mut service = RecorderService::new(dir.clone());
    let recording = service.start_oled_with_max_frames(13_230).unwrap();
    assert!(
        recording
            .oled
            .try_submit(solid_frame(1, 0, 0xf800))
            .accepted
    );
    assert!(
        recording
            .oled
            .try_submit(solid_frame(2, 4_410, 0x001f))
            .accepted
    );
    let stale = recording.oled.try_submit(solid_frame(1, 8_820, 0x07e0));
    assert!(stale.stale);
    push_audio(&recording.tap, 13_230);
    let outcome = service.stop_audio().unwrap().unwrap();
    let bytes = fs::read(outcome.path).unwrap();
    let movi = find_list(&bytes, *b"movi");
    let video: Vec<_> = chunks_in(&bytes, movi.data_start, movi.end)
        .filter(|chunk| &chunk.id == b"00dc")
        .collect();
    assert_eq!(video.len(), 3);
    assert_ne!(video[0].payload, video[1].payload);
    assert_eq!(video[1].payload, video[2].payload);
    remove_temp_dir(dir);
}

#[test]
fn gaps_make_silent_audio_and_incomplete_avi() {
    let dir = temp_dir("avi-gap");
    let mut service = RecorderService::new(dir.clone());
    let recording = service.start_oled_with_max_frames(8).unwrap();
    let mut first = RecordingChunk::new(0);
    assert!(first.push_frame(1, 2));
    recording.tap.push_chunk(first);
    let mut second = RecordingChunk::new(3);
    assert!(second.push_frame(3, 4));
    recording.tap.push_chunk(second);
    let outcome = service.stop_audio().unwrap().unwrap();
    assert!(outcome.path.to_string_lossy().contains(".incomplete.avi"));
    assert_eq!(
        outcome.status,
        RecordingStatus::Incomplete {
            gap_count: 1,
            gap_frames: 2,
            overflow_count: 0,
            overflow_frames: 0,
        }
    );
    let bytes = fs::read(outcome.path).unwrap();
    let movi = find_list(&bytes, *b"movi");
    let audio = chunks_in(&bytes, movi.data_start, movi.end)
        .find(|chunk| &chunk.id == b"01wb")
        .unwrap();
    assert_eq!(audio.payload.len(), 16);
    assert_eq!(&audio.payload[..4], &[1, 0, 2, 0]);
    assert_eq!(&audio.payload[4..12], &[0; 8]);
    assert_eq!(&audio.payload[12..], &[3, 0, 4, 0]);
    remove_temp_dir(dir);
}

#[test]
fn oled_ingress_is_bounded_nonblocking_and_reports_loss() {
    let dir = temp_dir("avi-ingress");
    let mut service = RecorderService::new(dir.clone());
    let recording = service.start_oled_with_max_frames(1).unwrap();
    assert_eq!(file_count_with_suffix(&dir, ".partial.avi"), 1);
    let started = Instant::now();
    let mut loss = false;
    for revision in 0..(OLED_QUEUE_CAPACITY as u64 + 2) {
        let result = recording
            .oled
            .try_submit(solid_frame(revision, revision, 0xffff));
        loss |= result.material_loss;
    }
    assert!(started.elapsed().as_millis() < 100);
    assert!(loss);
    let outcome = service.stop_audio().unwrap().unwrap();
    assert!(matches!(outcome.status, RecordingStatus::Incomplete { .. }));
    remove_temp_dir(dir);
}

#[test]
fn avi_mode_rejects_other_starts_and_stop_is_idle_safe() {
    let dir = temp_dir("avi-lifecycle");
    let mut service = RecorderService::new(dir.clone());
    let recording = service.start_oled_with_max_frames(1).unwrap();
    assert_eq!(
        service.start_with_max_frames(1).unwrap_err(),
        RecordingStartError::AlreadyActive
    );
    drop(recording);
    service.stop_audio().unwrap();
    assert!(service.stop_audio().unwrap().is_none());
    remove_temp_dir(dir);
}

#[test]
fn avi_duration_limit_cleanly_stops_and_finalizes() {
    let dir = temp_dir("avi-limit");
    let mut service = RecorderService::new(dir.clone());
    let recording = service.start_oled_with_max_frames(4_410).unwrap();
    push_audio(&recording.tap, 5_000);
    while service.is_recording() {
        std::thread::yield_now();
    }
    let outcome = service.stop_audio().unwrap().unwrap();
    assert_eq!(outcome.frames_written, 4_410);
    assert_eq!(outcome.status, RecordingStatus::Complete);
    assert!(outcome
        .path
        .extension()
        .is_some_and(|extension| extension == "avi"));
    assert!(!outcome.path.to_string_lossy().contains(".incomplete."));
    remove_temp_dir(dir);
}

#[test]
fn avi_size_limit_stops_at_a_complete_interleave_unit() {
    let dir = temp_dir("avi-size-limit");
    let (partial, final_path, file) =
        crate::service::reserve_avi_paths_for_test(&dir, "size").unwrap();
    let (tx, rx) = mpsc::sync_channel(16);
    for offset in (0..8_820).step_by(CHUNK_FRAMES) {
        let count = (8_820 - offset).min(CHUNK_FRAMES);
        tx.send(chunk_with_frames(offset as u64, count)).unwrap();
    }
    drop(tx);
    let state = Arc::new(IngressState::new());
    state
        .next_frame
        .store(8_820, std::sync::atomic::Ordering::Release);
    state
        .accepting
        .store(false, std::sync::atomic::Ordering::Release);
    let oled = OledIngress {
        state: Arc::new(OledIngressState::new()),
    };
    let outcome = crate::avi::write_recording(crate::avi::AviRecordingInput {
        file,
        partial_path: partial,
        final_path,
        rx,
        stop: Arc::new(AtomicBool::new(true)),
        state,
        oled,
        max_frames: 8_820,
        file_limit: 20_000,
    })
    .unwrap();
    assert_eq!(outcome.frames_written, 4_410);
    assert_eq!(outcome.status, RecordingStatus::Complete);
    assert!(fs::metadata(outcome.path).unwrap().len() <= 20_000);
    remove_temp_dir(dir);
}

#[test]
fn avi_finalization_failure_retains_partial_file() {
    let dir = temp_dir("avi-finalization-failure");
    let (partial, final_path, file) =
        crate::service::reserve_avi_paths_for_test(&dir, "failure").unwrap();
    fs::create_dir(&final_path).unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let state = Arc::new(IngressState::new());
    state
        .accepting
        .store(false, std::sync::atomic::Ordering::Release);
    let oled = OledIngress {
        state: Arc::new(OledIngressState::new()),
    };
    let error = crate::avi::write_recording(crate::avi::AviRecordingInput {
        file,
        partial_path: partial.clone(),
        final_path,
        rx,
        stop: Arc::new(AtomicBool::new(true)),
        state,
        oled,
        max_frames: 8,
        file_limit: AVI_TARGET_MAX_BYTES,
    })
    .unwrap_err();
    assert!(error.to_string().contains("already exists"));
    assert!(partial.exists());
    remove_temp_dir(dir);
}

#[test]
fn avi_collision_reservation_keeps_existing_files() {
    let dir = temp_dir("avi-collision");
    fs::write(dir.join("take.avi"), []).unwrap();
    fs::write(dir.join("take.incomplete.avi"), []).unwrap();
    fs::write(dir.join("take-1.avi"), []).unwrap();
    let (partial, final_path, file) =
        crate::service::reserve_avi_paths_for_test(&dir, "take").unwrap();
    assert!(partial.ends_with("take-2.partial.avi"));
    assert!(final_path.ends_with("take-2.avi"));
    drop(file);
    remove_temp_dir(dir);
}

#[test]
fn avi_worker_failure_retains_partial_file() {
    let dir = temp_dir("avi-failure");
    let partial = dir.join("failure.partial.avi");
    fs::write(&partial, []).unwrap();
    let file = File::open(&partial).unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let state = Arc::new(IngressState::new());
    state
        .accepting
        .store(false, std::sync::atomic::Ordering::Release);
    let oled = OledIngress {
        state: Arc::new(OledIngressState::new()),
    };
    let error = crate::avi::write_recording(crate::avi::AviRecordingInput {
        file,
        partial_path: partial.clone(),
        final_path: dir.join("failure.avi"),
        rx,
        stop: Arc::new(AtomicBool::new(true)),
        state,
        oled,
        max_frames: 8,
        file_limit: AVI_TARGET_MAX_BYTES,
    })
    .unwrap_err();
    assert!(error.partial_path().ends_with("failure.partial.avi"));
    assert!(partial.exists());
    remove_temp_dir(dir);
}

fn push_audio(tap: &RecordingTap, frames: usize) {
    let mut chunk = tap.new_chunk();
    for index in 0..frames {
        if !chunk.push_frame(index as i16, -(index as i16)) {
            tap.push_chunk(chunk);
            chunk = tap.new_chunk();
            assert!(chunk.push_frame(index as i16, -(index as i16)));
        }
    }
    if !chunk.is_empty() {
        tap.push_chunk(chunk);
    }
}

fn chunk_with_frames(offset: u64, frames: usize) -> RecordingChunk {
    let mut chunk = RecordingChunk::new(offset);
    for index in 0..frames {
        assert!(chunk.push_frame(index as i16, -(index as i16)));
    }
    chunk
}

fn solid_frame(revision: u64, audio_frame: u64, color: u16) -> OledFrame {
    let bytes = color.to_be_bytes();
    let mut pixels = [0_u8; OLED_FRAME_BYTES];
    for pair in pixels.chunks_exact_mut(2) {
        pair.copy_from_slice(&bytes);
    }
    OledFrame::new(revision, audio_frame, pixels)
}

#[derive(Clone, Copy)]
struct Chunk<'a> {
    id: [u8; 4],
    payload: &'a [u8],
    offset: usize,
}

struct List {
    list_start: usize,
    data_start: usize,
    end: usize,
}

fn find_list(bytes: &[u8], kind: [u8; 4]) -> List {
    let start = bytes
        .windows(12)
        .position(|window| &window[..4] == b"LIST" && window[8..12] == kind)
        .unwrap();
    let size = read_u32(bytes, start + 4) as usize;
    List {
        list_start: start,
        data_start: start + 12,
        end: start + 8 + size,
    }
}

fn find_chunks<'a>(bytes: &'a [u8], id: [u8; 4]) -> impl Iterator<Item = Chunk<'a>> {
    bytes
        .windows(8)
        .enumerate()
        .filter_map(move |(offset, header)| {
            if header[..4] != id {
                return None;
            }
            let size = read_u32(bytes, offset + 4) as usize;
            let end = offset + 8 + size;
            (end <= bytes.len()).then(|| Chunk {
                id,
                payload: &bytes[offset + 8..end],
                offset,
            })
        })
}

fn chunks_in<'a>(
    bytes: &'a [u8],
    mut offset: usize,
    end: usize,
) -> impl Iterator<Item = Chunk<'a>> {
    let mut chunks = Vec::new();
    while offset + 8 <= end {
        let id = bytes[offset..offset + 4].try_into().unwrap();
        let size = read_u32(bytes, offset + 4) as usize;
        let payload_end = offset + 8 + size;
        if payload_end > end {
            break;
        }
        chunks.push(Chunk {
            id,
            payload: &bytes[offset + 8..payload_end],
            offset,
        });
        offset = payload_end + size % 2;
    }
    chunks.into_iter()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
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

fn remove_temp_dir(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}

fn file_count_with_suffix(dir: &std::path::Path, suffix: &str) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().to_string_lossy().ends_with(suffix))
        .count()
}
