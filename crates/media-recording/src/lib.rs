use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::{collections::VecDeque, fmt};

mod avi;
mod avi_writer;
mod service;
mod wav;

pub use service::RecorderService;

pub const SAMPLE_RATE: u32 = 44_100;
pub const CHANNELS: u16 = 2;
pub const BITS_PER_SAMPLE: u16 = 16;
pub const CHUNK_FRAMES: usize = 1_024;
pub const QUEUE_CAPACITY: usize = 32;
pub const OLED_WIDTH: u32 = 128;
pub const OLED_HEIGHT: u32 = 128;
pub const OLED_FRAME_BYTES: usize = OLED_WIDTH as usize * OLED_HEIGHT as usize * 2;
pub const OLED_QUEUE_CAPACITY: usize = 8;
pub const AVI_TARGET_MAX_BYTES: u64 = 3_500_000_000;

const CHUNK_SAMPLES: usize = CHUNK_FRAMES * 2;

#[derive(Debug)]
pub struct RecordingChunk {
    frame_offset: u64,
    frame_count: usize,
    samples: [i16; CHUNK_SAMPLES],
}

impl RecordingChunk {
    pub fn new(frame_offset: u64) -> Self {
        Self {
            frame_offset,
            frame_count: 0,
            samples: [0; CHUNK_SAMPLES],
        }
    }

    pub fn push_frame(&mut self, left: i16, right: i16) -> bool {
        if self.frame_count >= CHUNK_FRAMES {
            return false;
        }
        let index = self.frame_count * usize::from(CHANNELS);
        self.samples[index] = left;
        self.samples[index + 1] = right;
        self.frame_count += 1;
        true
    }

    pub fn frame_offset(&self) -> u64 {
        self.frame_offset
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn is_empty(&self) -> bool {
        self.frame_count == 0
    }

    fn samples(&self) -> &[i16] {
        &self.samples[..self.frame_count * usize::from(CHANNELS)]
    }
}

#[derive(Clone, Debug)]
pub struct RecordingTap {
    tx: SyncSender<RecordingChunk>,
    state: Arc<IngressState>,
}

impl RecordingTap {
    pub fn new_chunk(&self) -> RecordingChunk {
        RecordingChunk::new(self.state.next_frame.load(Ordering::Relaxed))
    }

    pub fn push_chunk(&self, chunk: RecordingChunk) {
        if !self.state.accepting.load(Ordering::Acquire) || chunk.is_empty() {
            return;
        }
        self.push_chunk_unchecked(chunk);
    }

    pub fn push_frame(&self, left: i16, right: i16) {
        if !self.state.accepting.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut pending) = self.state.pending_chunk.try_lock() else {
            self.state.next_frame.fetch_add(1, Ordering::Relaxed);
            self.state.overflow_chunks.fetch_add(1, Ordering::Relaxed);
            self.state.overflow_frames.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if !self.state.accepting.load(Ordering::Acquire) {
            return;
        }
        if pending.is_none() {
            *pending = Some(self.new_chunk());
        }
        let chunk = pending.as_mut().expect("pending recording chunk");
        if !chunk.push_frame(left, right) {
            let full = pending.take().expect("full recording chunk");
            self.push_chunk_unchecked(full);
            let mut next = self.new_chunk();
            let _ = next.push_frame(left, right);
            *pending = Some(next);
        }
        let chunk = pending.as_ref().expect("pending recording chunk");
        advance_cursor(
            &self.state.next_frame,
            chunk
                .frame_offset()
                .saturating_add(chunk.frame_count() as u64),
        );
    }

    fn push_chunk_unchecked(&self, chunk: RecordingChunk) {
        let end = chunk
            .frame_offset()
            .saturating_add(chunk.frame_count() as u64);
        advance_cursor(&self.state.next_frame, end);
        match self.tx.try_send(chunk) {
            Ok(()) => {}
            Err(TrySendError::Full(chunk)) => {
                self.state.overflow_chunks.fetch_add(1, Ordering::Relaxed);
                self.state
                    .overflow_frames
                    .fetch_add(chunk.frame_count() as u64, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {
                self.state.accepting.store(false, Ordering::Release);
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.state.accepting.load(Ordering::Acquire)
    }

    pub fn audio_frame_cursor(&self) -> u64 {
        self.state.next_frame.load(Ordering::Acquire)
    }

    fn deactivate_and_flush(&self) {
        if let Ok(mut pending) = self.state.pending_chunk.lock() {
            if let Some(chunk) = pending.take() {
                self.push_chunk_unchecked(chunk);
            }
            self.state.accepting.store(false, Ordering::Release);
        } else {
            self.state.accepting.store(false, Ordering::Release);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingKind {
    AudioWav,
    AudioOledAvi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OledFrame {
    revision: u64,
    audio_frame: u64,
    pixels: Box<[u8; OLED_FRAME_BYTES]>,
}

impl OledFrame {
    pub fn new(revision: u64, audio_frame: u64, pixels: [u8; OLED_FRAME_BYTES]) -> Self {
        Self {
            revision,
            audio_frame,
            pixels: Box::new(pixels),
        }
    }

    pub fn from_bytes(
        revision: u64,
        audio_frame: u64,
        pixels: impl AsRef<[u8]>,
    ) -> Result<Self, OledFrameError> {
        let pixels = pixels.as_ref();
        let array = <[u8; OLED_FRAME_BYTES]>::try_from(pixels)
            .map_err(|_| OledFrameError::WrongSize(pixels.len()))?;
        Ok(Self::new(revision, audio_frame, array))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn audio_frame(&self) -> u64 {
        self.audio_frame
    }

    pub fn pixels(&self) -> &[u8; OLED_FRAME_BYTES] {
        &self.pixels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OledFrameError {
    WrongSize(usize),
}

impl fmt::Display for OledFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSize(size) => write!(
                formatter,
                "OLED frame has {size} bytes; expected {OLED_FRAME_BYTES}"
            ),
        }
    }
}

impl std::error::Error for OledFrameError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OledSubmission {
    pub accepted: bool,
    pub stale: bool,
    pub material_loss: bool,
    pub dropped_frames: u64,
}

#[derive(Clone, Debug)]
pub struct OledIngress {
    state: Arc<OledIngressState>,
}

impl OledIngress {
    pub fn try_submit(&self, frame: OledFrame) -> OledSubmission {
        if !self.state.accepting.load(Ordering::Acquire) {
            return OledSubmission {
                accepted: false,
                stale: false,
                material_loss: false,
                dropped_frames: 0,
            };
        }
        let Ok(mut queue) = self.state.queue.try_lock() else {
            self.state.material_loss.fetch_add(1, Ordering::Relaxed);
            return OledSubmission {
                accepted: false,
                stale: false,
                material_loss: true,
                dropped_frames: 1,
            };
        };
        if !self.state.accepting.load(Ordering::Acquire) {
            return OledSubmission {
                accepted: false,
                stale: false,
                material_loss: false,
                dropped_frames: 0,
            };
        }
        if self.state.has_revision.load(Ordering::Acquire)
            && frame.revision <= self.state.last_revision.load(Ordering::Acquire)
        {
            return OledSubmission {
                accepted: false,
                stale: true,
                material_loss: false,
                dropped_frames: 0,
            };
        }
        self.state
            .last_revision
            .store(frame.revision, Ordering::Release);
        self.state.has_revision.store(true, Ordering::Release);
        let dropped = if queue.len() == OLED_QUEUE_CAPACITY {
            queue.pop_front();
            1
        } else {
            0
        };
        queue.push_back(frame);
        if dropped != 0 {
            self.state
                .material_loss
                .fetch_add(dropped, Ordering::Relaxed);
        }
        OledSubmission {
            accepted: true,
            stale: false,
            material_loss: dropped != 0,
            dropped_frames: dropped,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state.accepting.load(Ordering::Acquire)
    }

    pub fn material_loss_count(&self) -> u64 {
        self.state.material_loss.load(Ordering::Acquire)
    }

    fn close(&self) {
        self.state.accepting.store(false, Ordering::Release);
    }

    fn drain(&self) -> Vec<OledFrame> {
        self.state
            .queue
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug)]
struct OledIngressState {
    accepting: AtomicBool,
    material_loss: AtomicU64,
    has_revision: AtomicBool,
    last_revision: AtomicU64,
    queue: Mutex<VecDeque<OledFrame>>,
}

impl OledIngressState {
    fn new() -> Self {
        Self {
            accepting: AtomicBool::new(true),
            material_loss: AtomicU64::new(0),
            has_revision: AtomicBool::new(false),
            last_revision: AtomicU64::new(0),
            queue: Mutex::new(VecDeque::with_capacity(OLED_QUEUE_CAPACITY)),
        }
    }
}

pub struct AudioOledRecording {
    pub tap: RecordingTap,
    pub oled: OledIngress,
}

#[derive(Debug)]
struct IngressState {
    accepting: AtomicBool,
    next_frame: AtomicU64,
    overflow_chunks: AtomicU64,
    overflow_frames: AtomicU64,
    pending_chunk: Mutex<Option<RecordingChunk>>,
}

impl IngressState {
    fn new() -> Self {
        Self {
            accepting: AtomicBool::new(true),
            next_frame: AtomicU64::new(0),
            overflow_chunks: AtomicU64::new(0),
            overflow_frames: AtomicU64::new(0),
            pending_chunk: Mutex::new(None),
        }
    }
}

fn advance_cursor(cursor: &AtomicU64, end: u64) {
    let mut current = cursor.load(Ordering::Relaxed);
    while current < end {
        match cursor.compare_exchange_weak(current, end, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum RecordingStatus {
    Complete,
    Incomplete {
        gap_count: u64,
        gap_frames: u64,
        overflow_count: u64,
        overflow_frames: u64,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct RecordingOutcome {
    pub path: PathBuf,
    pub frames_written: u64,
    pub status: RecordingStatus,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RecordingStartError {
    AlreadyActive,
    Io(String),
}

impl std::fmt::Display for RecordingStartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyActive => formatter.write_str("audio recording is already active"),
            Self::Io(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for RecordingStartError {}

#[derive(Debug, Eq, PartialEq)]
pub struct RecordingError {
    message: String,
    partial_path: PathBuf,
}

impl RecordingError {
    pub fn partial_path(&self) -> &Path {
        &self.partial_path
    }
}

impl std::fmt::Display for RecordingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RecordingError {}

#[cfg(test)]
mod avi_tests;
#[cfg(test)]
mod tests;
