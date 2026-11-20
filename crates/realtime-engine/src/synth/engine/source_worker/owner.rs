use super::super::super::synth_voice_pool::SynthVoicePartition;
use super::super::routing_tree::worker::{RoutingTreeOwnerData, RoutingTreeWorkerContext};
use super::super::sample_voice_pool::SampleVoicePartition;
use super::super::source_worker_protocol::WorkStamp;

pub(in crate::synth::engine) struct SourceLanePartitionBundle {
    pub(in crate::synth::engine) synth: Box<SynthVoicePartition>,
    pub(in crate::synth::engine) sample: Box<SampleVoicePartition>,
}

pub(in crate::synth::engine) struct OwnerEnvelope {
    pub(in crate::synth::engine) runtime_generation: u64,
    pub(in crate::synth::engine) parity: usize,
    pub(in crate::synth::engine) partitions: SourceLanePartitionBundle,
    pub(in crate::synth::engine) bus_carriers: [Option<
        super::super::bus_chain_owner::BusChainCarrier,
    >; super::super::super::types::BUS_COUNT],
    pub(in crate::synth::engine) routing_tree: Option<RoutingTreeOwnerData>,
}

pub(in crate::synth::engine) struct RoutingTreeWork {
    pub(super) owner: OwnerEnvelope,
    pub(super) stamp: WorkStamp,
    pub(super) context: RoutingTreeWorkerContext,
}

impl RoutingTreeWork {
    pub(super) fn render(&mut self) -> Result<u16, ()> {
        super::super::routing_tree::worker::render_owner(&mut self.owner, self.context, self.stamp)
    }

    pub(super) fn active_cost_units(&self) -> u16 {
        let (synth_count, sample_count, preview_count) = self
            .owner
            .routing_tree
            .as_ref()
            .and_then(|routing| {
                routing.source_bank.as_ref().map(|bank| {
                    (
                        bank.synth.iter().filter(|voice| voice.active).count(),
                        bank.sample.iter().filter(|voice| voice.active).count(),
                        routing
                            .preview_sample_voices
                            .iter()
                            .filter(|voice| voice.is_some())
                            .count(),
                    )
                })
            })
            .unwrap_or((0, 0, 0));
        let synth_units =
            synth_count * super::super::source_worker_load::SOURCE_WORKER_SYNTH_COST_UNITS as usize;
        let sample_units = (sample_count + preview_count)
            * super::super::source_worker_load::SOURCE_WORKER_SAMPLE_COST_UNITS as usize;
        synth_units
            .saturating_add(sample_units)
            .min(usize::from(u16::MAX)) as u16
    }
}

pub(in crate::synth::engine) struct CompletedEnvelope {
    pub(super) owner: OwnerEnvelope,
    pub(super) stamp: WorkStamp,
    pub(super) render_ok: bool,
    pub(super) worker_exited: bool,
    pub(super) transport_failed: bool,
    pub(super) dsp_duration_ns: u64,
    pub(super) active_cost_units: u16,
}

pub(in crate::synth::engine) struct WorkerExit {
    pub(super) unsent_completion: Option<CompletedEnvelope>,
}

impl CompletedEnvelope {
    pub(super) fn from_routing_tree_work(
        work: RoutingTreeWork,
        worker_exited: bool,
        render_ok: bool,
        dsp_duration_ns: u64,
        active_cost_units: u16,
    ) -> Self {
        Self {
            owner: work.owner,
            stamp: work.stamp,
            render_ok,
            worker_exited,
            transport_failed: false,
            dsp_duration_ns,
            active_cost_units,
        }
    }
}
