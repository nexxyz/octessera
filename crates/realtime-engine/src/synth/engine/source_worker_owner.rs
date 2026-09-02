#[cfg(feature = "source-worker-benchmark-timing")]
use super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::super::synth_voice_pool::SynthVoicePartition;
use super::sample_voice_pool::SampleVoicePartition;
use super::source_lane_renderer::{
    render_sample_partition, render_synth_partition, SampleSourceContext, SourceLaneBlockScratch,
    SynthSourceContext,
};
#[cfg(feature = "source-worker-benchmark-timing")]
use std::sync::Arc;
#[cfg(feature = "source-worker-benchmark-timing")]
use std::time::Instant;

pub(super) struct SourceWorkerScratch {
    pub(super) synth: SourceLaneBlockScratch,
    pub(super) sample: SourceLaneBlockScratch,
}

impl SourceWorkerScratch {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self {
            synth: SourceLaneBlockScratch::new(),
            sample: SourceLaneBlockScratch::new(),
        }
    }

    pub(super) fn from_inline_scratch(
        synth: [SourceLaneBlockScratch; 2],
        sample: [SourceLaneBlockScratch; 2],
    ) -> Option<[Self; 2]> {
        let mut synth = synth.map(Some);
        let mut sample = sample.map(Some);
        Some([
            Self {
                synth: synth[0].take()?,
                sample: sample[0].take()?,
            },
            Self {
                synth: synth[1].take()?,
                sample: sample[1].take()?,
            },
        ])
    }
}

pub(super) struct SourceLanePartitionBundle {
    pub(super) synth: Box<SynthVoicePartition>,
    pub(super) sample: Box<SampleVoicePartition>,
}

pub(super) struct OwnerEnvelope {
    pub(super) parity: usize,
    pub(super) partitions: SourceLanePartitionBundle,
    pub(super) scratch: SourceWorkerScratch,
}

pub(super) struct WorkEnvelope {
    pub(super) owner: OwnerEnvelope,
    pub(super) sequence: u64,
    pub(super) frames: usize,
    pub(super) base_sample_clock: u64,
    pub(super) synth_context: SynthSourceContext,
    pub(super) sample_context: SampleSourceContext,
    #[cfg(feature = "source-worker-benchmark-timing")]
    pub(super) dispatch_started_at: Option<Instant>,
    #[cfg(feature = "source-worker-benchmark-timing")]
    pub(super) timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
}

impl WorkEnvelope {
    pub(super) fn render(&mut self) -> bool {
        if !self.owner.scratch.sample.prepare(self.frames)
            || !self.owner.scratch.synth.prepare(self.frames)
        {
            return false;
        }
        render_sample_partition(
            &mut self.owner.partitions.sample,
            self.frames,
            self.sample_context,
            &mut self.owner.scratch.sample,
        );
        render_synth_partition(
            &mut self.owner.partitions.synth,
            self.frames,
            self.base_sample_clock,
            &self.synth_context,
            &mut self.owner.scratch.synth,
        );
        true
    }
}

pub(super) struct CompletedEnvelope {
    pub(super) owner: OwnerEnvelope,
    pub(super) sequence: u64,
    pub(super) frames: usize,
    pub(super) base_sample_clock: u64,
    pub(super) render_ok: bool,
    pub(super) worker_exited: bool,
    pub(super) transport_failed: bool,
}

pub(super) struct WorkerExit {
    pub(super) unsent_completion: Option<CompletedEnvelope>,
}
