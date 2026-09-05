use super::super::fx::FxBusState;
use super::routing_tree_plan::RoutingTreePlan;
use super::SynthEngine;
use crate::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, SampleBankConfig,
    SampleBuffer, SampleSlotConfig, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
    SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
};
use serde_json::json;
use std::collections::BTreeMap;

pub(super) fn direct_synth_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: (0..2)
            .map(|_| InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

pub(super) fn bus_assignment_config() -> InstrumentsConfig {
    let mut config = direct_synth_config();
    config.mixer = Some(MixerConfig {
        buses: vec![FxBusConfig::default(), FxBusConfig::default()],
        master: None,
    });
    config
}

pub(super) fn master_fx_config() -> InstrumentsConfig {
    let mut config = sample_direct_config();
    config.mixer = Some(MixerConfig {
        buses: Vec::new(),
        master: Some(MasterFxConfig {
            slots: vec![FxBusSlotConfig::Kind("compressor".into())],
        }),
    });
    config
}

pub(super) fn raw_duck_config() -> InstrumentsConfig {
    let mut config = direct_synth_config();
    config.instruments[0].kind = "sampler".into();
    config.instruments[0].mixer.as_mut().unwrap().route = "fx_bus_1".into();
    config.instruments[1].kind = "sampler".into();
    config.instruments[1].mixer.as_mut().unwrap().volume = 25.0;
    config.instruments.push(InstrumentSlotConfig {
        kind: "sampler".into(),
        synth: default_synth_config(),
        mixer: Some(InstrumentMixerConfig {
            route: "fx_bus_2".into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 40.0,
        }),
    });
    let instrument_duck = FxBusSlotConfig::Config {
        kind: "duck".into(),
        params: [
            ("source".into(), json!("I2")),
            ("amountPct".into(), json!(80.0)),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>(),
    };
    let bus_duck = FxBusSlotConfig::Config {
        kind: "duck".into(),
        params: [
            ("source".into(), json!("B2")),
            ("amountPct".into(), json!(80.0)),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>(),
    };
    config.mixer = Some(MixerConfig {
        buses: vec![
            FxBusConfig {
                slots: vec![instrument_duck, bus_duck],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            },
            FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("delay".into())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            },
        ],
        master: None,
    });
    config
}

pub(super) fn invalid_state_config() -> InstrumentsConfig {
    let mut config = routed_config();
    config.mixer.as_mut().unwrap().master = Some(MasterFxConfig {
        slots: vec![FxBusSlotConfig::Kind("compressor".into())],
    });
    config
}

pub(super) fn sample_direct_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".into(),
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

pub(super) fn sample_bus_config() -> InstrumentsConfig {
    let mut config = sample_direct_config();
    config.instruments[0].mixer.as_mut().unwrap().route = "fx_bus_1".into();
    config.mixer = Some(MixerConfig {
        buses: vec![FxBusConfig {
            slots: vec![FxBusSlotConfig::Kind("delay".into())],
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume_pct: 100.0,
        }],
        master: None,
    });
    config
}

pub(super) fn stereo_bus_config() -> InstrumentsConfig {
    let mut config = sample_direct_config();
    config.instruments[0].kind = "synth".into();
    config.instruments[0].synth = default_synth_config();
    config.instruments[0].mixer.as_mut().unwrap().route = "fx_bus_1".into();
    config.mixer = Some(MixerConfig {
        buses: vec![FxBusConfig {
            slots: vec![
                FxBusSlotConfig::Config {
                    kind: "delay".into(),
                    params: [
                        ("mixPct".into(), json!(50.0)),
                        ("spreadPct".into(), json!(100.0)),
                    ]
                    .into_iter()
                    .collect(),
                },
                FxBusSlotConfig::Config {
                    kind: "auto_pan".into(),
                    params: [
                        ("rateHz".into(), json!(1.0)),
                        ("depthPct".into(), json!(100.0)),
                    ]
                    .into_iter()
                    .collect(),
                },
            ],
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume_pct: 100.0,
        }],
        master: None,
    });
    config
}

pub(super) fn routed_config() -> InstrumentsConfig {
    let duck = FxBusSlotConfig::Config {
        kind: "duck".into(),
        params: [
            ("source".into(), json!("I2")),
            ("amountPct".into(), json!(60.0)),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>(),
    };
    InstrumentsConfig {
        instruments: ["sampler", "synth", "synth", "synth"]
            .into_iter()
            .enumerate()
            .map(|(slot, kind)| InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: if slot == 0 { "fx_bus_1" } else { "direct" }.into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("delay".into()), duck],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

pub(super) fn sample_banks() -> Vec<SampleBankConfig> {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![1.0, 0.5, 0.25, 0.0].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    let mut banks = vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT];
    banks[0] = bank;
    banks
}

pub(super) fn assert_routing_tree_matches_reference(
    tree: &mut SynthEngine,
    reference: &mut SynthEngine,
    frames: usize,
) {
    let plan = RoutingTreePlan::from_render_plan(&tree.render_plan);
    let mut left = vec![0.0; frames];
    let mut right = vec![0.0; frames];
    let mut expected_left = vec![0.0; frames];
    let mut expected_right = vec![0.0; frames];
    let mut expected_interleaved = Vec::new();
    reference.render_interleaved_block(
        frames,
        &mut expected_left,
        &mut expected_right,
        &mut expected_interleaved,
    );
    assert!(tree.render_routing_tree_block_for_test(frames, &mut left, &mut right));
    for frame in 0..frames {
        let expected_left = expected_left[frame];
        let expected_right = expected_right[frame];
        if plan.component_count <= 1 {
            assert_eq!(left[frame], expected_left, "left frame {frame}");
            assert_eq!(right[frame], expected_right, "right frame {frame}");
        } else {
            let workers = tree.routing_tree_scratch.worker_outputs_for_test(frame);
            assert_reassociated_close(left[frame], expected_left, workers, 0, "left frame");
            assert_reassociated_close(right[frame], expected_right, workers, 1, "right frame");
        }
    }
    assert_eq!(tree.sample_clock, reference.sample_clock);
    assert_eq!(tree.profile_snapshot(), reference.profile_snapshot());
    assert_eq!(
        tree.active_bus_activity_count,
        reference.active_bus_activity_count
    );
    assert_eq!(
        format!("{:?}", tree.bus_chains),
        format!("{:?}", reference.bus_chains)
    );
    assert_momentary_state_matches(tree, reference);
}

pub(super) fn assert_momentary_state_matches(actual: &SynthEngine, expected: &SynthEngine) {
    assert_eq!(actual.momentary_fx.len(), expected.momentary_fx.len());
    for (actual, expected) in actual.momentary_fx.iter().zip(&expected.momentary_fx) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.target, expected.target);
        assert_eq!(actual.releasing, expected.releasing);
        assert_eq!(actual.release_pos, expected.release_pos);
        assert_eq!(actual.release_len, expected.release_len);
        assert_eq!(actual.sweep_pos.to_bits(), expected.sweep_pos.to_bits());
        assert_eq!(actual.filt_l, expected.filt_l);
        assert_eq!(actual.filt_r, expected.filt_r);
        assert_eq!(actual.pitch_ramp_pos, expected.pitch_ramp_pos);
        assert_eq!(actual.pitch_ramp_len, expected.pitch_ramp_len);
        assert_eq!(actual.stutter_write, expected.stutter_write);
        assert_eq!(actual.stutter_ready, expected.stutter_ready);
        assert_eq!(actual.stutter_segment_len, expected.stutter_segment_len);
        assert_eq!(actual.stutter_ramp_len, expected.stutter_ramp_len);
        assert_eq!(actual.stutter_ramp_pos, expected.stutter_ramp_pos);
        assert_eq!(actual.freeze_idxs, expected.freeze_idxs);
        assert_eq!(actual.freeze_lp, expected.freeze_lp);
        assert_eq!(actual.freeze_inject_pos, expected.freeze_inject_pos);
        assert_eq!(actual.freeze_inject_len, expected.freeze_inject_len);
        assert_eq!(
            actual.pitch_shifter.write_pos,
            expected.pitch_shifter.write_pos
        );
    }
}

pub(super) fn engine_state_signature(engine: &SynthEngine) -> String {
    let synth_voices = active_synth_voice_state(engine);
    let sample_voices = active_sample_voice_state(engine);
    let bus_chains = bus_chain_state(engine);
    let momentary = momentary_state(engine);
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        engine.sample_clock,
        synth_voices,
        sample_voices,
        engine.active_synth_slots,
        engine.active_sample_slots,
        format!("{:?}", engine.preview_sample_voices),
        bus_chains,
        format!("{:?}", engine.bus_output_spread_state),
        &engine.bus_mono_scratch,
        &engine.bus_mono_snapshot,
        &engine.master_slot_params,
        format!("{:?}", engine.master_slot_state),
        &engine.master_active_slot_indices,
        engine.master_activity_frames,
        engine.active_bus_activity_count,
        engine.routed_bus_slot_count,
        momentary,
        &engine.dry_history,
        engine.dry_history_pos,
        engine.cumulative_voice_steals,
        engine.cumulative_voice_admission_drops,
        engine.voice_steal_since_status,
        engine.render_profile.snapshot(),
        engine.pending_render_retired.is_empty(),
        engine.pending_render_retired.sample_voice_count(),
    )
}

pub(super) fn bus_chain_state(engine: &SynthEngine) -> Vec<String> {
    engine
        .bus_chains
        .iter()
        .map(|chain| {
            format!(
                "{:?}",
                (
                    chain.logical_bus_id,
                    &chain.slot_params,
                    &chain.slot_state,
                    &chain.slot_costs,
                    &chain.active_slot_indices,
                    chain.active_slot_count,
                    chain.render_hold_frames,
                    chain.quiet_frames,
                )
            )
        })
        .collect()
}

pub(super) fn momentary_state(engine: &SynthEngine) -> Vec<String> {
    engine
        .momentary_fx
        .iter()
        .map(|fx| {
            format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                &fx.id,
                fx.kind,
                fx.target,
                fx.releasing,
                fx.release_pos,
                fx.release_len,
                fx.sweep_pos,
                fx.filt_l,
                fx.filt_r,
                fx.pitch_ramp_pos,
                fx.pitch_ramp_len,
                fx.stutter_write,
                fx.stutter_ready,
                fx.stutter_segment_len,
                fx.stutter_ramp_len,
                fx.stutter_ramp_pos,
                fx.freeze_idxs,
                fx.freeze_lp,
                fx.freeze_inject_pos,
                fx.freeze_inject_len,
                fx.pitch_shifter.write_pos,
            )
        })
        .collect()
}

pub(super) fn active_synth_voice_state(engine: &SynthEngine) -> Vec<String> {
    let mut voices = (0..SYNTH_VOICE_LANE_CAPACITY)
        .filter_map(|lane| {
            let voice = engine.synth_voice_pool.lane(lane)?;
            voice.active.then(|| {
                (
                    voice
                        .canonical_lane
                        .expect("active synth voice canonical lane"),
                    format!("{:?}", voice),
                )
            })
        })
        .collect::<Vec<_>>();
    voices.sort_unstable_by_key(|(canonical_lane, _)| *canonical_lane);
    voices
        .into_iter()
        .map(|(canonical_lane, state)| format!("{canonical_lane}:{state}"))
        .collect()
}

pub(super) fn active_sample_voice_state(engine: &SynthEngine) -> Vec<String> {
    let mut voices = (0..SAMPLE_VOICE_LANE_CAPACITY)
        .filter_map(|lane| {
            let voice = engine.sample_voice_pool.lane(lane)?;
            voice.active.then(|| {
                (
                    voice
                        .canonical_lane
                        .expect("active sample voice canonical lane"),
                    format!("{:?}", voice),
                )
            })
        })
        .collect::<Vec<_>>();
    voices.sort_unstable_by_key(|(canonical_lane, _)| *canonical_lane);
    voices
        .into_iter()
        .map(|(canonical_lane, state)| format!("{canonical_lane}:{state}"))
        .collect()
}

pub(super) fn assert_duck_env(state: &FxBusState, source: f32, attack_ms: f32, label: &str) {
    let attack_samples = (attack_ms / 1000.0 * 48_000.0).max(1.0);
    let expected = (source.abs().min(1.0) - 0.0) * (1.0 / attack_samples);
    match state {
        FxBusState::Duck { env } => assert_eq!(*env, expected, "{label} duck source"),
        _ => panic!("expected {label} duck state"),
    }
}

pub(super) fn assert_ulp_close(actual: f32, expected: f32, max_ulps: u32, label: &str) {
    let distance = f32_ulp_distance(actual, expected);
    assert!(
        distance <= max_ulps,
        "{label}: actual {actual}, expected {expected}, ULP distance {distance} > {max_ulps}"
    );
}

pub(super) fn assert_reassociated_close(
    actual: f32,
    expected: f32,
    workers: [(f32, f32); 2],
    channel: usize,
    label: &str,
) {
    let magnitude = workers
        .iter()
        .map(|(left, right)| {
            if channel == 0 {
                left.abs()
            } else {
                right.abs()
            }
        })
        .sum::<f32>();
    let unit_roundoff = f32::EPSILON * 0.5;
    let operation_count = (INSTRUMENT_SLOT_COUNT + super::super::types::BUS_COUNT) as f32;
    let error_ratio = (operation_count * unit_roundoff) / (1.0 - operation_count * unit_roundoff);
    let bound = magnitude * error_ratio;
    let ulp = ulp_size(actual.abs().max(expected.abs()));
    let max_ulps = (bound / ulp).ceil() as u32 + 1;
    assert_ulp_close(actual, expected, max_ulps, label);
}

fn ulp_size(value: f32) -> f32 {
    if value == 0.0 {
        return f32::from_bits(1);
    }
    f32::from_bits(value.to_bits().saturating_add(1)) - value
}

fn f32_ulp_distance(actual: f32, expected: f32) -> u32 {
    if actual == expected {
        return 0;
    }
    if !actual.is_finite() || !expected.is_finite() {
        return u32::MAX;
    }
    let ordered = |value: f32| {
        let bits = value.to_bits() as i32;
        if bits < 0 {
            i32::MIN.wrapping_sub(bits)
        } else {
            bits
        }
    };
    ordered(actual).abs_diff(ordered(expected))
}
