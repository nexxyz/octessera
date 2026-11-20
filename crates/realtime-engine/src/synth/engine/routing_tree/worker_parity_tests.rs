use super::super::*;
use crate::synth::{
    FxParamId, FxParamMutation, SampleBankParamId, SampleLoopMode, SamplePlaybackConfig,
    ScalarMutation,
};

const RATE: u32 = 44_100;
const BLOCK: usize = 128;
/// 1.25 s, so a released note's tail and five repeats of the default 250 ms bus delay are
/// compared.
const BLOCKS: usize = 431;

const PLAY_FX_TYPES: [&str; 11] = [
    "stutter",
    "freeze",
    "filter_sweep",
    "pitch_shift",
    "tape_stop",
    "reverse",
    "trance_gate",
    "dub_throw",
    "wash",
    "hp_riser",
    "crush",
];

fn one_instrument_config(kind: &str, route: &str) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: kind.into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: MixerRoute::from_wire_name(route).unwrap(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("delay".into())],
                ..FxBusConfig::default()
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

/// Renders `BLOCKS` blocks on the routing-tree workers and inline, asserting identical output.
/// `control(engine, block, routed)` runs at each routed block's control point and before each
/// inline block. The workers dispatch one block ahead, so a control reaches a worker-owned
/// target one block later than a Global one.
fn assert_worker_render_matches_inline(
    routed: &mut SynthEngine,
    inline: &mut SynthEngine,
    label: &str,
    control: impl Fn(&mut SynthEngine, usize, bool),
) {
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(routed, BLOCK)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = vec![0.0; BLOCK];
    let mut right = vec![0.0; BLOCK];
    let mut routed_out = vec![0.0; BLOCK * 2];
    let mut inline_left = Vec::new();
    let mut inline_right = Vec::new();
    let mut inline_out = Vec::new();
    let mut peak = 0.0_f32;
    for block in 0..BLOCKS {
        control(inline, block, false);
        assert_eq!(
            routed.render_interleaved_block_with_source_runtime_ready_with_controls(
                &mut runtime,
                BLOCK,
                &mut left,
                &mut right,
                &mut routed_out,
                |engine| {
                    control(engine, block, true);
                    Ok(())
                },
            ),
            SourceWorkerRenderDisposition::Fresh,
            "{label} block {block}"
        );
        inline.render_interleaved_block(
            BLOCK,
            &mut inline_left,
            &mut inline_right,
            &mut inline_out,
        );
        for (index, (routed, inline)) in routed_out.iter().zip(&inline_out).enumerate() {
            assert_eq!(
                routed.to_bits(),
                inline.to_bits(),
                "{label} block {block} sample {index}: {routed} vs {inline}"
            );
            peak = peak.max(inline.abs());
        }
    }
    assert!(peak > 1.0e-3, "{label} rendered silence");
    lifecycle.shutdown(runtime.retire());
}

#[test]
fn every_play_fx_type_renders_the_same_on_the_routing_tree_workers() {
    for fx_type in PLAY_FX_TYPES {
        for target in [
            MomentaryFxTarget::Instrument { index: 0 },
            MomentaryFxTarget::FxBus { index: 0 },
            MomentaryFxTarget::Global,
        ] {
            let label = format!("{fx_type} on {target:?}");
            let mut routed = SynthEngine::new(RATE);
            let mut inline = SynthEngine::new(RATE);
            for engine in [&mut routed, &mut inline] {
                engine.set_instruments(one_instrument_config("synth", "fx_bus_1"));
                let start = prepare_momentary_fx_start_with_epoch(
                    "fx".into(),
                    1,
                    fx_type.into(),
                    BTreeMap::new(),
                    target,
                    RATE,
                )
                .unwrap();
                drop(engine.apply_prepared_momentary_fx_start(start));
                engine.note_on(0, 60, 100, 200);
            }
            let inline_stop = if target == MomentaryFxTarget::Global {
                40
            } else {
                41
            };
            assert_worker_render_matches_inline(
                &mut routed,
                &mut inline,
                &label,
                |engine, block, routed| {
                    if block == if routed { 40 } else { inline_stop } {
                        drop(engine.momentary_fx_stop_by_epoch(1));
                    }
                },
            );
        }
    }
}

#[test]
fn looping_sampler_renders_the_same_on_the_routing_tree_workers() {
    let samples: Vec<f32> = (0..1_000)
        .map(|frame| (frame as f32 * 0.07).sin() * 0.5)
        .collect();
    for (loop_mode, route) in [
        (SampleLoopMode::Forward, "direct"),
        (SampleLoopMode::PingPong, "direct"),
        (SampleLoopMode::Forward, "fx_bus_1"),
        (SampleLoopMode::PingPong, "fx_bus_1"),
    ] {
        let mut bank = SampleBankConfig::default();
        bank.slots[0].buffer = Some(SampleBuffer {
            samples: samples.clone().into(),
            channels: 1,
            sample_rate: RATE,
        });
        bank.slots[0].playback = SamplePlaybackConfig {
            loop_mode,
            start_pct: 5.0,
            loop_start_pct: 20.0,
            loop_end_pct: 70.0,
        };
        let mut banks = vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT];
        banks[0] = bank;
        let mut routed = SynthEngine::new(RATE);
        let mut inline = SynthEngine::new(RATE);
        for engine in [&mut routed, &mut inline] {
            engine.set_instruments(one_instrument_config("sampler", route));
            drop(engine.set_sample_banks(banks.clone()));
            engine.note_on(0, 36, 110, 400);
        }
        assert_worker_render_matches_inline(
            &mut routed,
            &mut inline,
            &format!("{loop_mode:?} loop via {route}"),
            |engine, block, routed| {
                if block + usize::from(routed) == 60 {
                    assert_eq!(
                        engine.set_sample_bank_param_typed(
                            0,
                            SampleBankParamId::FilterCutoffHz,
                            700.0
                        ),
                        ScalarMutation::Changed
                    );
                }
            },
        );
    }
}

#[test]
fn level_glides_render_the_same_on_the_routing_tree_workers() {
    let mut routed = SynthEngine::new(RATE);
    let mut inline = SynthEngine::new(RATE);
    for engine in [&mut routed, &mut inline] {
        engine.set_instruments(one_instrument_config("synth", "direct"));
        engine.note_on(0, 60, 100, 2_000);
    }
    assert_worker_render_matches_inline(
        &mut routed,
        &mut inline,
        "level glides",
        |engine, block, routed| {
            if block + usize::from(routed) == 2 {
                engine.set_synth_param_typed(0, crate::synth::SynthParamId::AmpGainPct, 30.0);
                engine.set_synth_param_typed(0, crate::synth::SynthParamId::Osc1LevelPct, 20.0);
            }
        },
    );
}

#[test]
fn mixer_gain_ramps_render_the_same_on_the_routing_tree_workers() {
    for route in ["direct", "fx_bus_1"] {
        let mut routed = SynthEngine::new(RATE);
        let mut inline = SynthEngine::new(RATE);
        for engine in [&mut routed, &mut inline] {
            engine.set_instruments(one_instrument_config("synth", route));
            engine.note_on(0, 60, 100, 2_000);
        }
        assert_worker_render_matches_inline(
            &mut routed,
            &mut inline,
            &format!("mixer ramps via {route}"),
            |engine, block, routed| {
                let worker_block = if routed { block + 1 } else { block };
                if block == 10 {
                    assert_eq!(engine.set_master_volume(40.0), ScalarMutation::Changed);
                }
                if worker_block == 20 {
                    engine.set_instrument_mixer(0, Some(30.0), Some(0));
                }
                if worker_block == 21 {
                    engine.set_instrument_mixer(0, Some(90.0), None);
                }
                if worker_block == 30 {
                    engine.set_fx_bus_mixer(0, Some(DEFAULT_PAN_POSITIONS - 1), Some(20.0));
                }
            },
        );
    }
}

const BUS_FX_TYPES: [&str; 16] = [
    "tremolo",
    "delay",
    "vibrato",
    "chorus",
    "flanger",
    "filter_lfo",
    "wah",
    "reverb",
    "glitch",
    "auto_pan",
    "saturator",
    "distortion",
    "bitcrusher",
    "compressor",
    "eq",
    "vinyl",
];

const MASTER_FX_TYPES: [&str; 5] = ["saturator", "distortion", "compressor", "eq", "vinyl"];

fn assert_fx_config_renders_the_same(config: &InstrumentsConfig, label: &str) {
    let mut routed = SynthEngine::new(RATE);
    let mut inline = SynthEngine::new(RATE);
    for engine in [&mut routed, &mut inline] {
        engine.set_instruments(config.clone());
        for slot in 0..config.instruments.len() as u8 {
            engine.note_on(slot, 60 + slot * 7, 100, 300);
        }
    }
    let has_master = config
        .mixer
        .as_ref()
        .is_some_and(|mixer| mixer.master.is_some());
    assert_worker_render_matches_inline(
        &mut routed,
        &mut inline,
        label,
        |engine, block, routed| {
            if block + usize::from(routed) == 60 {
                let glided = [FxParamId::MixPct, FxParamId::DepthPct, FxParamId::AmountPct]
                    .into_iter()
                    .any(|id| engine.set_fx_bus_param(0, 0, id, 25.0) == FxParamMutation::Changed);
                assert!(glided, "{label} bus glide");
            }
            if has_master && block == 80 {
                assert_eq!(
                    engine.set_global_fx_param(0, FxParamId::MixPct, 40.0),
                    FxParamMutation::Changed,
                    "{label} master glide"
                );
            }
        },
    );
}

#[test]
fn every_bus_fx_type_renders_the_same_on_the_routing_tree_workers() {
    for fx_type in BUS_FX_TYPES {
        let mut config = one_instrument_config("synth", "fx_bus_1");
        config.mixer.as_mut().unwrap().buses[0].slots = vec![FxBusSlotConfig::Kind(fx_type.into())];
        assert_fx_config_renders_the_same(&config, &format!("bus {fx_type}"));
    }
}

#[test]
fn duck_renders_the_same_on_the_routing_tree_workers() {
    for source in ["I2", "B2"] {
        let mut config = one_instrument_config("synth", "fx_bus_1");
        let mut source_slot = config.instruments[0].clone();
        source_slot.mixer.as_mut().unwrap().route = MixerRoute::FxBus(1);
        config.instruments.push(source_slot);
        let mixer = config.mixer.as_mut().unwrap();
        mixer.buses[0].slots = vec![FxBusSlotConfig::Config {
            kind: "duck".into(),
            params: [
                ("source".into(), Value::from(source)),
                ("amountPct".into(), Value::from(80.0)),
            ]
            .into_iter()
            .collect(),
        }];
        mixer.buses.push(FxBusConfig {
            slots: vec![FxBusSlotConfig::Kind("delay".into())],
            ..FxBusConfig::default()
        });
        assert_fx_config_renders_the_same(&config, &format!("duck from {source}"));
    }
}

#[test]
fn every_master_fx_type_renders_the_same_on_the_routing_tree_workers() {
    for fx_type in MASTER_FX_TYPES {
        for route in ["direct", "fx_bus_1"] {
            let mut config = one_instrument_config("synth", route);
            config.mixer.as_mut().unwrap().master = Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind(fx_type.into())],
            });
            assert_fx_config_renders_the_same(&config, &format!("master {fx_type} via {route}"));
        }
    }
}
