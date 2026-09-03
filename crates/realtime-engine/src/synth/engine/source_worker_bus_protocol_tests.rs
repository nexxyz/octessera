use super::super::source_worker_bus::render_bus_block;
use super::super::source_worker_test_fixtures::dynamic_engine;
use super::super::*;
use super::{configured_fx, install_momentary, one_bus, slot_out};
use std::collections::BTreeMap;
use std::thread;

#[test]
fn worker_bus_command_returns_successful_stamped_completion_and_cost() {
    let config = one_bus(vec![
        configured_fx("reverb"),
        configured_fx("reverb"),
        configured_fx("reverb"),
    ]);
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(config);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();
    let slot_out = slot_out(128, 0.5);
    assert!(runtime.stage_and_dispatch_buses_for_test(&mut engine, &slot_out, 128, 0));
    let stamp = runtime.expected_stamp_for_test().expect("bus stamp");
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(0) {
            break;
        }
        thread::yield_now();
    }
    let evidence = runtime
        .completion_evidence_for_test(0)
        .expect("bus completion");
    assert_eq!(evidence.0, initial[0]);
    assert_eq!(evidence.1, WorkerPhase::Buses);
    assert_eq!(evidence.2, stamp);
    assert!(evidence.3);
    assert!(!evidence.4);
    assert!(!evidence.5);
    assert!(evidence.6 > 0);
    assert_eq!(evidence.7, 9);
    assert!(!runtime.workers_exited_for_test()[0]);

    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
}

#[test]
fn zero_bus_worker_completion_is_successful_with_zero_cost() {
    let mut engine = dynamic_engine();
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let stamp = runtime.stamp_for_test(&engine, 128);
    assert!(runtime.dispatch_buses_for_test(stamp, 0));
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(0) {
            break;
        }
        thread::yield_now();
    }
    let evidence = runtime
        .completion_evidence_for_test(0)
        .expect("zero bus completion");
    assert_eq!(evidence.1, WorkerPhase::Buses);
    assert_eq!(evidence.2, stamp);
    assert!(evidence.3);
    assert!(!evidence.4);
    assert_eq!(evidence.7, 0);
    assert!(!runtime.workers_exited_for_test()[0]);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn invalid_bus_owner_is_rejected_without_owner_loss() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(one_bus(vec![
        configured_fx("reverb"),
        configured_fx("none"),
        configured_fx("none"),
    ]));
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    let carrier = owners[0]
        .bus_carriers
        .iter_mut()
        .flatten()
        .find(|carrier| carrier.logical_bus_id == 0)
        .expect("bus zero carrier");
    carrier.logical_bus_id = BUS_COUNT;
    let stamp = runtime.stamp_for_test(&engine, 128);
    assert!(render_bus_block(
        &mut owners[0],
        0,
        stamp,
        128,
        engine.sample_rate,
        engine.dsp_config.bus_idle_threshold,
        engine.fx_activity_hold_frames,
    )
    .is_err());
    runtime.return_home_owners_for_test(owners);
    assert_eq!(runtime.home_owner_identities_for_test(), initial);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn bus_worker_panic_preserves_owner_for_terminal_cleanup() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(one_bus(vec![
        configured_fx("reverb"),
        configured_fx("none"),
        configured_fx("none"),
    ]));
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();
    lifecycle.set_panic_on_job_for_test(0);
    let stamp = runtime.stamp_for_test(&engine, 128);
    assert!(runtime.dispatch_buses_for_test(stamp, 0));
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(0) {
            break;
        }
        thread::yield_now();
    }
    let evidence = runtime
        .completion_evidence_for_test(0)
        .expect("panic completion");
    assert_eq!(evidence.0, initial[0]);
    assert_eq!(evidence.1, WorkerPhase::Buses);
    assert_eq!(evidence.2, stamp);
    assert!(!evidence.3);
    assert!(evidence.4);
    assert!(!evidence.5);
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.home_owner_identity_for_test(0), None);
    assert_eq!(
        lifecycle.fault_owner_identities_for_test()[0],
        Some(initial[0])
    );
    assert_eq!(
        lifecycle.shutdown(runtime.retire()).destroyed_owner_count,
        2
    );
}

#[test]
fn block_liveness_updates_quiet_and_render_hold_without_advancing_parked_fx() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(one_bus(vec![
        configured_fx("delay"),
        configured_fx("none"),
        configured_fx("none"),
    ]));
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    owners[0]
        .bus_carriers
        .iter_mut()
        .flatten()
        .find(|carrier| carrier.logical_bus_id == 0)
        .and_then(|carrier| carrier.owner.as_mut())
        .expect("bus owner")
        .assigned_worker = Some(0);
    let silent = slot_out(128, 0.0);
    let required_blocks = (48_000usize * 250 / 1000).div_ceil(128);
    for _ in 0..required_blocks {
        assert!(super::super::source_worker_bus::stage_bus_block(
            &mut engine,
            &mut owners,
            &silent,
            128,
        ));
        let stamp = runtime.stamp_for_test(&engine, 128);
        for owner in &mut owners {
            assert!(render_bus_block(
                owner,
                owner.parity,
                stamp,
                128,
                engine.sample_rate,
                engine.dsp_config.bus_idle_threshold,
                engine.fx_activity_hold_frames,
            )
            .is_ok());
        }
    }
    let chain = owners[0]
        .bus_carriers
        .iter()
        .flatten()
        .find(|carrier| carrier.logical_bus_id == 0)
        .and_then(|carrier| carrier.owner.as_ref())
        .expect("bus owner");
    assert_eq!(chain.assigned_worker, None);
    let active = slot_out(128, 0.5);
    assert!(super::super::source_worker_bus::stage_bus_block(
        &mut engine,
        &mut owners,
        &active,
        128,
    ));
    let stamp = runtime.stamp_for_test(&engine, 128);
    for owner in &mut owners {
        assert!(render_bus_block(
            owner,
            owner.parity,
            stamp,
            128,
            engine.sample_rate,
            engine.dsp_config.bus_idle_threshold,
            engine.fx_activity_hold_frames,
        )
        .is_ok());
    }
    let hold = owners[0]
        .bus_carriers
        .iter()
        .flatten()
        .find(|carrier| carrier.logical_bus_id == 0)
        .and_then(|carrier| carrier.owner.as_ref())
        .expect("bus owner")
        .render_hold_frames;
    assert!(hold > 0);
    assert!(super::super::source_worker_bus::stage_bus_block(
        &mut engine,
        &mut owners,
        &silent,
        128,
    ));
    let stamp = runtime.stamp_for_test(&engine, 128);
    for owner in &mut owners {
        assert!(render_bus_block(
            owner,
            owner.parity,
            stamp,
            128,
            engine.sample_rate,
            engine.dsp_config.bus_idle_threshold,
            engine.fx_activity_hold_frames,
        )
        .is_ok());
    }
    let next_hold = owners[0]
        .bus_carriers
        .iter()
        .flatten()
        .find(|carrier| carrier.logical_bus_id == 0)
        .and_then(|carrier| carrier.owner.as_ref())
        .expect("bus owner")
        .render_hold_frames;
    assert!(next_hold < hold);
    runtime.return_home_owners_for_test(owners);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn momentary_retirement_is_scoped_to_the_processed_target() {
    let mut engine = SynthEngine::new(48_000);
    install_momentary(
        &mut engine,
        "instrument",
        "stutter",
        MomentaryFxTarget::Instrument { index: 0 },
        BTreeMap::new(),
    );
    install_momentary(
        &mut engine,
        "bus",
        "freeze",
        MomentaryFxTarget::FxBus { index: 0 },
        BTreeMap::new(),
    );
    for fx in &mut engine.momentary_fx {
        fx.releasing = true;
        match fx.kind {
            MomentaryFxKind::Freeze => fx.release_pos = fx.release_len,
            MomentaryFxKind::FilterSweep => fx.sweep_pos = 0.0,
            _ => {}
        }
    }
    let _ =
        engine.process_momentary_fx_target(MomentaryFxTarget::Instrument { index: 0 }, 0.0, 0.0);
    assert_eq!(engine.momentary_fx.len(), 1);
    assert!(engine
        .momentary_fx
        .iter()
        .any(|fx| fx.target == MomentaryFxTarget::FxBus { index: 0 }));
    for retired in &mut engine.pending_render_retired.displaced_momentary_fx {
        *retired = None;
    }
    let _ = engine.process_momentary_fx_target(MomentaryFxTarget::FxBus { index: 0 }, 0.0, 0.0);
    assert!(engine.momentary_fx.is_empty());
    for retired in &mut engine.pending_render_retired.displaced_momentary_fx {
        *retired = None;
    }
    install_momentary(
        &mut engine,
        "global-again",
        "filter_sweep",
        MomentaryFxTarget::Global,
        BTreeMap::new(),
    );
    let global = engine.momentary_fx.first_mut().expect("global FX");
    global.releasing = true;
    global.sweep_pos = 0.0;
    let _ = engine.process_momentary_fx_target(MomentaryFxTarget::Global, 0.0, 0.0);
    assert!(engine.momentary_fx.is_empty());
}
