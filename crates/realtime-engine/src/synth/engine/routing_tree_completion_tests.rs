use self::spread_state::{
    assert_spread_state_is_nontrivial, assert_spread_state_matches,
    bus_output_spread_state_signature, recovered_source_state_signature,
};
use super::routing_tree_executor_state_tests::engine_source_state_signature;
use super::routing_tree_executor_test_support::bus_chain_state;
use super::routing_tree_pipeline_tests::{
    assert_global_mixer_state_matches, assert_interleaved_reassociated_close,
    assert_worker_outputs_are_nonzero, shutdown,
};
use super::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MixerConfig, SourceWorkerHealth, SourceWorkerLifecycle, SourceWorkerRenderDisposition,
    SynthEngine,
};
use crate::synth::types::{default_synth_config, DEFAULT_PAN_POSITIONS};
use crate::synth::{
    MomentaryFxTarget, SampleBankConfig, SampleBuffer, SampleSlotConfig, VoiceStealingMode,
};
use serde_json::json;
use std::collections::BTreeMap;

#[path = "routing_tree_spread_state_test_support.rs"]
mod spread_state;

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
    normal_runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    reverse_runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
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

#[test]
fn routing_tree_inline_parity_tracks_bus_hold_expiry_inside_quantum() {
    let config = hold_expiry_config();
    let mut routed = SynthEngine::new(44_100);
    let mut inline = SynthEngine::new(44_100);
    for engine in [&mut routed, &mut inline] {
        engine.set_instruments(config.clone());
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut routed, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut inline_left = vec![0.0; 128];
    let mut inline_right = vec![0.0; 128];
    let mut inline_out = vec![0.0; 256];
    inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
    inline.bus_chains[0].render_hold_frames = 64;
    assert!(runtime
        .with_controls_ready(&mut routed, |engine| {
            engine.bus_chains[0].render_hold_frames = 64;
        })
        .is_some());

    let mut routed_left = vec![0.0; 128];
    let mut routed_right = vec![0.0; 128];
    let mut routed_out = vec![0.0; 256];
    assert_eq!(
        routed.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut routed_left,
            &mut routed_right,
            &mut routed_out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        routed.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut routed_left,
            &mut routed_right,
            &mut routed_out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
    assert_interleaved_reassociated_close(&runtime, &routed_out, &inline_out);
    assert_eq!(
        routed.block_slot_scratch.bus_active[..128]
            .iter()
            .filter(|active| **active)
            .count(),
        63
    );
    assert_eq!(routed.master_activity_frames, inline.master_activity_frames);
    assert_eq!(routed.dry_history, inline.dry_history);
    lifecycle.shutdown(runtime.retire());
}

#[test]
fn routing_tree_inline_parity_tracks_local_momentary_expiry_frame() {
    let config = momentary_expiry_config();
    let mut routed = SynthEngine::new(44_100);
    let mut inline = SynthEngine::new(44_100);
    for engine in [&mut routed, &mut inline] {
        engine.set_instruments(config.clone());
        engine.momentary_fx_start(
            "expiry".into(),
            "filter_sweep".into(),
            BTreeMap::from([
                ("sweepInMs".into(), json!(1.0)),
                ("sweepOutMs".into(), json!(1.0)),
            ]),
            MomentaryFxTarget::Instrument { index: 0 },
        );
    }
    let mut warm_left = vec![0.0; 128];
    let mut warm_right = vec![0.0; 128];
    let mut warm_out = vec![0.0; 256];
    inline.render_interleaved_block(128, &mut warm_left, &mut warm_right, &mut warm_out);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut routed, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    assert!(runtime
        .with_controls_ready(&mut routed, |engine| {
            engine.momentary_fx_stop("expiry");
        })
        .is_some());
    inline.momentary_fx_stop("expiry");

    let mut routed_left = vec![0.0; 128];
    let mut routed_right = vec![0.0; 128];
    let mut routed_out = vec![0.0; 256];
    assert_eq!(
        routed.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut routed_left,
            &mut routed_right,
            &mut routed_out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        routed.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut routed_left,
            &mut routed_right,
            &mut routed_out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );

    let mut expected_active = Vec::with_capacity(128);
    let mut inline_out = vec![0.0; 256];
    for frame in 0..128 {
        let (left, right) = inline.next_stereo_sample();
        inline_out[frame * 2] = left;
        inline_out[frame * 2 + 1] = right;
        expected_active.push(!inline.momentary_fx.is_empty());
    }
    assert_eq!(routed_out, vec![0.0; 256]);
    assert_eq!(routed_out, inline_out);
    assert_eq!(
        &routed.block_slot_scratch.source_active[..128],
        expected_active.as_slice()
    );
    let expiry_frame = expected_active
        .iter()
        .position(|active| !active)
        .expect("momentary expiry frame");
    assert_eq!(expiry_frame, 44);
    assert!(expected_active[..expiry_frame].iter().all(|active| *active));
    assert!(!routed.block_slot_scratch.source_active[expiry_frame]);
    shutdown(lifecycle, runtime);
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

fn hold_expiry_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".into(),
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

fn momentary_expiry_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
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
