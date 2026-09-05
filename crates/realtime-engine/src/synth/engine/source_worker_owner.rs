use super::super::dsp_config::BusIdleThreshold;
#[cfg(feature = "source-worker-benchmark-timing")]
use super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::super::synth_voice_pool::SynthVoicePartition;
#[cfg(feature = "routing-tree-benchmark")]
use super::routing_tree_worker::{RoutingTreeOwnerData, RoutingTreeWorkerContext};
use super::sample_voice_pool::SampleVoicePartition;
use super::source_lane_renderer::{
    render_sample_partition, render_synth_partition, SampleSourceContext, SourceLaneBlockScratch,
    SynthSourceContext,
};
use super::source_worker_protocol::{WorkStamp, WorkerPhase};
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
    pub(super) runtime_generation: u64,
    pub(super) parity: usize,
    pub(super) partitions: SourceLanePartitionBundle,
    pub(super) scratch: SourceWorkerScratch,
    pub(super) bus_carriers:
        [Option<super::bus_chain_owner::BusChainCarrier>; super::super::types::BUS_COUNT],
    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) routing_tree: Option<RoutingTreeOwnerData>,
}

#[allow(clippy::large_enum_variant)]
pub(super) enum WorkerCommand {
    Sources {
        stamp: WorkStamp,
        owner: OwnerEnvelope,
        synth_context: SynthSourceContext,
        sample_context: SampleSourceContext,

        #[cfg(feature = "source-worker-benchmark-timing")]
        dispatch_started_at: Option<Instant>,
        #[cfg(feature = "source-worker-benchmark-timing")]
        timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
    },
    Buses {
        stamp: WorkStamp,
        owner: OwnerEnvelope,
        frames: usize,
        sample_rate: u32,
        bus_idle_threshold: BusIdleThreshold,
        fx_activity_hold_frames: u32,
        #[cfg(feature = "source-worker-benchmark-timing")]
        dispatch_started_at: Option<Instant>,
        #[cfg(feature = "source-worker-benchmark-timing")]
        timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
    },
    #[cfg(feature = "routing-tree-benchmark")]
    RoutingTree {
        stamp: WorkStamp,
        owner: OwnerEnvelope,
        context: RoutingTreeWorkerContext,
        #[cfg(feature = "source-worker-benchmark-timing")]
        dispatch_started_at: Option<Instant>,
        #[cfg(feature = "source-worker-benchmark-timing")]
        timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
    },
}

pub(super) struct SourceWork {
    pub(super) owner: OwnerEnvelope,
    pub(super) stamp: WorkStamp,
    pub(super) synth_context: SynthSourceContext,
    pub(super) sample_context: SampleSourceContext,
    #[cfg(feature = "source-worker-benchmark-timing")]
    pub(super) dispatch_started_at: Option<Instant>,
    #[cfg(feature = "source-worker-benchmark-timing")]
    pub(super) timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
}

#[cfg(feature = "routing-tree-benchmark")]
pub(super) struct RoutingTreeWork {
    pub(super) owner: OwnerEnvelope,
    pub(super) stamp: WorkStamp,
    pub(super) context: RoutingTreeWorkerContext,
    #[cfg(feature = "source-worker-benchmark-timing")]
    pub(super) dispatch_started_at: Option<Instant>,
    #[cfg(feature = "source-worker-benchmark-timing")]
    pub(super) timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
}

impl WorkerCommand {
    pub(super) fn into_source_work(self) -> Option<SourceWork> {
        let Self::Sources {
            stamp,
            owner,
            synth_context,
            sample_context,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe,
        } = self
        else {
            return None;
        };
        Some(SourceWork {
            owner,
            stamp,
            synth_context,
            sample_context,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe,
        })
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn into_routing_tree_work(self) -> Option<RoutingTreeWork> {
        let Self::RoutingTree {
            owner,
            stamp,
            context,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe,
        } = self
        else {
            return None;
        };
        Some(RoutingTreeWork {
            owner,
            stamp,
            context,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe,
        })
    }
}

impl SourceWork {
    pub(super) fn render(&mut self) -> bool {
        if !self.owner.scratch.sample.prepare(self.stamp.frames)
            || !self.owner.scratch.synth.prepare(self.stamp.frames)
        {
            return false;
        }
        render_sample_partition(
            &mut self.owner.partitions.sample,
            self.stamp.frames,
            self.sample_context,
            &mut self.owner.scratch.sample,
        );
        render_synth_partition(
            &mut self.owner.partitions.synth,
            self.stamp.frames,
            self.stamp.base_sample_clock,
            &self.synth_context,
            &mut self.owner.scratch.synth,
        );
        true
    }

    pub(super) fn active_cost_units(&self) -> u16 {
        let synth_units = self.owner.partitions.synth.render_lane_count
            * super::source_worker_load::SOURCE_WORKER_SYNTH_COST_UNITS as usize;
        let sample_units = self.owner.partitions.sample.render_lane_count
            * super::source_worker_load::SOURCE_WORKER_SAMPLE_COST_UNITS as usize;
        synth_units
            .saturating_add(sample_units)
            .min(usize::from(u16::MAX)) as u16
    }
}

#[cfg(feature = "routing-tree-benchmark")]
impl RoutingTreeWork {
    pub(super) fn render(&mut self) -> Result<u16, ()> {
        super::routing_tree_worker::render_owner(&mut self.owner, self.context, self.stamp)
    }

    pub(super) fn active_cost_units(&self) -> u16 {
        let synth_units = self.owner.partitions.synth.render_lane_count
            * super::source_worker_load::SOURCE_WORKER_SYNTH_COST_UNITS as usize;
        let sample_units = self.owner.partitions.sample.render_lane_count
            * super::source_worker_load::SOURCE_WORKER_SAMPLE_COST_UNITS as usize;
        synth_units
            .saturating_add(sample_units)
            .min(usize::from(u16::MAX)) as u16
    }
}

#[cfg(test)]
#[path = "source_worker_owner_tests.rs"]
mod tests;

pub(super) struct CompletedEnvelope {
    pub(super) owner: OwnerEnvelope,
    pub(super) phase: WorkerPhase,
    pub(super) stamp: WorkStamp,
    pub(super) render_ok: bool,
    pub(super) worker_exited: bool,
    pub(super) transport_failed: bool,
    pub(super) dsp_duration_ns: u64,
    pub(super) active_cost_units: u16,
}

pub(super) struct WorkerExit {
    pub(super) unsent_completion: Option<CompletedEnvelope>,
}

impl CompletedEnvelope {
    pub(super) fn from_bus_work(
        owner: OwnerEnvelope,
        stamp: WorkStamp,
        worker_exited: bool,
        render_ok: bool,
        dsp_duration_ns: u64,
        active_cost_units: u16,
    ) -> Self {
        Self {
            owner,
            phase: WorkerPhase::Buses,
            stamp,
            render_ok,
            worker_exited,
            transport_failed: false,
            dsp_duration_ns,
            active_cost_units,
        }
    }

    pub(super) fn from_work(
        work: SourceWork,
        worker_exited: bool,
        render_ok: bool,
        dsp_duration_ns: u64,
        active_cost_units: u16,
    ) -> Self {
        Self {
            owner: work.owner,
            phase: WorkerPhase::Sources,
            stamp: work.stamp,
            render_ok,
            worker_exited,
            transport_failed: false,
            dsp_duration_ns,
            active_cost_units,
        }
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn from_routing_tree_work(
        work: RoutingTreeWork,
        worker_exited: bool,
        render_ok: bool,
        dsp_duration_ns: u64,
        active_cost_units: u16,
    ) -> Self {
        Self {
            owner: work.owner,
            phase: WorkerPhase::RoutingTree,
            stamp: work.stamp,
            render_ok,
            worker_exited,
            transport_failed: false,
            dsp_duration_ns,
            active_cost_units,
        }
    }
}
