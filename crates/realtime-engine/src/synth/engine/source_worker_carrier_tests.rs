use super::source_worker_protocol::WorkerPhase;
use super::source_worker_test_fixtures::dynamic_engine;
use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::thread;

fn bus_config(bus_count: usize, kind: &str) -> InstrumentsConfig {
    bus_config_on(bus_count, kind, "B1")
}

fn bus_config_on(bus_count: usize, kind: &str, route: &str) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: route.into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: (0..bus_count)
                .map(|_| FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind(kind.into()); BUS_SLOTS_PER_BUS],
                    ..FxBusConfig::default()
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn wait_for_completion(runtime: &SourceWorkerRuntime, parity: usize) {
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(parity) {
            return;
        }
        thread::yield_now();
    }
    assert!(runtime.completion_ready_for_test(parity));
}

#[test]
fn persistent_startup_has_four_carriers_with_one_logical_id_each() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config(BUS_COUNT, "reverb"));
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");

    assert!(engine.bus_chains.is_empty());
    assert_eq!(
        runtime.home_bus_carrier_ids_for_test(),
        [
            [Some(0), None, Some(2), None],
            [None, Some(1), None, Some(3)]
        ]
    );
    assert_eq!(
        runtime.bus_carrier_scratch_shape_for_test(),
        Some((
            BLOCK_SLOT_SCRATCH_FRAMES,
            [BLOCK_SLOT_SCRATCH_FRAMES; BUS_SLOTS_PER_BUS],
            BLOCK_SLOT_SCRATCH_FRAMES,
            BLOCK_SLOT_SCRATCH_FRAMES,
        ))
    );
    assert_eq!(runtime.bus_carrier_scratch_bytes_for_test(), Some(49_152));

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn persistent_bus_render_is_inline_and_has_no_callback_allocation() {
    let config = bus_config(BUS_COUNT, "reverb");
    let mut worker = SynthEngine::new(48_000);
    let mut inline = SynthEngine::new(48_000);
    worker.set_instruments(config.clone());
    inline.set_instruments(config);
    worker.note_on(0, 60, 100, 5_000);
    inline.note_on(0, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut worker).expect("persistent runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut worker_left = Vec::with_capacity(128);
    let mut worker_right = Vec::with_capacity(128);
    let mut worker_out = Vec::with_capacity(256);
    let mut inline_left = Vec::with_capacity(128);
    let mut inline_right = Vec::with_capacity(128);
    let mut inline_out = Vec::with_capacity(256);
    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            worker.render_interleaved_block_with_source_runtime(
                &mut runtime,
                128,
                &mut worker_left,
                &mut worker_right,
                &mut worker_out,
            );
        });
    inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(worker_out, inline_out);
    assert!(worker.bus_chains.is_empty());

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn config_replacement_moves_owner_state_but_not_carrier_scratch() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config(BUS_COUNT, "delay"));
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let addresses = runtime.bus_carrier_scratch_addresses_for_test();

    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.set_instruments(bus_config(0, "none"));
        })
        .is_some());
    assert!(engine.bus_chains.is_empty());
    assert_eq!(runtime.bus_carrier_scratch_addresses_for_test(), addresses);

    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.set_instruments(bus_config(BUS_COUNT, "reverb"));
        })
        .is_some());
    assert!(engine.bus_chains.is_empty());
    assert_eq!(runtime.bus_carrier_scratch_addresses_for_test(), addresses);
    assert_eq!(engine.pending_render_retired.bus_chains.len(), BUS_COUNT);

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn active_carriers_follow_their_assigned_worker_and_parked_carriers_use_custodians() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config_on(BUS_COUNT, "reverb", "B2"));
    engine.note_on(0, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    engine.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );
    let locations = runtime.home_bus_carrier_assignments_for_test();
    for (logical_bus_id, (first, second)) in
        locations[0].iter().zip(locations[1].iter()).enumerate()
    {
        let expected = (*first).or(*second).expect("carrier location");
        let target = expected.unwrap_or(logical_bus_id % 2);
        assert!(locations[target][logical_bus_id].is_some());
    }
    assert_eq!(locations[0][0], Some(None));
    assert_eq!(locations[0][2], Some(None));
    assert_eq!(locations[1][3], Some(None));
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn setup_failure_restores_bus_owners_and_destroys_temporary_carriers() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config(BUS_COUNT, "reverb"));
    engine.block_slot_scratch.inline_source_executor = None;
    let error = match SourceWorkerLifecycle::start_prewarmed(&mut engine) {
        Ok(_) => panic!("missing source scratch must fail setup"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        SourceWorkerSetupError::InlineSourceExecutorUnavailable
    );
    assert_eq!(engine.bus_chains.len(), BUS_COUNT);
    assert!(engine.voice_pools_home());
    assert!(engine.block_slot_scratch.inline_source_executor.is_none());
}

#[test]
fn persistent_five_bus_setup_rejects_before_ownership_transfer() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config(BUS_COUNT + 1, "reverb"));
    let error = match SourceWorkerLifecycle::start_prewarmed(&mut engine) {
        Ok(_) => panic!("five buses must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        SourceWorkerSetupError::UnsupportedPersistentBusCount {
            requested: BUS_COUNT + 1,
            max: BUS_COUNT,
        }
    );
    assert_eq!(engine.bus_chains.len(), BUS_COUNT + 1);
    assert!(engine.voice_pools_home());
    assert!(engine.block_slot_scratch.inline_source_executor.is_some());

    engine.set_instruments(bus_config(8, "reverb"));
    assert_eq!(engine.bus_chains.len(), 8);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    engine.render_interleaved_block(32, &mut left, &mut right, &mut out);
    assert_eq!(out.len(), 64);
}

#[test]
fn panic_while_carriers_are_leased_returns_the_complete_pair_to_fault_escrow() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config(BUS_COUNT, "delay"));
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();
    let result = catch_unwind(AssertUnwindSafe(|| {
        runtime.with_controls_ready(&mut engine, |_| panic!("carrier lease panic"));
    }));
    assert!(result.is_err());
    assert!(engine.bus_chains.is_empty());
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );
    assert_eq!(
        lifecycle.fault_owner_identities_for_test(),
        [Some(initial[0]), Some(initial[1])]
    );

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn every_stamp_axis_and_phase_mismatch_is_faulted() {
    for axis in 0..9 {
        let mut engine = dynamic_engine();
        engine.note_on(1, 60, 100, 5_000);
        let (lifecycle, mut runtime) =
            SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
        assert!(runtime.dispatch_only_for_test(&mut engine, 128));
        wait_for_completion(&runtime, 0);
        let rewritten = match axis {
            0 => runtime.rewrite_completion_stamp_for_test(0, |stamp| {
                stamp.runtime_generation = stamp.runtime_generation.wrapping_add(1)
            }),
            1 => runtime.rewrite_completion_stamp_for_test(0, |stamp| {
                stamp.render_plan_generation = stamp.render_plan_generation.wrapping_add(1)
            }),
            2 => runtime.rewrite_completion_sequence_for_test(0),
            3 => runtime.rewrite_completion_stamp_for_test(0, |stamp| stamp.frames += 1),
            4 => runtime.rewrite_completion_stamp_for_test(0, |stamp| {
                stamp.base_sample_clock = stamp.base_sample_clock.wrapping_add(1)
            }),
            5 => runtime.rewrite_completion_owner_generation_for_test(0, 0),
            6 => runtime.rewrite_completion_phase_for_test(0),
            7 => runtime.rewrite_completion_render_ok_for_test(0, false),
            8 => runtime.rewrite_completion_owner_parity_for_test(0, 2),
            _ => unreachable!(),
        };
        assert!(rewritten);
        assert!(!runtime.collect_wait_for_test(&mut engine));
        assert_eq!(
            runtime.health_snapshot().status,
            SourceWorkerHealth::CompletionFailed
        );
        assert!(lifecycle.fault_owner_identities_for_test()[0].is_some());
        let retirement = runtime.retire();
        assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    }
}

#[test]
fn source_worker_executes_sources_with_full_stamp_and_owner() {
    let mut engine = dynamic_engine();
    engine.note_on(1, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();

    assert!(runtime.dispatch_only_for_test(&mut engine, 128));
    let stamp = runtime.expected_stamp_for_test().expect("source stamp");
    wait_for_completion(&runtime, 0);
    wait_for_completion(&runtime, 1);

    for (parity, initial_identity) in initial.iter().enumerate() {
        let evidence = runtime
            .completion_evidence_for_test(parity)
            .expect("source completion");
        assert_eq!(evidence.0, *initial_identity);
        assert_eq!(evidence.1, WorkerPhase::Sources);
        assert_eq!(evidence.2, stamp);
        assert!(evidence.3);
        assert!(!evidence.4);
        assert!(!evidence.5);
    }
    assert!(runtime.collect_wait_for_test(&mut engine));

    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn unsupported_bus_completion_preserves_owner_and_routes_to_fault_escrow() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_config(BUS_COUNT, "reverb"));
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();
    let stamp = runtime.stamp_for_test(&engine, 128);

    assert!(runtime.dispatch_buses_for_test(stamp, 0));
    wait_for_completion(&runtime, 0);
    let evidence = runtime
        .completion_evidence_for_test(0)
        .expect("unsupported bus completion");
    assert_eq!(evidence.0, initial[0]);
    assert_eq!(evidence.1, WorkerPhase::Buses);
    assert_eq!(evidence.2, stamp);
    assert!(!evidence.3);
    assert!(!evidence.4);
    assert!(!evidence.5);
    assert!(!runtime.workers_exited_for_test()[0]);
    assert_eq!(runtime.jobs_started_for_test(), [1, 0]);

    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );
    assert_eq!(runtime.in_flight_mask_for_test(), 0);
    assert_eq!(
        lifecycle.fault_owner_identities_for_test()[0],
        Some(initial[0])
    );
    assert_eq!(runtime.home_owner_identity_for_test(0), None);
    assert_eq!(runtime.home_owner_identity_for_test(1), Some(initial[1]));

    let retirement = runtime.retire();
    let shutdown = lifecycle.shutdown(retirement);
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
}
