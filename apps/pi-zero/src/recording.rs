use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const QUEUE_CAPACITY: usize = 32;
const CHUNK_SAMPLES: usize = 4096;

pub(crate) struct RecordingChunk {
    len: usize,
    samples: [i16; CHUNK_SAMPLES],
}

impl RecordingChunk {
    pub(crate) fn new() -> Self {
        Self {
            len: 0,
            samples: [0; CHUNK_SAMPLES],
        }
    }

    pub(crate) fn push(&mut self, sample: i16) -> bool {
        if self.len >= CHUNK_SAMPLES {
            return false;
        }
        self.samples[self.len] = sample;
        self.len += 1;
        true
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn take(&mut self) -> Self {
        std::mem::replace(self, Self::new())
    }

    fn samples(&self) -> &[i16] {
        &self.samples[..self.len]
    }
}

#[derive(Clone)]
pub(crate) struct RecordingTap {
    tx: SyncSender<RecordingChunk>,
    overflowed: Arc<AtomicBool>,
}

impl RecordingTap {
    pub(crate) fn push_chunk(&self, chunk: RecordingChunk) {
        match self.tx.try_send(chunk) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.overflowed.store(true, Ordering::Relaxed),
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

pub(crate) struct RecorderService {
    dir: PathBuf,
    active: Option<ActiveRecording>,
}

struct ActiveRecording {
    tap: RecordingTap,
    stop: Arc<AtomicBool>,
    join: JoinHandle<RecordingResult>,
}

#[derive(Debug, PartialEq, Eq)]
struct RecordingResult {
    final_path: Option<PathBuf>,
    overflowed: bool,
    samples_written: u64,
}

impl RecorderService {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir, active: None }
    }

    pub(crate) fn start_audio(&mut self, max_minutes: u16) -> Result<RecordingTap, String> {
        self.stop_audio();
        fs::create_dir_all(&self.dir).map_err(|e| format!("recording dir unavailable: {e}"))?;
        let (tx, rx) = mpsc::sync_channel::<RecordingChunk>(QUEUE_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let overflowed = Arc::new(AtomicBool::new(false));
        let tap = RecordingTap {
            tx,
            overflowed: overflowed.clone(),
        };
        let dir = self.dir.clone();
        let max_samples = u64::from(max_minutes.clamp(1, 120)) * 60 * u64::from(SAMPLE_RATE);
        let worker_stop = stop.clone();
        let join =
            thread::spawn(move || write_recording(dir, rx, worker_stop, overflowed, max_samples));
        self.active = Some(ActiveRecording {
            tap: tap.clone(),
            stop,
            join,
        });
        Ok(tap)
    }

    pub(crate) fn stop_audio(&mut self) {
        let Some(active) = self.active.take() else {
            return;
        };
        active.stop.store(true, Ordering::Relaxed);
        drop(active.tap);
        match active.join.join() {
            Ok(result) => println!(
                "recording stopped: path={:?} samples={} overflowed={}",
                result.final_path, result.samples_written, result.overflowed
            ),
            Err(_) => eprintln!("recording writer thread panicked"),
        }
    }
}

fn write_recording(
    dir: PathBuf,
    rx: mpsc::Receiver<RecordingChunk>,
    stop: Arc<AtomicBool>,
    overflowed: Arc<AtomicBool>,
    max_samples: u64,
) -> RecordingResult {
    let stem = recording_stem();
    let partial = dir.join(format!("{stem}.partial.wav"));
    let final_path = dir.join(format!("{stem}.wav"));
    let result = write_wav_stream(&partial, &final_path, rx, stop, &overflowed, max_samples);
    match result {
        Ok(samples_written) => RecordingResult {
            final_path: Some(final_path),
            overflowed: overflowed.load(Ordering::Relaxed),
            samples_written,
        },
        Err(error) => {
            eprintln!("recording failed: {error}");
            let _ = fs::remove_file(partial);
            RecordingResult {
                final_path: None,
                overflowed: overflowed.load(Ordering::Relaxed),
                samples_written: 0,
            }
        }
    }
}

fn write_wav_stream(
    partial: &Path,
    final_path: &Path,
    rx: mpsc::Receiver<RecordingChunk>,
    stop: Arc<AtomicBool>,
    overflowed: &AtomicBool,
    max_samples: u64,
) -> Result<u64, String> {
    let mut file = File::create(partial).map_err(|e| e.to_string())?;
    write_header(&mut file, 0)?;
    let mut samples_written = 0_u64;
    while !stop.load(Ordering::Relaxed) && samples_written < max_samples {
        let Ok(chunk) = rx.recv() else {
            break;
        };
        let remaining = ((max_samples - samples_written) * u64::from(CHANNELS)) as usize;
        let samples = chunk.samples();
        let len = samples.len().min(remaining);
        write_i16_samples(&mut file, &samples[..len])?;
        samples_written += (len / usize::from(CHANNELS)) as u64;
    }
    finalize_wav(file, samples_written)?;
    fs::rename(partial, final_path).map_err(|e| e.to_string())?;
    if overflowed.load(Ordering::Relaxed) {
        eprintln!("recording queue overflowed; WAV contains dropped audio");
    }
    Ok(samples_written)
}

fn write_i16_samples(file: &mut File, samples: &[i16]) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    file.write_all(&bytes).map_err(|e| e.to_string())
}

fn finalize_wav(mut file: File, samples_written: u64) -> Result<(), String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    write_header(&mut file, samples_written)
}

fn write_header(file: &mut File, samples_written: u64) -> Result<(), String> {
    let data_bytes = samples_written * u64::from(CHANNELS) * 2;
    let riff_size = 36 + data_bytes;
    file.write_all(b"RIFF").map_err(|e| e.to_string())?;
    file.write_all(&(riff_size as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"WAVEfmt ").map_err(|e| e.to_string())?;
    file.write_all(&16_u32.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&1_u16.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&CHANNELS.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&SAMPLE_RATE.to_le_bytes())
        .map_err(|e| e.to_string())?;
    let byte_rate = SAMPLE_RATE * u32::from(CHANNELS) * u32::from(BITS_PER_SAMPLE) / 8;
    file.write_all(&byte_rate.to_le_bytes())
        .map_err(|e| e.to_string())?;
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    file.write_all(&block_align.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&BITS_PER_SAMPLE.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"data").map_err(|e| e.to_string())?;
    file.write_all(&(data_bytes as u32).to_le_bytes())
        .map_err(|e| e.to_string())
}

fn recording_stem() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("octessera-{seconds}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_wav_header_is_finalized_and_renamed() {
        let dir = tempfile_dir("wav-finalize");
        let partial = dir.join("test.partial.wav");
        let final_path = dir.join("test.wav");
        let (tx, rx) = mpsc::sync_channel(2);
        tx.send(chunk(&[0, i16::MAX, i16::MIN, -1])).unwrap();
        drop(tx);
        let stop = Arc::new(AtomicBool::new(false));
        let overflowed = AtomicBool::new(false);
        let samples = write_wav_stream(&partial, &final_path, rx, stop, &overflowed, 10).unwrap();
        let bytes = fs::read(&final_path).unwrap();
        assert_eq!(samples, 2);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[40..44], 8_u32.to_le_bytes().as_slice());
        assert!(!partial.exists());
    }

    #[test]
    fn recording_stops_at_max_duration_samples() {
        let dir = tempfile_dir("max-duration");
        let partial = dir.join("test.partial.wav");
        let final_path = dir.join("test.wav");
        let (tx, rx) = mpsc::sync_channel(2);
        tx.send(chunk(&[1; 12])).unwrap();
        drop(tx);
        let samples = write_wav_stream(
            &partial,
            &final_path,
            rx,
            Arc::new(AtomicBool::new(false)),
            &AtomicBool::new(false),
            2,
        )
        .unwrap();
        assert_eq!(samples, 2);
        assert_eq!(fs::metadata(final_path).unwrap().len(), 44 + 8);
    }

    #[test]
    fn recording_tap_marks_overflow_without_blocking() {
        let (tx, _rx) = mpsc::sync_channel(0);
        let overflowed = Arc::new(AtomicBool::new(false));
        let tap = RecordingTap {
            tx,
            overflowed: overflowed.clone(),
        };
        tap.push_chunk(chunk(&[1, 2]));
        assert!(overflowed.load(Ordering::Relaxed));
    }

    fn chunk(samples: &[i16]) -> RecordingChunk {
        let mut chunk = RecordingChunk::new();
        for sample in samples {
            assert!(chunk.push(*sample));
        }
        chunk
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("octessera-recording-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
