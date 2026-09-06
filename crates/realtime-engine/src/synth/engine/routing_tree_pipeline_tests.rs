#[cfg(feature = "source-worker-benchmark-timing")]
use super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::routing_tree_executor_test_support::{
    assert_momentary_state_matches, assert_reassociated_close,
};
use super::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MixerConfig,
};
use super::{
    SourceWorkerHealth, SourceWorkerLifecycle, SourceWorkerRenderDisposition, SynthEngine,
};
use crate::synth::types::{default_synth_config, DEFAULT_PAN_POSITIONS};
use crate::synth::{
    RoutingTreePipelineProbe, SourceWorkerRuntime, BUS_CHAIN_SLOT_COST_UNITS,
    ROUTING_TREE_WORKER_THREAD_NAMES, SOURCE_WORKER_SYNTH_COST_UNITS,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn start_runtime() -> (SynthEngine, SourceWorkerLifecycle, SourceWorkerRuntime) {
    let mut engine = SynthEngine::new(44_100);
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    let mut runtime = runtime;
    runtime.set_deadline_for_test(Duration::from_secs(1));
    (engine, lifecycle, runtime)
}

fn render_block(
    engine: &mut SynthEngine,
    runtime: &mut SourceWorkerRuntime,
) -> SourceWorkerRenderDisposition {
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    engine
        .render_interleaved_block_with_source_runtime(runtime, 128, &mut left, &mut right, &mut out)
}

pub(super) fn shutdown(lifecycle: SourceWorkerLifecycle, runtime: SourceWorkerRuntime) {
    let retirement = runtime.retire();
    let report = lifecycle.shutdown(retirement);
    assert_eq!(report.joined_workers, 2);
}

pub(super) fn assert_interleaved_reassociated_close(
    runtime: &SourceWorkerRuntime,
    actual: &[f32],
    expected: &[f32],
) {
    assert_eq!(actual.len(), expected.len());
    for frame in 0..actual.len() / 2 {
        let workers = runtime.routing_tree_worker_outputs_for_test(frame);
        assert_reassociated_close(
            actual[frame * 2],
            expected[frame * 2],
            workers,
            0,
            "routing-tree left frame",
        );
        assert_reassociated_close(
            actual[frame * 2 + 1],
            expected[frame * 2 + 1],
            workers,
            1,
            "routing-tree right frame",
        );
    }
}

pub(super) fn assert_worker_outputs_are_nonzero(runtime: &SourceWorkerRuntime, frames: usize) {
    let mut peak_by_worker = [0.0_f32; 2];
    for frame in 0..frames {
        for (worker, (left, right)) in runtime
            .routing_tree_worker_outputs_for_test(frame)
            .into_iter()
            .enumerate()
        {
            peak_by_worker[worker] = peak_by_worker[worker].max(left.abs()).max(right.abs());
        }
    }
    for (worker, peak) in peak_by_worker.into_iter().enumerate() {
        assert!(
            peak > 0.0001,
            "routing-tree worker {worker} produced no observable output"
        );
    }
}

pub(super) fn assert_global_mixer_state_matches(
    runtime: &SourceWorkerRuntime,
    actual: &SynthEngine,
    expected: &SynthEngine,
    frames: usize,
) {
    assert_eq!(actual.sample_clock(), expected.sample_clock());
    assert_momentary_state_matches(actual, expected);
    assert_eq!(actual.master_slot_params, expected.master_slot_params);
    assert_eq!(
        format!("{:?}", actual.master_slot_state),
        format!("{:?}", expected.master_slot_state)
    );
    assert_eq!(
        actual.master_active_slot_indices,
        expected.master_active_slot_indices
    );
    assert_eq!(
        actual.master_activity_frames,
        expected.master_activity_frames
    );
    assert_eq!(actual.routed_bus_slot_count, expected.routed_bus_slot_count);
    assert_eq!(actual.dry_history_pos, expected.dry_history_pos);
    let start = actual
        .dry_history_pos
        .wrapping_add(actual.dry_history.len())
        .wrapping_sub(frames * 2)
        % actual.dry_history.len();
    for frame in 0..frames {
        let index = (start + frame * 2) % actual.dry_history.len();
        let workers = runtime.routing_tree_worker_outputs_for_test(frame);
        assert_reassociated_close(
            actual.dry_history[index],
            expected.dry_history[index],
            workers,
            0,
            "routing-tree dry-history left frame",
        );
        assert_reassociated_close(
            actual.dry_history[index + 1],
            expected.dry_history[index + 1],
            workers,
            1,
            "routing-tree dry-history right frame",
        );
    }
}

fn verify_routing_tree_worker_name(parity: usize) -> Result<(), ()> {
    (thread::current().name() == Some(ROUTING_TREE_WORKER_THREAD_NAMES[parity]))
        .then_some(())
        .ok_or(())
}

#[test]
fn routing_tree_start_hook_runs_in_both_named_workers() {
    let mut engine = SynthEngine::new(44_100);
    let (lifecycle, runtime) = SourceWorkerLifecycle::start_routing_tree_prewarmed_with_hook(
        &mut engine,
        128,
        verify_routing_tree_worker_name,
    )
    .expect("routing-tree runtime");
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_pipeline_uses_block_lookahead_and_orders_overlap() {
    let (mut engine, lifecycle, mut runtime) = start_runtime();
    assert_eq!(runtime.lookahead_frames(), 128);
    let probe = Arc::new(RoutingTreePipelineProbe::default());
    runtime.set_routing_tree_probe_for_test(Arc::clone(&probe));

    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(probe.last_dispatch(), 1);
    assert_eq!(probe.last_coordinator(), 0);
    assert_eq!(probe.ordering_violations(), 0);
    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(probe.last_dispatch(), 2);
    assert_eq!(probe.last_coordinator(), 1);
    assert_eq!(probe.ordering_violations(), 0);

    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_pipeline_has_no_callback_allocations_after_prewarm() {
    let (mut engine, lifecycle, mut runtime) = start_runtime();
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.render_interleaved_block_with_source_runtime(
                &mut runtime,
                128,
                &mut left,
                &mut right,
                &mut out,
            )
        });
    assert_eq!((allocations, deallocations), (0, 0));
    shutdown(lifecycle, runtime);
}

#[cfg(feature = "source-worker-benchmark-timing")]
#[test]
fn routing_tree_pipeline_recovers_after_a_deadline_miss() {
    let (mut engine, lifecycle, mut runtime) = start_runtime();
    let probe = Arc::new(RoutingTreePipelineProbe::default());
    let timing_probe = Arc::new(SourceWorkerTimingProbe::new(None));
    runtime.set_routing_tree_probe_for_test(Arc::clone(&probe));
    runtime.attach_timing_probe(Arc::clone(&timing_probe));
    runtime.set_pause_for_parity_for_test(0, true);
    runtime.set_deadline_for_test(Duration::ZERO);
    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fresh
    );
    assert!(runtime.wait_until_paused_for_test(0, Duration::from_secs(1)));
    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::NewlyMissed
    );
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DeadlineMiss
    );
    let failed_sequence = timing_probe
        .snapshot()
        .coordinator
        .sequence
        .expect("failed routing timing sequence");

    runtime.set_pause_for_parity_for_test(0, false);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut disposition = SourceWorkerRenderDisposition::Recovering;
    let recovery_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while disposition != SourceWorkerRenderDisposition::Fresh {
        disposition = render_block(&mut engine, &mut runtime);
        assert!(std::time::Instant::now() < recovery_deadline);
        std::thread::yield_now();
    }
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    assert_eq!(probe.last_coordinator_base_sample_clock(), 256);
    assert_eq!(probe.last_dispatch_base_sample_clock(), 384);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_ne!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DispatchFailed
    );
    let recovered_timing = timing_probe.snapshot();
    assert_eq!(recovered_timing.coordinator.sequence, Some(failed_sequence));
    assert!(recovered_timing.coordinator.failed);
    assert_eq!(recovered_timing.coordinator.completed_mask, Some(0b11));
    assert!(recovered_timing
        .workers
        .iter()
        .all(|worker| worker.finished));
    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(
        timing_probe.snapshot().coordinator.sequence,
        Some(failed_sequence)
    );
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_worker_exit_latches_terminal_health() {
    let (mut engine, lifecycle, mut runtime) = start_runtime();
    lifecycle.set_panic_on_job_for_test(0);
    runtime.set_deadline_for_test(Duration::from_secs(1));

    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fatal
    );
    let health = runtime.health_snapshot();
    assert_eq!(health.status, SourceWorkerHealth::WorkerExited);
    assert_ne!(health.failed_mask & 1, 0);
    assert_eq!(health.worker_exits, 1);

    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_completion_disconnect_latches_terminal_health() {
    let (mut engine, lifecycle, mut runtime) = start_runtime();
    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fresh
    );
    runtime.disconnect_completion_for_test(0);

    assert_eq!(
        render_block(&mut engine, &mut runtime),
        SourceWorkerRenderDisposition::Fatal
    );
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );

    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_reasserts_bus_assignment_before_dispatch() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(bus_config());
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    let expected_worker = engine
        .routing_tree_assignment
        .as_ref()
        .and_then(|assignment| assignment.worker_for_bus(0))
        .expect("bus worker assignment");

    runtime.set_home_bus_assignment_for_test(0, 1 - expected_worker);
    let disposition = render_block(&mut engine, &mut runtime);
    assert_eq!(
        disposition,
        SourceWorkerRenderDisposition::Fresh,
        "{disposition:?} {:?}",
        runtime.health_snapshot()
    );
    assert!(runtime.collect_wait_for_test(&mut engine));
    assert_eq!(
        runtime.home_bus_carrier_assignments_for_test()[expected_worker][0],
        Some(Some(expected_worker))
    );

    shutdown(lifecycle, runtime);
}

fn bus_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "bus_1".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn bus_config_with_reverb() -> InstrumentsConfig {
    let mut config = bus_config();
    config.mixer.as_mut().expect("bus mixer").buses[0].slots =
        vec![FxBusSlotConfig::Kind("reverb".into())];
    config
}

#[path = "routing_tree_capacity_tests.rs"]
mod capacity;
