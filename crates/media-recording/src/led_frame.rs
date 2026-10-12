use super::OledIngress;
use std::collections::VecDeque;
use std::sync::atomic::Ordering;

pub const GRID_LED_COUNT: usize = 64;
pub const NEOKEY_COUNT: usize = 4;
const LED_QUEUE_CAPACITY: usize = 32;

/// Full-brightness NeoTrellis colors in display order (top row first) and
/// NeoKey colors in Back, Space, Shift, Fn order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LedColors {
    pub grid: [[u8; 3]; GRID_LED_COUNT],
    pub keys: [[u8; 3]; NEOKEY_COUNT],
}

impl LedColors {
    pub const DARK: Self = Self {
        grid: [[0; 3]; GRID_LED_COUNT],
        keys: [[0; 3]; NEOKEY_COUNT],
    };

    pub fn from_grid_rgb(grid_rgb: &[u8], keys: [[u8; 3]; NEOKEY_COUNT]) -> Option<Self> {
        if grid_rgb.len() != GRID_LED_COUNT * 3 {
            return None;
        }
        let (cells, _) = grid_rgb.as_chunks::<3>();
        Some(Self {
            grid: std::array::from_fn(|index| cells[index]),
            keys,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LedFrame {
    audio_frame: u64,
    colors: LedColors,
}

impl LedFrame {
    pub fn new(audio_frame: u64, colors: LedColors) -> Self {
        Self {
            audio_frame,
            colors,
        }
    }

    pub fn audio_frame(&self) -> u64 {
        self.audio_frame
    }

    pub fn colors(&self) -> &LedColors {
        &self.colors
    }
}

#[derive(Debug, Default)]
pub(crate) struct LedQueue {
    frames: VecDeque<LedFrame>,
    last: Option<LedColors>,
}

impl OledIngress {
    /// Queues LED colors that changed since the previous submission; the
    /// oldest frame is dropped when the writer falls behind.
    pub fn submit_leds(&self, frame: LedFrame) -> bool {
        if !self.state.accepting.load(Ordering::Acquire) {
            return false;
        }
        let Ok(mut queue) = self.state.leds.lock() else {
            return false;
        };
        if queue.last == Some(frame.colors) {
            return false;
        }
        queue.last = Some(frame.colors);
        if queue.frames.len() == LED_QUEUE_CAPACITY {
            queue.frames.pop_front();
        }
        queue.frames.push_back(frame);
        true
    }

    pub(crate) fn drain_leds(&self) -> Vec<LedFrame> {
        self.state
            .leds
            .lock()
            .map(|mut queue| queue.frames.drain(..).collect())
            .unwrap_or_default()
    }
}

pub(crate) fn bound_pending_leds(pending: &mut Vec<LedFrame>) {
    if pending.len() > LED_QUEUE_CAPACITY {
        pending.drain(..pending.len() - LED_QUEUE_CAPACITY);
    }
}
