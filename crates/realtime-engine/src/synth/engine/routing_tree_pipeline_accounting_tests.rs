use self::spread_state::{
    assert_spread_state_is_nontrivial, assert_spread_state_matches,
    bus_output_spread_state_signature, recovered_source_state_signature,
};
use super::super::routing_tree_executor_state_tests::engine_source_state_signature;
use super::*;
use crate::synth::engine::routing_tree_executor_test_support::bus_chain_state;
use crate::synth::{
    MomentaryFxTarget, SampleBankConfig, SampleBuffer, SampleSlotConfig, VoiceStealingMode,
};
use serde_json::json;
use std::collections::BTreeMap;

#[path = "routing_tree_spread_state_test_support.rs"]
mod spread_state;

fn observed_routing_cost(active: bool) -> [u16; 2] {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(bus_config_with_reverb());
    if active {
        engine.note_on(0, 60, 100, 500);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let disposition = render_block(&mut engine, &mut runtime);
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    let observed = runtime.load_snapshot().expect("routing worker load");
    let result = observed.observed_active_cost_units;
    shutdown(lifecycle, runtime);
    result
}

#[test]
fn routing_tree_actual_threaded_active_bus_cost_is_counted_once() {
    assert_eq!(
        observed_routing_cost(true),
        [
            SOURCE_WORKER_SYNTH_COST_UNITS + BUS_CHAIN_SLOT_COST_UNITS - 1,
            0
        ]
    );
}

#[test]
fn routing_tree_actual_threaded_quiet_bus_cost_is_zero() {
    assert_eq!(observed_routing_cost(false), [0, 0]);
}

#[test]
fn routing_tree_reverse_completion_preserves_four_bus_output_parity() {
    let mut normal = SynthEngine::new(44_100);
    let mut forced_reverse = SynthEngine::new(44_100);
    let mut inline = SynthEngine::new(44_100);
    for engine in [&mut normal, &mut forced_reverse, &mut inline] {
        engine.set_instruments(duplicated_analogue_config());
        drop(engine.set_sample_banks(duplicated_analogue_sample_banks()));
        apply_initial_bus_spread_sources(engine);
        engine.momentary_fx_start(
            "global".into(),
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
        );
    }
    normal.set_voice_stealing_mode(VoiceStealingMode::None);
    forced_reverse.set_voice_stealing_mode(VoiceStealingMode::None);
    inline.set_voice_stealing_mode(VoiceStealingMode::None);
    let (normal_lifecycle, mut normal_runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut normal, 128)
            .expect("normal routing-tree runtime");
    let (reverse_lifecycle, mut reverse_runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut forced_reverse, 128)
            .expect("routing-tree runtime");
    reverse_lifecycle.set_reverse_completion_for_test(true);
    normal_runtime.set_deadline_for_test(Duration::from_secs(1));
    reverse_runtime.set_deadline_for_test(Duration::from_secs(1));
    for block in 0..4 {
        let pending_source_state = if block > 0 {
            assert!(normal_runtime.collect_wait_for_test(&mut normal));
            assert!(reverse_runtime.collect_wait_for_test(&mut forced_reverse));
            assert_eq!(
                reverse_lifecycle.take_reverse_completion_order_for_test(),
                vec![1, 0],
                "reverse routing-tree completion order"
            );
            let normal_state = recovered_source_state_signature(&mut normal_runtime, &mut normal);
            let reverse_state =
                recovered_source_state_signature(&mut reverse_runtime, &mut forced_reverse);
            assert_eq!(
                normal_state.0, reverse_state.0,
                "normal/reverse logical state at block {block}"
            );
            assert_eq!(
                normal_state.1, reverse_state.1,
                "normal/reverse bus worker state at block {block}"
            );
            assert_spread_state_matches(
                &normal_state.2,
                &reverse_state.2,
                0,
                "normal/reverse spread state",
            );
            assert_spread_state_is_nontrivial(&normal_state.2, "normal recovered");
            Some(normal_state)
        } else {
            None
        };
        let mut normal_left = vec![0.0; 128];
        let mut normal_right = vec![0.0; 128];
        let mut normal_out = vec![0.0; 256];
        let mut reverse_left = vec![0.0; 128];
        let mut reverse_right = vec![0.0; 128];
        let mut reverse_out = vec![0.0; 256];
        let mut inline_left = vec![0.0; 128];
        let mut inline_right = vec![0.0; 128];
        let mut inline_out = vec![0.0; 256];
        let normal_disposition = if block == 0 {
            normal.render_interleaved_block_with_source_runtime_ready_with_controls(
                &mut normal_runtime,
                128,
                &mut normal_left,
                &mut normal_right,
                &mut normal_out,
                |engine| {
                    apply_duplicated_analogue_sources(engine);
                    Ok(())
                },
            )
        } else {
            normal.render_interleaved_block_with_source_runtime(
                &mut normal_runtime,
                128,
                &mut normal_left,
                &mut normal_right,
                &mut normal_out,
            )
        };
        assert_eq!(normal_disposition, SourceWorkerRenderDisposition::Fresh);
        let reverse_disposition = if block == 0 {
            forced_reverse.render_interleaved_block_with_source_runtime_ready_with_controls(
                &mut reverse_runtime,
                128,
                &mut reverse_left,
                &mut reverse_right,
                &mut reverse_out,
                |engine| {
                    apply_duplicated_analogue_sources(engine);
                    Ok(())
                },
            )
        } else {
            forced_reverse.render_interleaved_block_with_source_runtime(
                &mut reverse_runtime,
                128,
                &mut reverse_left,
                &mut reverse_right,
                &mut reverse_out,
            )
        };
        assert_eq!(reverse_disposition, SourceWorkerRenderDisposition::Fresh);
        inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
        assert_eq!(normal_out, reverse_out);
        assert_interleaved_reassociated_close(&normal_runtime, &normal_out, &inline_out);
        assert_global_mixer_state_matches(&normal_runtime, &normal, &forced_reverse, 128);
        assert_global_mixer_state_matches(&normal_runtime, &normal, &inline, 128);
        assert_eq!(normal.profile_snapshot(), forced_reverse.profile_snapshot());
        assert_eq!(normal.profile_snapshot(), inline.profile_snapshot());
        if block == 0 {
            apply_duplicated_analogue_sources(&mut inline);
        } else {
            assert_worker_outputs_are_nonzero(&normal_runtime, 128);
            let observed = normal_runtime.load_snapshot().expect("normal worker load");
            assert!(observed
                .observed_active_cost_units
                .iter()
                .all(|cost| *cost > 0));
        }
        if let Some(normal_state) = pending_source_state {
            assert_eq!(normal_state.0, engine_source_state_signature(&inline));
            assert_eq!(normal_state.1, bus_chain_state(&inline));
            assert_spread_state_matches(
                &normal_state.2,
                &bus_output_spread_state_signature(&inline),
                32,
                "routing/canonical spread state",
            );
        }
    }
    assert!(reverse_runtime.collect_wait_for_test(&mut forced_reverse));
    assert_eq!(
        reverse_lifecycle.take_reverse_completion_order_for_test(),
        vec![1, 0],
        "reverse routing-tree completion order"
    );
    let final_normal_state = recovered_source_state_signature(&mut normal_runtime, &mut normal);
    let final_reverse_state =
        recovered_source_state_signature(&mut reverse_runtime, &mut forced_reverse);
    assert_eq!(final_normal_state.0, final_reverse_state.0);
    assert_eq!(final_normal_state.1, final_reverse_state.1);
    assert_spread_state_matches(
        &final_normal_state.2,
        &final_reverse_state.2,
        0,
        "final normal/reverse spread state",
    );
    assert_spread_state_is_nontrivial(&final_normal_state.2, "final normal recovered");
    let mut final_inline_left = vec![0.0; 128];
    let mut final_inline_right = vec![0.0; 128];
    let mut final_inline_out = vec![0.0; 256];
    inline.render_interleaved_block(
        128,
        &mut final_inline_left,
        &mut final_inline_right,
        &mut final_inline_out,
    );
    assert_eq!(final_normal_state.0, engine_source_state_signature(&inline));
    assert_eq!(final_normal_state.1, bus_chain_state(&inline));
    assert_spread_state_matches(
        &final_normal_state.2,
        &bus_output_spread_state_signature(&inline),
        32,
        "final routing/canonical spread state",
    );
    assert_eq!(
        normal_runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(
        reverse_runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(
        normal_lifecycle
            .shutdown(normal_runtime.retire())
            .joined_workers,
        2
    );
    assert_eq!(
        reverse_lifecycle
            .shutdown(reverse_runtime.retire())
            .joined_workers,
        2
    );
}

fn apply_duplicated_analogue_sources(engine: &mut SynthEngine) {
    for (slot, note) in [(0, 36), (1, 36), (2, 48), (4, 67), (5, 36), (6, 72)] {
        engine.note_on(slot, note, 100, 5_000);
    }
}

fn apply_initial_bus_spread_sources(engine: &mut SynthEngine) {
    for (slot, note) in [(3, 60), (7, 84)] {
        engine.note_on(slot, note, 100, 5_000);
    }
}

fn duplicated_analogue_sample_banks() -> Vec<SampleBankConfig> {
    let mut banks = vec![SampleBankConfig::default(); 8];
    for slot in [1, 5] {
        banks[slot].slots[0] = SampleSlotConfig {
            buffer: Some(SampleBuffer {
                samples: vec![0.25; 128].into(),
                channels: 1,
                sample_rate: 44_100,
            }),
        };
    }
    banks
}

fn duplicated_analogue_config() -> InstrumentsConfig {
    let ducks = ["I2", "I1", "I6", "I5"].map(|source| FxBusSlotConfig::Config {
        kind: "duck".into(),
        params: BTreeMap::from([
            ("source".into(), json!(source)),
            ("amountPct".into(), json!(80.0)),
        ]),
    });
    let kinds = [
        "synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth",
    ];
    let routes = [
        "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "direct", "fx_bus_3", "fx_bus_4",
    ];
    InstrumentsConfig {
        instruments: kinds
            .into_iter()
            .zip(routes)
            .map(|(kind, route)| InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: route.into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: ducks
                .into_iter()
                .enumerate()
                .map(|(bus, duck)| FxBusConfig {
                    slots: if bus % 2 == 1 {
                        vec![duck, spread_delay_slot()]
                    } else {
                        vec![duck]
                    },
                    pan_pos: [
                        DEFAULT_PAN_POSITIONS / 8,
                        DEFAULT_PAN_POSITIONS * 3 / 8,
                        DEFAULT_PAN_POSITIONS * 5 / 8,
                        DEFAULT_PAN_POSITIONS * 7 / 8,
                    ][bus],
                    volume_pct: 100.0,
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn spread_delay_slot() -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: "delay".into(),
        params: BTreeMap::from([
            ("mixPct".into(), json!(10.0)),
            ("spreadPct".into(), json!(25.0)),
        ]),
    }
}

#[test]
fn routing_tree_actual_threaded_over_max_combined_completion_is_terminal() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(bus_config_with_reverb());
    engine.note_on(0, 60, 100, 500);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_routing_tree_for_test(&engine, 128, engine.sample_clock()));
    for parity in 0..2 {
        for _ in 0..100_000 {
            if runtime.completion_ready_for_test(parity) {
                break;
            }
            thread::yield_now();
        }
    }
    assert!(runtime.rewrite_completion_measurement_for_test(
        0,
        1_000_000,
        crate::synth::SOURCE_WORKER_MAX_COST_UNITS + 1,
    ));
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );
    shutdown(lifecycle, runtime);
}

fn partition_fill_config(kind: &str) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: (0..4)
            .map(|_| InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

#[test]
fn routing_tree_synth_note_admission_overflow_is_rejected_without_mutation() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(partition_fill_config("synth"));
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    assert_eq!(
        engine
            .routing_tree_assignment
            .as_ref()
            .and_then(|assignment| assignment.worker_for_slot(0)),
        Some(0)
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            let per_slot = crate::synth::types::SYNTH_VOICE_PARTITION_LANE_CAPACITY / 4;
            for slot in 0..4 {
                for note in 0..per_slot {
                    engine.note_on(slot as u8, 36 + note as u8, 100, 5_000);
                }
            }
        })
        .is_some());
    let before = recovered_profile(&mut runtime, &mut engine);
    assert_eq!(
        before.active_synth_voices,
        crate::synth::types::SYNTH_VOICE_PARTITION_LANE_CAPACITY
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.note_on(0, 36, 100, 5_000);
        })
        .is_some());
    assert!(engine.take_routing_tree_rejection());
    assert_eq!(recovered_profile(&mut runtime, &mut engine), before);
    assert!(engine.routing_tree_assignment_is_valid());
    shutdown(lifecycle, runtime);
}

fn sample_partition_banks() -> Vec<SampleBankConfig> {
    let mut banks = vec![SampleBankConfig::default(); 8];
    for bank in banks.iter_mut().take(4) {
        bank.slots[0] = SampleSlotConfig {
            buffer: Some(SampleBuffer {
                samples: vec![0.25; 128].into(),
                channels: 1,
                sample_rate: 44_100,
            }),
        };
    }
    banks
}

#[test]
fn routing_tree_sample_note_admission_overflow_is_rejected_without_mutation() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(partition_fill_config("sampler"));
    drop(engine.set_sample_banks(sample_partition_banks()));
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    assert_eq!(
        engine
            .routing_tree_assignment
            .as_ref()
            .and_then(|assignment| assignment.worker_for_slot(0)),
        Some(0)
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            let per_slot = crate::synth::types::SAMPLE_VOICE_PARTITION_LANE_CAPACITY / 4;
            for slot in 0..4 {
                for _ in 0..per_slot {
                    engine.note_on(slot as u8, 36, 100, 5_000);
                }
            }
        })
        .is_some());
    let before = recovered_profile(&mut runtime, &mut engine);
    assert_eq!(
        before.active_sample_voices,
        crate::synth::types::SAMPLE_VOICE_PARTITION_LANE_CAPACITY
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.note_on(0, 36, 100, 5_000);
        })
        .is_some());
    assert!(engine.take_routing_tree_rejection());
    assert_eq!(recovered_profile(&mut runtime, &mut engine), before);
    assert!(engine.routing_tree_assignment_is_valid());
    shutdown(lifecycle, runtime);
}

fn recovered_profile(
    runtime: &mut SourceWorkerRuntime,
    engine: &mut SynthEngine,
) -> crate::synth::SynthProfileSnapshot {
    runtime
        .with_recovered_owners(engine, |engine| engine.profile_snapshot())
        .expect("recovered source owners")
}
