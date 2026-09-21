use super::super::fx::{fx_bus_state_from_params, FxBusState, MasterFxState};
use super::super::fx_param::{FxParamId, FxParamMutation};

#[test]
fn scalar_mutation_preserves_render_and_state_identity_without_allocation() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(scalar_config());
    engine.bus_chains[0].assigned_worker = Some(1);
    engine.bus_chains[0].quiet_frames = 17;
    engine.bus_chains[0].render_hold_frames = 23;
    if let FxBusState::Delay { buf, idx, .. } = &mut engine.bus_chains[0].slot_state[0] {
        *idx = 3;
        buf[3] = 0.25;
    } else {
        panic!("prepared delay state");
    }
    if let FxBusState::Tremolo { phase } = &mut engine.bus_chains[0].slot_state[1] {
        *phase = 0.37;
    } else {
        panic!("prepared tremolo state");
    }
    if let MasterFxState::Compressor { env } = &mut engine.master_slot_state[0] {
        *env = 0.41;
    } else {
        panic!("prepared compressor state");
    }

    let plan = engine.render_plan.clone();
    let delay_ptr = match &engine.bus_chains[0].slot_state[0] {
        FxBusState::Delay { buf, .. } => buf.as_ptr(),
        _ => unreachable!(),
    };
    let phase = match engine.bus_chains[0].slot_state[1] {
        FxBusState::Tremolo { phase } => phase,
        _ => unreachable!(),
    };
    let envelope = match engine.master_slot_state[0] {
        MasterFxState::Compressor { env } => env,
        _ => unreachable!(),
    };
    let owner = (
        engine.bus_chains[0].quiet_frames,
        engine.bus_chains[0].render_hold_frames,
        engine.bus_chains[0].assigned_worker,
        engine.bus_chains[0].active_slot_count,
    );
    let ((), allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            for index in 0..1_000 {
                match index % 4 {
                    0 => assert_ne!(
                        engine.set_fx_bus_param(
                            0,
                            0,
                            FxParamId::Feedback,
                            (index % 100) as f32 / 100.0
                        ),
                        FxParamMutation::Rejected
                    ),
                    1 => assert_ne!(
                        engine.set_fx_bus_param(0, 1, FxParamId::RateHz, index as f32),
                        FxParamMutation::Rejected
                    ),
                    2 => assert_ne!(
                        engine.set_fx_bus_param(0, 2, FxParamId::MixPct, (index % 100) as f32),
                        FxParamMutation::Rejected
                    ),
                    _ => assert_ne!(
                        engine.set_global_fx_param(
                            0,
                            FxParamId::ThresholdDb,
                            -((index % 60) as f32),
                        ),
                        FxParamMutation::Rejected
                    ),
                }
            }
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(engine.render_plan, plan);
    assert_eq!(
        (
            engine.bus_chains[0].quiet_frames,
            engine.bus_chains[0].render_hold_frames,
            engine.bus_chains[0].assigned_worker,
            engine.bus_chains[0].active_slot_count,
        ),
        owner
    );
    assert!(engine.pending_render_retired_is_empty());
    assert_eq!(
        match &engine.bus_chains[0].slot_state[0] {
            FxBusState::Delay { buf, idx, .. } => (buf.as_ptr(), *idx),
            _ => unreachable!(),
        },
        (delay_ptr, 3)
    );
    assert_eq!(
        match engine.bus_chains[0].slot_state[1] {
            FxBusState::Tremolo { phase } => phase,
            _ => unreachable!(),
        },
        phase
    );
    assert_eq!(
        match engine.master_slot_state[0] {
            MasterFxState::Compressor { env } => env,
            _ => unreachable!(),
        },
        envelope
    );
    assert_eq!(delay_feedback(&engine.bus_chains[0].slot_params[0]), 0.96);
    assert_eq!(tremolo_rate(&engine.bus_chains[0].slot_params[1]), 40.0);
    assert_eq!(saturator_mix(&engine.bus_chains[0].slot_params[2]), 0.98);
    assert_eq!(compressor_threshold(&engine.master_slot_params[0]), -39.0);
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_scalar_control_returns_the_same_owner_pair() {
    use std::time::Duration;

    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(scalar_config());
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let initial = runtime.home_owner_identities_for_test();
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut output = vec![0.0; 256];
    let disposition = engine.render_interleaved_block_with_source_runtime_ready_with_controls(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut output,
        |engine| {
            assert_eq!(
                engine.set_fx_bus_param(0, 0, FxParamId::Feedback, 0.7),
                FxParamMutation::Changed
            );
            assert_eq!(
                engine.set_global_fx_param(0, FxParamId::ThresholdDb, -8.0),
                FxParamMutation::Changed
            );
            Ok(())
        },
    );
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    assert!(!engine.take_routing_tree_rejection());
    assert!(runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.home_owner_identities_for_test(), initial);
    let owners = runtime.take_home_owners_for_test().expect("owner pair");
    let feedback = owners
        .iter()
        .filter_map(|owner| owner.bus_carriers[0].as_ref())
        .filter_map(|carrier| carrier.owner.as_ref())
        .find_map(|owner| match owner.slot_params[0] {
            FxBusParams::Delay { feedback, .. } => Some(feedback),
            _ => None,
        });
    assert_eq!(feedback, Some(0.7));
    runtime.return_home_owners_for_test(owners);
    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
}

#[test]
fn prepared_delay_geometry_grows_and_declines_without_callback_resize() {
    let small = FxBusParams::Delay {
        time_ms: 10.0,
        feedback: 0.2,
        mix: 0.3,
        spread: 0.0,
    };
    let large = FxBusParams::Delay {
        time_ms: 40.0,
        feedback: 0.2,
        mix: 0.3,
        spread: 0.0,
    };
    assert_geometry_replacement(small, large, false);
    assert_geometry_replacement(large, small, true);

    let small = FxBusParams::ModDelay {
        rate_hz: 0.8,
        depth_ms: 4.0,
        base_ms: 8.0,
        feedback: 0.2,
        mix: 0.3,
    };
    let large = FxBusParams::ModDelay {
        rate_hz: 0.8,
        depth_ms: 20.0,
        base_ms: 30.0,
        feedback: 0.2,
        mix: 0.3,
    };
    assert_geometry_replacement(small, large, false);
    assert_geometry_replacement(large, small, true);
}

fn scalar_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![
                    FxBusSlotConfig::Kind("delay".into()),
                    FxBusSlotConfig::Kind("tremolo".into()),
                    FxBusSlotConfig::Kind("saturator".into()),
                ],
                ..FxBusConfig::default()
            }],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind("compressor".into())],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn assert_geometry_replacement(old: FxBusParams, next: FxBusParams, preserve_old: bool) {
    let mut owner = BusChainOwner::new(
        0,
        [old, FxBusParams::None, FxBusParams::None],
        [
            fx_bus_state_from_params(&old, 48_000),
            FxBusState::None,
            FxBusState::None,
        ],
        [1, 0, 0],
    );
    let old_ptr = state_buffer_ptr(&owner.slot_state[0]);
    let prepared = fx_bus_state_from_params(&next, 48_000);
    let prepared_ptr = state_buffer_ptr(&prepared);
    let ((current_ptr, retired_ptr, _retired), allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            let retired = owner
                .replace_slot(0, next, prepared, 1)
                .expect("valid slot");
            let current_ptr = state_buffer_ptr(&owner.slot_state[0]);
            let retired_ptr = state_buffer_ptr(&retired.state);
            for _ in 0..64 {
                let _ = owner.process(0.25, &[0.0; BUS_SLOTS_PER_BUS], 48_000);
            }
            (current_ptr, retired_ptr, retired)
        });
    if preserve_old {
        assert_eq!(current_ptr, old_ptr);
        assert_eq!(retired_ptr, prepared_ptr);
    } else {
        assert_eq!(current_ptr, prepared_ptr);
        assert_eq!(retired_ptr, old_ptr);
    }
    assert_eq!((allocations, deallocations), (0, 0));
}

fn state_buffer_ptr(state: &FxBusState) -> *const f32 {
    match state {
        FxBusState::Delay { buf, .. } | FxBusState::ModDelay { buf, .. } => buf.as_ptr(),
        _ => std::ptr::null(),
    }
}

fn tremolo_rate(params: &FxBusParams) -> f32 {
    let FxBusParams::Tremolo { rate_hz, .. } = params else {
        unreachable!()
    };
    *rate_hz
}

fn delay_feedback(params: &FxBusParams) -> f32 {
    let FxBusParams::Delay { feedback, .. } = params else {
        unreachable!()
    };
    *feedback
}

fn saturator_mix(params: &FxBusParams) -> f32 {
    let FxBusParams::Saturator { mix, .. } = params else {
        unreachable!()
    };
    *mix
}

fn compressor_threshold(params: &FxBusParams) -> f32 {
    let FxBusParams::Compressor { threshold_db, .. } = params else {
        unreachable!()
    };
    *threshold_db
}

mod fx_param_tests {
    include!("fx_param_tests.rs");
}

mod fx_param_contract_tests {
    include!("fx_param_contract_tests.rs");
}
