use super::*;
use std::fs::{self, File};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-media-header-failure-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// A handle that refuses writes stands in for a card with no room left for the header.
fn unwritable(partial: &std::path::Path, reserved: File) -> File {
    drop(reserved);
    File::open(partial).unwrap()
}

#[test]
fn a_wav_take_whose_header_cannot_be_written_leaves_no_file() {
    let dir = temp_dir("wav");
    let (partial, final_path, reserved) =
        crate::service::reserve_paths_for_test(&dir, "full").unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let error = crate::wav::write_recording(
        unwritable(&partial, reserved),
        partial.clone(),
        final_path,
        rx,
        Arc::new(AtomicBool::new(true)),
        Arc::new(IngressState::new()),
        8,
    )
    .unwrap_err();
    assert!(error.to_string().contains("audio recording failed"));
    assert!(!partial.exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_avi_take_whose_header_cannot_be_written_leaves_no_file() {
    let dir = temp_dir("avi");
    let (partial, final_path, reserved) =
        crate::service::reserve_avi_paths_for_test(&dir, "full").unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let result = crate::avi::write_recording(crate::avi::AviRecordingInput {
        file: unwritable(&partial, reserved),
        partial_path: partial.clone(),
        final_path,
        rx,
        stop: Arc::new(AtomicBool::new(true)),
        state: Arc::new(IngressState::new()),
        oled: OledIngress {
            state: Arc::new(OledIngressState::new()),
        },
        max_frames: 8,
        file_limit: AVI_TARGET_MAX_BYTES,
    });
    assert!(result.is_err());
    assert!(!partial.exists());
    let _ = fs::remove_dir_all(dir);
}

/// A card that takes `room` bytes of the take and then reports itself full.
struct CardWithRoom {
    file: File,
    room: u64,
}

impl std::io::Write for CardWithRoom {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use std::io::Seek;
        let left = self.room.saturating_sub(self.file.stream_position()?);
        if left == 0 {
            return Err(std::io::Error::other("no space left on device"));
        }
        self.file.write(&buf[..buf.len().min(left as usize)])
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl std::io::Seek for CardWithRoom {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

fn record_onto_card(name: &str, room: u64) -> (std::path::PathBuf, Vec<i16>) {
    let dir = temp_dir(name);
    let (partial, final_path, file) = crate::service::reserve_paths_for_test(&dir, "take").unwrap();
    let writer =
        crate::wav::WavWriter::new(CardWithRoom { file, room }, partial.clone(), final_path)
            .unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let mut chunk = RecordingChunk::new(0);
    for frame in 0..CHUNK_FRAMES as i16 {
        chunk.push_frame(frame, -frame);
    }
    let samples = chunk.samples().to_vec();
    tx.send(chunk).unwrap();
    drop(tx);
    let result = crate::wav::write_take(
        writer,
        &rx,
        &AtomicBool::new(false),
        &IngressState::new(),
        CHUNK_FRAMES as u64,
    );
    assert!(result.is_err());
    (partial, samples)
}

#[test]
fn a_wav_take_whose_first_audio_write_fails_leaves_no_file() {
    let (partial, _) = record_onto_card("header-only", 44);
    assert!(!partial.exists());
    let _ = fs::remove_dir_all(partial.parent().unwrap());
}

#[test]
fn a_wav_take_that_fails_after_some_audio_keeps_a_playable_partial() {
    let (partial, samples) = record_onto_card("some-audio", 44 + 100 * 4 + 2);
    let repaired = crate::wav::repair_partial_wav(&partial).unwrap();
    assert_eq!(repaired.frames_written, 100);
    let bytes = fs::read(&repaired.path).unwrap();
    assert_eq!(bytes.len(), 44 + 100 * 4);
    let audio: Vec<i16> = bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect();
    assert_eq!(audio, samples[..200]);
    let _ = fs::remove_dir_all(partial.parent().unwrap());
}
