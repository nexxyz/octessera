use super::*;
use crate::synth::engine::source_worker_test_fixtures::dynamic_engine;
use crate::synth::engine::{SOURCE_WORKER_SAMPLE_COST_UNITS, SOURCE_WORKER_SYNTH_COST_UNITS};
use crate::synth::types::{SampleBankConfig, SampleBuffer, SampleSlotConfig};
use std::thread;

#[test]
fn completion_reports_pre_render_source_cost_when_all_sources_finish() {
    let mut engine = dynamic_engine();
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![1.0].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    let _ = engine.set_sample_banks(vec![bank]);
    engine.set_synth_param(1, "synth.ampEnv.releaseMs", 0.0);
    engine.note_on(0, 36, 100, 10_000);
    engine.note_on(1, 60, 100, 1);
    assert_eq!(engine.synth_voice_pool.active_count_for_parity(0), Some(1));
    assert_eq!(engine.sample_voice_pool.active_count_for_parity(0), Some(1));

    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, 128)
            .expect("worker runtime");
    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    let mut measurements = [0; 2];
    for (parity, measurement) in measurements.iter_mut().enumerate() {
        let mut ready = false;
        for _ in 0..10_000 {
            if runtime.completion_ready_for_test(parity) {
                ready = true;
                break;
            }
            thread::yield_now();
        }
        assert!(ready, "worker {parity} did not complete");
        *measurement = runtime
            .completion_measurement_for_test(parity)
            .expect("worker completion")
            .1;
    }
    assert_eq!(
        measurements,
        [
            SOURCE_WORKER_SYNTH_COST_UNITS + SOURCE_WORKER_SAMPLE_COST_UNITS,
            0
        ]
    );
    assert!(runtime.collect_wait_for_test(&mut engine));
    let owner = runtime.take_home_owner_for_test(0).expect("home owner");
    assert_eq!(owner.partitions.synth.render_lane_count, 0);
    assert_eq!(owner.partitions.sample.render_lane_count, 0);
    runtime.return_home_owner_for_test(owner);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
