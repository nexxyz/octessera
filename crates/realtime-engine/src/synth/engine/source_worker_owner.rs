use super::super::synth_voice_pool::SynthVoicePartition;
use super::routing_tree_worker::{RoutingTreeOwnerData, RoutingTreeWorkerContext};
use super::sample_voice_pool::SampleVoicePartition;
use super::source_worker_protocol::WorkStamp;

pub(super) struct SourceLanePartitionBundle {
    pub(super) synth: Box<SynthVoicePartition>,
    pub(super) sample: Box<SampleVoicePartition>,
}

pub(super) struct OwnerEnvelope {
    pub(super) runtime_generation: u64,
    pub(super) parity: usize,
    pub(super) partitions: SourceLanePartitionBundle,
    pub(super) bus_carriers:
        [Option<super::bus_chain_owner::BusChainCarrier>; super::super::types::BUS_COUNT],
    pub(super) routing_tree: Option<RoutingTreeOwnerData>,
}

pub(super) struct RoutingTreeWork {
    pub(super) owner: OwnerEnvelope,
    pub(super) stamp: WorkStamp,
    pub(super) context: RoutingTreeWorkerContext,
}

impl RoutingTreeWork {
    pub(super) fn render(&mut self) -> Result<u16, ()> {
        super::routing_tree_worker::render_owner(&mut self.owner, self.context, self.stamp)
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
            synth_count * super::source_worker_load::SOURCE_WORKER_SYNTH_COST_UNITS as usize;
        let sample_units = (sample_count + preview_count)
            * super::source_worker_load::SOURCE_WORKER_SAMPLE_COST_UNITS as usize;
        synth_units
            .saturating_add(sample_units)
            .min(usize::from(u16::MAX)) as u16
    }
}

pub(super) struct CompletedEnvelope {
    pub(super) owner: OwnerEnvelope,
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
