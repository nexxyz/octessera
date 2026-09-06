use super::super::dsp_config::{BusIdleThreshold, DspRuntimeConfig};
use super::super::fx::{fx_bus_state_from_params, process_fx_bus_slot, FxBusState};
use super::super::fx_params::{DuckSource, FilterLfoKind, FxBusParams};
use super::super::types::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, BUS_SLOTS_PER_BUS, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use super::*;

#[test]
fn every_persistent_fx_owner_path_is_bit_exact_to_the_slot_kernel() {
    let params = [
        FxBusParams::None,
        FxBusParams::Tremolo {
            rate_hz: 2.0,
            depth: 0.5,
        },
        FxBusParams::Delay {
            time_ms: 20.0,
            feedback: 0.2,
            mix: 0.3,
            spread: 0.7,
        },
        FxBusParams::ModDelay {
            rate_hz: 0.5,
            depth_ms: 3.0,
            base_ms: 8.0,
            feedback: 0.0,
            mix: 0.2,
        },
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::FilterLfo,
            rate_hz: 0.5,
            depth: 0.7,
            center_hz: 1_000.0,
            q: 0.8,
        },
        FxBusParams::Reverb {
            mix: 0.4,
            decay: 0.8,
            damp: 0.2,
        },
        FxBusParams::Glitch {
            chance: 1.0,
            slice_ms: 10.0,
            mix: 0.6,
        },
        FxBusParams::AutoPan {
            rate_hz: 0.5,
            depth: 1.0,
        },
        FxBusParams::Duck {
            source: DuckSource::Bus(0),
            threshold: 0.1,
            amount: 0.6,
            attack_ms: 5.0,
            release_ms: 20.0,
        },
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Bitcrusher {
            rate_div: 4,
            bits: 6,
            mix: 0.5,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];
    let slot_out = [0.75; INSTRUMENT_SLOT_COUNT];
    let bus_snapshot = [0.5, 0.25];

    for param in params {
        let mut legacy_state = fx_bus_state_from_params(&param, 48_000);
        let mut owner = BusChainOwner::new(
            0,
            [param, FxBusParams::None, FxBusParams::None],
            [
                fx_bus_state_from_params(&param, 48_000),
                FxBusState::None,
                FxBusState::None,
            ],
            [1, 0, 0],
        );
        for input in [0.5, 0.25] {
            let expected = process_fx_bus_slot(
                &param,
                &mut legacy_state,
                input,
                &slot_out,
                &bus_snapshot,
                48_000,
            );
            let expected_auto_pan = match &legacy_state {
                FxBusState::AutoPan { pos, .. } => Some(pos.to_bits()),
                _ => None,
            };
            let expected_spread = match param {
                FxBusParams::Delay { mix, spread, .. } => spread * mix,
                _ => 0.0,
            };
            let actual = owner.process(input, &slot_out, &bus_snapshot, 48_000);
            assert_eq!(actual.mono.to_bits(), expected.to_bits());
            assert_eq!(actual.auto_pan_pos.map(f32::to_bits), expected_auto_pan);
            assert_eq!(actual.spread.to_bits(), expected_spread.to_bits());
        }
    }
}

#[test]
fn four_buses_with_three_slots_match_inline_reference_at_supported_quanta() {
    for frames in [64, 128, 256] {
        let config = four_bus_config();
        let mut block = SynthEngine::new(44_100);
        let mut reference = SynthEngine::new(44_100);
        block.set_instruments(config.clone());
        reference.set_instruments(config);
        assert_eq!(block.bus_chains.len(), 4);
        assert_eq!(block.profile_snapshot().active_bus_fx_slots, 12);
        for slot in 0..4 {
            block.note_on(slot, 36 + slot, 100, 5_000);
            reference.note_on(slot, 36 + slot, 100, 5_000);
        }
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut out = Vec::new();
        block.render_interleaved_block(frames, &mut left, &mut right, &mut out);
        for [left, right] in out.as_chunks::<2>().0 {
            let (expected_left, expected_right) = reference.next_stereo_sample();
            assert_eq!(left.to_bits(), expected_left.to_bits());
            assert_eq!(right.to_bits(), expected_right.to_bits());
        }
    }
}

#[test]
fn profiler_keeps_dynamic_bus_capacity_above_generated_product_shape() {
    let mut config = four_bus_config();
    config
        .mixer
        .as_mut()
        .expect("mixer")
        .buses
        .push(FxBusConfig {
            slots: vec![FxBusSlotConfig::Kind("reverb".into()); BUS_SLOTS_PER_BUS],
            ..FxBusConfig::default()
        });
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(config);
    assert_eq!(engine.bus_chains.len(), 5);
    assert_eq!(engine.profile_snapshot().active_bus_fx_slots, 15);
}

#[test]
fn duck_reads_pre_chain_inputs_independently_of_bus_processing_order() {
    let params = FxBusParams::Duck {
        source: DuckSource::Bus(1),
        threshold: 0.1,
        amount: 0.8,
        attack_ms: 1.0,
        release_ms: 10.0,
    };
    let slot_out = [0.8; INSTRUMENT_SLOT_COUNT];
    let snapshot = [0.4, 0.8];
    let mut first = BusChainOwner::new(
        0,
        [params, FxBusParams::None, FxBusParams::None],
        [
            fx_bus_state_from_params(&params, 48_000),
            FxBusState::None,
            FxBusState::None,
        ],
        [1, 0, 0],
    );
    let mut second = BusChainOwner::new(
        1,
        [params, FxBusParams::None, FxBusParams::None],
        [
            fx_bus_state_from_params(&params, 48_000),
            FxBusState::None,
            FxBusState::None,
        ],
        [1, 0, 0],
    );
    let forward = [
        first
            .process(snapshot[0], &slot_out, &snapshot, 48_000)
            .mono,
        second
            .process(snapshot[1], &slot_out, &snapshot, 48_000)
            .mono,
    ];
    let reverse = [
        second
            .process(snapshot[1], &slot_out, &snapshot, 48_000)
            .mono,
        first
            .process(snapshot[0], &slot_out, &snapshot, 48_000)
            .mono,
    ];
    assert_eq!(forward[0].to_bits(), reverse[1].to_bits());
    assert_eq!(forward[1].to_bits(), reverse[0].to_bits());
}

#[test]
fn projected_assignment_is_sticky_and_ties_choose_worker_zero() {
    let mut engine = SynthEngine::new(48_000);
    engine.source_worker_load = Some(load([0, 0], [1, 1]));
    engine.bus_chains = vec![test_owner(2), test_owner(2)];
    assert_eq!(engine.choose_bus_worker(2), Some(0));
    engine.bus_chains[0].assigned_worker = Some(0);
    assert_eq!(engine.choose_bus_worker(2), Some(1));

    engine.observe_bus_chain(1, 1.0, 1.0);
    assert_eq!(engine.bus_chains[1].assigned_worker, Some(1));
    engine.source_worker_load = Some(load([0, 0], [100, 1]));
    engine.observe_bus_chain(1, 1.0, 1.0);
    assert_eq!(engine.bus_chains[1].assigned_worker, Some(1));
}

#[test]
fn inactivity_boundaries_and_signal_rules_are_exact_at_both_rates() {
    for sample_rate in [44_100, 48_000] {
        let required = (u64::from(sample_rate) * 250 / 1000) as usize;
        let mut owner = test_owner(1);
        owner.assigned_worker = Some(1);
        for frame in 0..required {
            owner.observe(0.0, 0.0, BusIdleThreshold::Exact, sample_rate);
            if frame + 1 < required {
                assert_eq!(owner.assigned_worker, Some(1));
            }
        }
        assert_eq!(owner.quiet_frames as usize, required);
        assert_eq!(owner.assigned_worker, None);

        owner.assigned_worker = Some(1);
        owner.observe(f32::EPSILON, 0.0, BusIdleThreshold::Exact, sample_rate);
        assert_eq!(owner.quiet_frames, 0);
        owner.observe(0.0, 0.000001, BusIdleThreshold::Db120, sample_rate);
        assert_eq!(owner.quiet_frames, 1);
        owner.observe(0.0, f32::NAN, BusIdleThreshold::Db120, sample_rate);
        assert_eq!(owner.quiet_frames, 0);
        owner.observe(f32::INFINITY, 0.0, BusIdleThreshold::Db120, sample_rate);
        assert_eq!(owner.quiet_frames, 0);
        owner.observe(0.0, 0.0000011, BusIdleThreshold::Db120, sample_rate);
        assert_eq!(owner.quiet_frames, 0);
    }
}

#[test]
fn slot_and_threshold_changes_reset_only_quiet_observation() {
    let mut owner = test_owner(1);
    owner.assigned_worker = Some(1);
    owner.quiet_frames = 12;
    let params = FxBusParams::Delay {
        time_ms: 2.0,
        feedback: 0.2,
        mix: 0.5,
        spread: 0.0,
    };
    let state = fx_bus_state_from_params(&params, 48_000);
    owner.replace_slot(0, params, state, 2);
    assert_eq!(owner.quiet_frames, 0);
    assert_eq!(owner.assigned_worker, Some(1));

    let mut engine = SynthEngine::new(48_000);
    engine.bus_chains = vec![owner];
    engine.bus_chains[0].quiet_frames = 9;
    engine.set_dsp_config(DspRuntimeConfig {
        worker_warning_threshold: super::super::dsp_config::WorkerWarningThreshold::Percent85,
        bus_idle_threshold: BusIdleThreshold::Exact,
    });
    assert_eq!(engine.bus_chains[0].quiet_frames, 0);
    assert_eq!(engine.bus_chains[0].assigned_worker, Some(1));
}

#[test]
fn parking_does_not_advance_or_reset_state_for_delay_reverb_glitch_or_vinyl() {
    let params = [
        FxBusParams::Delay {
            time_ms: 2.0,
            feedback: 0.7,
            mix: 0.6,
            spread: 0.0,
        },
        FxBusParams::Reverb {
            mix: 0.5,
            decay: 0.8,
            damp: 0.2,
        },
        FxBusParams::Glitch {
            chance: 1.0,
            slice_ms: 8.0,
            mix: 0.8,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.1,
            mix: 0.8,
        },
    ];
    let slot_out = [0.2; INSTRUMENT_SLOT_COUNT];
    let snapshot = [0.2];
    for param in params {
        let state = fx_bus_state_from_params(&param, 48_000);
        let mut parked = BusChainOwner::new(
            0,
            [param, FxBusParams::None, FxBusParams::None],
            [state.clone(), FxBusState::None, FxBusState::None],
            [1, 0, 0],
        );
        let mut active = BusChainOwner::new(
            0,
            [param, FxBusParams::None, FxBusParams::None],
            [state, FxBusState::None, FxBusState::None],
            [1, 0, 0],
        );
        parked.assigned_worker = Some(1);
        let _ = parked.process(0.8, &slot_out, &snapshot, 48_000);
        let _ = active.process(0.8, &slot_out, &snapshot, 48_000);
        for _ in 0..(48_000 * 250 / 1000) {
            parked.observe(0.0, 0.0, BusIdleThreshold::Exact, 48_000);
        }
        assert_eq!(parked.assigned_worker, None);
        let parked_output = parked.process(0.2, &slot_out, &snapshot, 48_000).mono;
        let active_output = active.process(0.2, &slot_out, &snapshot, 48_000).mono;
        assert_eq!(parked_output.to_bits(), active_output.to_bits());
    }
}

#[test]
fn prepared_full_and_single_replacements_retire_owner_state_without_callback_drops() {
    let config = four_bus_config();
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(config.clone());
    engine.bus_chains[0].assigned_worker = Some(1);
    engine.bus_chains[0].quiet_frames = 7;
    let full_retired = engine
        .apply_prepared_instruments_config(prepare_instruments_config(config.clone(), 44_100));
    assert_eq!(full_retired.bus_chains.len(), 4);
    assert_eq!(engine.bus_chains[0].assigned_worker, Some(1));
    assert_eq!(engine.bus_chains[0].quiet_frames, 7);
    drop(full_retired);

    let prepared = prepare_fx_bus_slot("reverb".into(), Default::default(), 44_100);
    let (retired, _, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.apply_prepared_fx_bus_slot(0, 0, prepared)
        });
    assert_eq!(retired.bus_chains.len(), 1);
    assert_eq!(deallocations, 0);
    drop(retired);
}

fn test_owner(cost: u16) -> BusChainOwner {
    BusChainOwner::new(
        0,
        [
            FxBusParams::Saturator {
                drive: 2.0,
                mix: 1.0,
            },
            FxBusParams::None,
            FxBusParams::None,
        ],
        [FxBusState::None, FxBusState::None, FxBusState::None],
        [cost, 0, 0],
    )
}

fn load(busy_ns_ewma: [u64; 2], ns_per_unit_ewma: [u64; 2]) -> SourceWorkerLoadSnapshot {
    SourceWorkerLoadSnapshot {
        quantum_ns: 1_000_000,
        ewma_coefficient_ppm: 1_000_000,
        busy_ns_ewma,
        ns_per_unit_ewma,
        observed_active_cost_units: [0, 0],
        has_useful_measurement: [true, true],
        utilization_ppm: None,
        observed: [true, true],
    }
}

fn four_bus_config() -> InstrumentsConfig {
    let slot_kinds = ["delay", "duck", "reverb"];
    InstrumentsConfig {
        instruments: (0..4)
            .map(|slot| InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: format!("fx_bus_{}", slot + 1),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: (0..4)
                .map(|_| FxBusConfig {
                    slots: slot_kinds
                        .iter()
                        .map(|kind| FxBusSlotConfig::Kind((*kind).into()))
                        .collect(),
                    ..FxBusConfig::default()
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
