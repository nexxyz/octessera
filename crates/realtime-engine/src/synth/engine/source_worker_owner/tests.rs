use super::*;
use crate::synth::engine::source_worker_lifecycle::owner_for_test;
use crate::synth::engine::SynthEngine;

#[test]
fn active_cost_units_uses_sparse_render_lane_counts() {
    let engine = SynthEngine::new(48_000);
    let mut owner = owner_for_test(0);
    owner.partitions.synth.render_lane_count = 2;
    owner.partitions.sample.render_lane_count = 3;
    let work = SourceWork {
        owner,
        stamp: WorkStamp {
            runtime_generation: 1,
            render_plan_generation: 0,
            quantum_sequence: 0,
            frames: 0,
            base_sample_clock: 0,
        },
        synth_context: engine.synth_source_context(),
        sample_context: SampleSourceContext {
            sample_rate: 48_000,
        },
        #[cfg(feature = "source-worker-benchmark-timing")]
        dispatch_started_at: None,
        #[cfg(feature = "source-worker-benchmark-timing")]
        timing_probe: None,
    };

    assert_eq!(work.active_cost_units(), 2 * 3 + 3 * 2);
}
