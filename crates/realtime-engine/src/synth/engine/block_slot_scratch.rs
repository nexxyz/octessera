use super::super::types::INSTRUMENT_SLOT_COUNT;
use super::inline_source_executor::InlineSourceExecutor;
use super::BLOCK_SLOT_SCRATCH_FRAMES;

pub(super) struct BlockSlotScratch {
    pub(super) inline_source_executor: Option<InlineSourceExecutor>,
    pub(super) sample_slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub(super) synth_slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub(super) sample_active: [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    pub(super) synth_active: [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    pub(super) source_active: Vec<bool>,
    pub(super) bus_active: Vec<bool>,
}

impl BlockSlotScratch {
    pub(super) fn new() -> Self {
        Self {
            inline_source_executor: Some(InlineSourceExecutor::new()),
            sample_slot_out: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_slot_out: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            sample_active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            source_active: vec![false; BLOCK_SLOT_SCRATCH_FRAMES],
            bus_active: vec![false; BLOCK_SLOT_SCRATCH_FRAMES],
        }
    }

    pub(super) fn prepare_output(&mut self, frames: usize) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for buffer in &mut self.sample_slot_out {
            buffer[..frames].fill(0.0);
        }
        for buffer in &mut self.synth_slot_out {
            buffer[..frames].fill(0.0);
        }
        for buffer in &mut self.sample_active {
            buffer[..frames].fill(false);
        }
        for buffer in &mut self.synth_active {
            buffer[..frames].fill(false);
        }
        self.source_active[..frames].fill(false);
        self.bus_active[..frames].fill(false);
        true
    }
}
