use super::{
    IngressState, RecordingChunk, RecordingError, RecordingOutcome, RecordingStatus,
    BITS_PER_SAMPLE, CHANNELS, SAMPLE_RATE,
};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

const RECEIVE_TIMEOUT: Duration = Duration::from_millis(100);
const WRITE_BUFFER_BYTES: usize = 4_096;
pub(crate) const PARTIAL_WAV_SUFFIX: &str = ".partial.wav";
const HEADER_BYTES: u64 = 44;
const FRAME_BYTES: u64 = CHANNELS as u64 * BITS_PER_SAMPLE as u64 / 8;

pub(crate) fn write_recording(
    file: File,
    partial_path: PathBuf,
    final_path: PathBuf,
    rx: Receiver<RecordingChunk>,
    stop: Arc<AtomicBool>,
    state: Arc<IngressState>,
    max_frames: u64,
) -> Result<RecordingOutcome, RecordingError> {
    let mut writer = WavWriter::new(file, partial_path.clone(), final_path)?;
    let mut next_frame = 0_u64;
    let mut gap_count = 0_u64;
    let mut gap_frames = 0_u64;
    loop {
        let chunk = if stop.load(Ordering::Acquire) {
            match rx.try_recv() {
                Ok(chunk) => chunk,
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        } else {
            match rx.recv_timeout(RECEIVE_TIMEOUT) {
                Ok(chunk) => chunk,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };
        if chunk.frame_offset() > next_frame {
            let gap = chunk.frame_offset() - next_frame;
            gap_count = gap_count.saturating_add(1);
            gap_frames = gap_frames.saturating_add(gap);
            let inserted = gap.min(max_frames.saturating_sub(next_frame));
            writer.write_silence(inserted)?;
            next_frame = next_frame.saturating_add(inserted);
        }
        if next_frame < max_frames {
            let skip = next_frame.saturating_sub(chunk.frame_offset()) as usize;
            let available = chunk.frame_count().saturating_sub(skip);
            let frames = available.min((max_frames - next_frame) as usize);
            if frames > 0 {
                let samples = &chunk.samples()[skip * 2..(skip + frames) * 2];
                writer.write_samples(samples)?;
                next_frame = next_frame.saturating_add(frames as u64);
            }
        }
        if next_frame >= max_frames {
            state.accepting.store(false, Ordering::Release);
            break;
        }
    }
    let cursor = state.next_frame.load(Ordering::Acquire).min(max_frames);
    if cursor > next_frame {
        let gap = cursor - next_frame;
        gap_count = gap_count.saturating_add(1);
        gap_frames = gap_frames.saturating_add(gap);
        writer.write_silence(gap)?;
        next_frame = cursor;
    }
    state.accepting.store(false, Ordering::Release);
    writer.finish(
        next_frame,
        gap_count,
        gap_frames,
        state.overflow_chunks.load(Ordering::Acquire),
        state.overflow_frames.load(Ordering::Acquire),
    )
}

struct WavWriter {
    file: File,
    partial_path: PathBuf,
    final_path: PathBuf,
}

impl WavWriter {
    fn new(
        mut file: File,
        partial_path: PathBuf,
        final_path: PathBuf,
    ) -> Result<Self, RecordingError> {
        if let Err(error) = write_header(&mut file, 0) {
            // Nothing was recorded yet (a full card, typically), so no take is left behind.
            drop(file);
            let _ = fs::remove_file(&partial_path);
            return Err(recording_error(&partial_path, error));
        }
        Ok(Self {
            file,
            partial_path,
            final_path,
        })
    }

    fn write_samples(&mut self, samples: &[i16]) -> Result<(), RecordingError> {
        let mut bytes = [0_u8; WRITE_BUFFER_BYTES];
        for portion in samples.chunks(WRITE_BUFFER_BYTES / 2) {
            for (index, sample) in portion.iter().enumerate() {
                let offset = index * 2;
                bytes[offset..offset + 2].copy_from_slice(&sample.to_le_bytes());
            }
            self.file
                .write_all(&bytes[..portion.len() * 2])
                .map_err(|error| recording_error(&self.partial_path, error))?;
        }
        Ok(())
    }

    fn write_silence(&mut self, mut frames: u64) -> Result<(), RecordingError> {
        let zeros = [0_u8; WRITE_BUFFER_BYTES];
        while frames > 0 {
            let bytes = (frames * 4).min(WRITE_BUFFER_BYTES as u64) as usize;
            self.file
                .write_all(&zeros[..bytes])
                .map_err(|error| recording_error(&self.partial_path, error))?;
            frames -= (bytes / 4) as u64;
        }
        Ok(())
    }

    fn finish(
        mut self,
        frames: u64,
        gap_count: u64,
        gap_frames: u64,
        overflow_count: u64,
        overflow_frames: u64,
    ) -> Result<RecordingOutcome, RecordingError> {
        write_header(&mut self.file, frames)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        self.file
            .flush()
            .map_err(|error| recording_error(&self.partial_path, error))?;
        self.file
            .sync_all()
            .map_err(|error| recording_error(&self.partial_path, error))?;
        let incomplete = gap_count != 0 || overflow_count != 0;
        let final_path = if incomplete {
            self.final_path.with_extension("incomplete.wav")
        } else {
            self.final_path.clone()
        };
        if final_path.exists() {
            return Err(recording_error(
                &self.partial_path,
                format!(
                    "final recording path already exists: {}",
                    final_path.display()
                ),
            ));
        }
        fs::rename(&self.partial_path, &final_path)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        let status = if !incomplete {
            RecordingStatus::Complete
        } else {
            RecordingStatus::Incomplete {
                gap_count,
                gap_frames,
                overflow_count,
                overflow_frames,
            }
        };
        Ok(RecordingOutcome {
            path: final_path,
            frames_written: frames,
            status,
            size_limited: false,
        })
    }
}

fn recording_error(partial_path: &Path, error: impl std::fmt::Display) -> RecordingError {
    RecordingError {
        message: format!(
            "audio recording failed for {}: {error}",
            partial_path.display()
        ),
        partial_path: partial_path.to_path_buf(),
    }
}

/// Saves an audio take a power cut left with an empty header as `.incomplete.wav`: the sizes
/// come from the file length, and a torn trailing frame is dropped. A file this recorder did
/// not write is left untouched.
pub(crate) fn repair_partial_wav(partial_path: &Path) -> Result<RecordingOutcome, String> {
    let failed = |error: std::io::Error| format!("{}: {error}", partial_path.display());
    let name = partial_path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(PARTIAL_WAV_SUFFIX))
        .ok_or_else(|| format!("{} is not a partial WAV take", partial_path.display()))?;
    let final_path = partial_path.with_file_name(format!("{name}.incomplete.wav"));
    if final_path.exists() {
        return Err(format!("{} already exists", final_path.display()));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(partial_path)
        .map_err(failed)?;
    let mut header = [0_u8; HEADER_BYTES as usize];
    let mut expected = Cursor::new(Vec::new());
    write_header(&mut expected, 0).map_err(failed)?;
    let ours = file.read_exact(&mut header).is_ok()
        && header[..4] == expected.get_ref()[..4]
        && header[8..40] == expected.get_ref()[8..40];
    if !ours {
        return Err(format!(
            "{} is not a WAV take this recorder wrote",
            partial_path.display()
        ));
    }
    let length = file.metadata().map_err(failed)?.len();
    let frames = (length - HEADER_BYTES) / FRAME_BYTES;
    file.set_len(HEADER_BYTES + frames * FRAME_BYTES)
        .map_err(failed)?;
    write_header(&mut file, frames).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    fs::rename(partial_path, &final_path).map_err(failed)?;
    Ok(RecordingOutcome {
        path: final_path,
        frames_written: frames,
        status: RecordingStatus::Incomplete {
            gap_count: 0,
            gap_frames: 0,
            overflow_count: 0,
            overflow_frames: 0,
        },
        size_limited: false,
    })
}

fn write_header(file: &mut (impl Write + Seek), frames: u64) -> std::io::Result<()> {
    let data_bytes = frames
        .checked_mul(u64::from(CHANNELS) * u64::from(BITS_PER_SAMPLE / 8))
        .ok_or_else(|| std::io::Error::other("WAV data size overflow"))?;
    let riff_size = 36_u64
        .checked_add(data_bytes)
        .filter(|size| *size <= u64::from(u32::MAX))
        .ok_or_else(|| std::io::Error::other("WAV exceeds RIFF size limit"))?;
    let data_bytes = u32::try_from(data_bytes).expect("checked RIFF data size");
    let riff_size = u32::try_from(riff_size).expect("checked RIFF size");
    file.seek(SeekFrom::Start(0))?;
    file.write_all(b"RIFF")?;
    file.write_all(&riff_size.to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&CHANNELS.to_le_bytes())?;
    file.write_all(&SAMPLE_RATE.to_le_bytes())?;
    let byte_rate = SAMPLE_RATE * u32::from(CHANNELS) * u32::from(BITS_PER_SAMPLE) / 8;
    file.write_all(&byte_rate.to_le_bytes())?;
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&BITS_PER_SAMPLE.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_bytes.to_le_bytes())
}
