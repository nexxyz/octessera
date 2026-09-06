use std::sync::atomic::{fence, AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub const PCM_MIRROR_CAPACITY_FRAMES: usize = 1_024;
pub const PCM_MIRROR_TARGET_OCCUPANCY_FRAMES: usize = 256;
const FRAME_SEQUENCE_MASK: u64 = (1_u64 << 63) - 1;

pub type PcmMirrorProducers = [Option<PcmMirrorProducer>; 2];

struct MirrorSlot {
    state: AtomicU64,
    left: AtomicU32,
    right: AtomicU32,
}

impl MirrorSlot {
    const fn new() -> Self {
        Self {
            state: AtomicU64::new(u64::MAX),
            left: AtomicU32::new(0),
            right: AtomicU32::new(0),
        }
    }
}

struct MirrorRing {
    slots: [MirrorSlot; PCM_MIRROR_CAPACITY_FRAMES],
    active: AtomicBool,
    generation: AtomicU64,
    write_sequence: AtomicU64,
}

impl MirrorRing {
    fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| MirrorSlot::new()),
            active: AtomicBool::new(true),
            generation: AtomicU64::new(0),
            write_sequence: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct PcmMirrorProducer {
    ring: Arc<MirrorRing>,
}

pub struct PcmMirrorConsumer {
    ring: Arc<MirrorRing>,
    observed_generation: u64,
    read_sequence: u64,
    current_sequence: u64,
    current_right: u32,
    current_channel: u8,
    priming: bool,
    callback_active: bool,
}

pub struct PcmMirrorPair {
    pub producer: PcmMirrorProducer,
    pub consumer: PcmMirrorConsumer,
}

pub fn new_pcm_mirror() -> PcmMirrorPair {
    let producer = PcmMirrorProducer {
        ring: Arc::new(MirrorRing::new()),
    };
    let consumer = producer.new_consumer();
    PcmMirrorPair { producer, consumer }
}

impl PcmMirrorProducer {
    pub fn new_consumer(&self) -> PcmMirrorConsumer {
        let generation = self.ring.generation.load(Ordering::Acquire);
        let write_sequence = self.ring.write_sequence.load(Ordering::Acquire);
        PcmMirrorConsumer {
            ring: self.ring.clone(),
            observed_generation: generation,
            read_sequence: initial_read_sequence(write_sequence),
            current_sequence: 0,
            current_right: 0,
            current_channel: 0,
            priming: false,
            callback_active: false,
        }
    }

    pub fn publish(&self, interleaved: &[f32], frames: usize) {
        if !self.ring.active.load(Ordering::Acquire) {
            return;
        }
        let frame_count = frames.min(interleaved.len() / 2);
        let mut sequence = self.ring.write_sequence.load(Ordering::Relaxed) & FRAME_SEQUENCE_MASK;
        for frame in 0..frame_count {
            let slot = &self.ring.slots[sequence as usize % PCM_MIRROR_CAPACITY_FRAMES];
            // The SeqCst odd marker and fence order the marker before relaxed payload stores.
            // The final release publishes both words; readers acquire the initial even marker,
            // read both words, acquire-fence, then validate an identical even marker.
            slot.state
                .store(sequence_state(sequence, true), Ordering::SeqCst);
            fence(Ordering::SeqCst);
            slot.left
                .store(interleaved[frame * 2].to_bits(), Ordering::Relaxed);
            slot.right
                .store(interleaved[frame * 2 + 1].to_bits(), Ordering::Relaxed);
            slot.state
                .store(sequence_state(sequence, false), Ordering::Release);
            sequence = next_sequence(sequence);
        }
        self.ring.write_sequence.store(sequence, Ordering::Release);
    }

    pub fn invalidate(&self) {
        self.ring.generation.fetch_add(1, Ordering::AcqRel);
        self.ring.active.store(false, Ordering::Release);
    }

    pub fn reactivate(&self) {
        self.ring.active.store(true, Ordering::Release);
    }
}

impl PcmMirrorConsumer {
    pub fn begin_callback(&mut self) -> bool {
        self.callback_active = false;
        self.current_channel = 0;
        let generation = self.ring.generation.load(Ordering::Acquire);
        let write_sequence = self.ring.write_sequence.load(Ordering::Acquire);
        if generation != self.observed_generation {
            self.reset_for_generation(generation, write_sequence);
            return false;
        }
        if !self.ring.active.load(Ordering::Acquire) {
            self.reset_for_discontinuity(write_sequence);
            return false;
        }
        if frame_distance(write_sequence, self.read_sequence) > PCM_MIRROR_CAPACITY_FRAMES as u64 {
            self.reset_for_discontinuity(write_sequence);
            return false;
        }
        if self.priming {
            if frame_distance(write_sequence, self.read_sequence)
                < PCM_MIRROR_TARGET_OCCUPANCY_FRAMES as u64
            {
                return false;
            }
            self.read_sequence =
                sequence_subtract(write_sequence, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES as u64);
            self.priming = false;
        }
        if self.read_sequence == write_sequence {
            self.reset_for_discontinuity(write_sequence);
            return false;
        }
        self.callback_active = true;
        true
    }

    fn continuity_is_valid(&mut self) -> bool {
        let generation = self.ring.generation.load(Ordering::Acquire);
        let write_sequence = self.ring.write_sequence.load(Ordering::Acquire);
        if generation != self.observed_generation {
            self.reset_for_generation(generation, write_sequence);
            return false;
        }
        if !self.ring.active.load(Ordering::Acquire)
            || frame_distance(write_sequence, self.read_sequence)
                > PCM_MIRROR_CAPACITY_FRAMES as u64
        {
            self.reset_for_discontinuity(write_sequence);
            return false;
        }
        true
    }

    fn reset_for_generation(&mut self, generation: u64, write_sequence: u64) {
        self.observed_generation = generation;
        self.reset_for_discontinuity(write_sequence);
    }

    fn reset_for_discontinuity(&mut self, write_sequence: u64) {
        self.read_sequence = write_sequence;
        self.current_sequence = 0;
        self.current_channel = 0;
        self.priming = true;
        self.callback_active = false;
    }

    fn current_frame_is_stable(&self) -> bool {
        let expected = sequence_state(self.current_sequence, false);
        let state = self.ring.slots[self.current_sequence as usize % PCM_MIRROR_CAPACITY_FRAMES]
            .state
            .load(Ordering::Relaxed);
        state == expected && state & 1 == 0
    }

    fn read_frame(&self, sequence: u64) -> Option<(u32, u32)> {
        let slot = &self.ring.slots[sequence as usize % PCM_MIRROR_CAPACITY_FRAMES];
        let expected = sequence_state(sequence, false);
        let first = slot.state.load(Ordering::Acquire);
        if first != expected || first & 1 != 0 {
            return None;
        }
        let left = slot.left.load(Ordering::Relaxed);
        let right = slot.right.load(Ordering::Relaxed);
        fence(Ordering::Acquire);
        let second = slot.state.load(Ordering::Relaxed);
        if second == first && second == expected && second & 1 == 0 {
            Some((left, right))
        } else {
            None
        }
    }

    fn abort_after_discontinuity(&mut self) {
        let write_sequence = self.ring.write_sequence.load(Ordering::Acquire);
        self.reset_for_discontinuity(write_sequence);
    }

    pub fn next_sample(&mut self) -> Option<f32> {
        if !self.callback_active || !self.continuity_is_valid() {
            return None;
        }
        if self.current_channel == 1 {
            if !self.current_frame_is_stable() || !self.continuity_is_valid() {
                self.abort_after_discontinuity();
                return None;
            }
            self.current_channel = 0;
            return Some(f32::from_bits(self.current_right));
        }
        if self.read_sequence == self.ring.write_sequence.load(Ordering::Acquire) {
            self.abort_after_discontinuity();
            return None;
        }
        let sequence = self.read_sequence;
        let Some((left, right)) = self.read_frame(sequence) else {
            self.abort_after_discontinuity();
            return None;
        };
        if !self.continuity_is_valid() {
            return None;
        }
        self.current_sequence = sequence;
        self.current_right = right;
        self.current_channel = 1;
        self.read_sequence = next_sequence(self.read_sequence);
        Some(f32::from_bits(left))
    }
}

fn sequence_state(sequence: u64, in_progress: bool) -> u64 {
    sequence
        .wrapping_shl(1)
        .wrapping_add(u64::from(in_progress))
}

fn frame_distance(write_sequence: u64, read_sequence: u64) -> u64 {
    write_sequence.wrapping_sub(read_sequence) & FRAME_SEQUENCE_MASK
}

fn next_sequence(sequence: u64) -> u64 {
    sequence.wrapping_add(1) & FRAME_SEQUENCE_MASK
}

fn sequence_subtract(sequence: u64, frames: u64) -> u64 {
    sequence.wrapping_sub(frames) & FRAME_SEQUENCE_MASK
}

fn initial_read_sequence(write_sequence: u64) -> u64 {
    if write_sequence < PCM_MIRROR_TARGET_OCCUPANCY_FRAMES as u64 {
        write_sequence
    } else {
        sequence_subtract(write_sequence, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES as u64)
    }
}

#[cfg(test)]
#[path = "pcm_mirror_tests.rs"]
mod tests;
