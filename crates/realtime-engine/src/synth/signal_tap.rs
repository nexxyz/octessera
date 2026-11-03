//! One selected signal, carried from the audio thread to the display through a preallocated
//! lock-free ring. The audio side writes once per block, and only while a visual reads it.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

pub const SIGNAL_TAP_CAPACITY: usize = 2048;
/// Each tapped sample is the mono average of this many output frames.
pub const SIGNAL_TAP_DECIMATION: usize = 2;
/// The interleaved samples one tapped sample averages.
const GROUP: usize = 2 * SIGNAL_TAP_DECIMATION;
const MONO_SCALE: f32 = 1.0 / GROUP as f32;
const OFF: u8 = 0;
const MASTER: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalSource {
    /// The final stereo mix the outputs play.
    Master,
}

pub struct SignalTap {
    source: AtomicU8,
    sample_rate: AtomicU32,
    written: AtomicU64,
    samples: [AtomicU32; SIGNAL_TAP_CAPACITY],
}

static MASTER_SIGNAL_TAP: SignalTap = SignalTap::new();

/// The process's tap: every engine source writes the master mix into it.
pub fn master_signal_tap() -> &'static SignalTap {
    &MASTER_SIGNAL_TAP
}

impl Default for SignalTap {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalTap {
    pub const fn new() -> Self {
        Self {
            source: AtomicU8::new(OFF),
            sample_rate: AtomicU32::new(0),
            written: AtomicU64::new(0),
            samples: [const { AtomicU32::new(0) }; SIGNAL_TAP_CAPACITY],
        }
    }

    /// Starts tapping `source`, or stops the writes with `None`.
    pub fn select(&self, source: Option<SignalSource>) {
        let code = match source {
            None => OFF,
            Some(SignalSource::Master) => MASTER,
        };
        self.source.store(code, Ordering::Relaxed);
    }

    /// Writes one rendered block of the master mix while the master is selected.
    pub fn publish_master(&self, interleaved: &[f32], frames: usize, sample_rate: u32) {
        if self.source.load(Ordering::Relaxed) != MASTER {
            return;
        }
        let frames = frames.min(interleaved.len() / 2);
        let start = self.written.load(Ordering::Relaxed);
        let mut position = start;
        for group in interleaved[..frames * 2].as_chunks::<GROUP>().0 {
            let mono = group.iter().sum::<f32>() * MONO_SCALE;
            let slot = position as usize % SIGNAL_TAP_CAPACITY;
            self.samples[slot].store(mono.to_bits(), Ordering::Relaxed);
            position += 1;
        }
        self.sample_rate.store(
            sample_rate / SIGNAL_TAP_DECIMATION as u32,
            Ordering::Relaxed,
        );
        self.written.store(position, Ordering::Release);
    }

    /// How many samples the tap has taken in total.
    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Acquire)
    }

    /// The rate of the tapped samples, 0 before the first write.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// Fills the end of `out` with the newest samples taken after the `since` count, oldest
    /// first, and zeroes the rest; returns how many it copied.
    pub fn read_latest(&self, since: u64, out: &mut [f32]) -> usize {
        let end = self.written();
        let count = end
            .saturating_sub(since)
            .min(out.len() as u64)
            .min(SIGNAL_TAP_CAPACITY as u64) as usize;
        let gap = out.len() - count;
        out[..gap].fill(0.0);
        for (index, sample) in out[gap..].iter_mut().enumerate() {
            let position = end - count as u64 + index as u64;
            let slot = position as usize % SIGNAL_TAP_CAPACITY;
            *sample = f32::from_bits(self.samples[slot].load(Ordering::Relaxed));
        }
        count
    }
}

#[cfg(test)]
mod tests;
