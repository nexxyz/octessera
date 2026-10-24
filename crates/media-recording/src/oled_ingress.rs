use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::{led_frame, OLED_FRAME_BYTES, OLED_QUEUE_CAPACITY};

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
    pub(crate) state: Arc<OledIngressState>,
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

    pub(crate) fn close(&self) {
        self.state.accepting.store(false, Ordering::Release);
    }

    pub(crate) fn drain(&self) -> Vec<OledFrame> {
        self.state
            .queue
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug)]
pub(crate) struct OledIngressState {
    pub(crate) accepting: AtomicBool,
    material_loss: AtomicU64,
    has_revision: AtomicBool,
    last_revision: AtomicU64,
    pub(crate) queue: Mutex<VecDeque<OledFrame>>,
    pub(crate) leds: Mutex<led_frame::LedQueue>,
}

impl OledIngressState {
    pub(crate) fn new() -> Self {
        Self {
            accepting: AtomicBool::new(true),
            material_loss: AtomicU64::new(0),
            has_revision: AtomicBool::new(false),
            last_revision: AtomicU64::new(0),
            queue: Mutex::new(VecDeque::with_capacity(OLED_QUEUE_CAPACITY)),
            leds: Mutex::new(led_frame::LedQueue::default()),
        }
    }
}
