use super::*;
use crate::synth::{SampleLoopMode, SamplePlaybackConfig};

const RATE: u32 = 44_100;
const BLOCK: usize = 128;

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
                route: route.into(),
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

/// Renders `blocks` blocks on the routing-tree workers and inline, asserting identical output.
/// `stop` is the block whose control point stops the Play FX on the workers, and the block the
/// inline engine stops it before. The workers dispatch one block ahead, so a control reaches a
/// worker-owned target one block later than a Global one.
fn assert_worker_render_matches_inline(
    routed: &mut SynthEngine,
    inline: &mut SynthEngine,
    blocks: usize,
    label: &str,
    stop: Option<(usize, usize)>,
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
    for block in 0..blocks {
        if stop.is_some_and(|(_, inline_block)| inline_block == block) {
            drop(inline.momentary_fx_stop_by_epoch(1));
        }
        let routed_stop = stop.is_some_and(|(routed_block, _)| routed_block == block);
        assert_eq!(
            routed.render_interleaved_block_with_source_runtime_ready_with_controls(
                &mut runtime,
                BLOCK,
                &mut left,
                &mut right,
                &mut routed_out,
                |engine| {
                    if routed_stop {
                        drop(engine.momentary_fx_stop_by_epoch(1));
                    }
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
                80,
                &label,
                Some((40, inline_stop)),
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
            40,
            &format!("{loop_mode:?} loop via {route}"),
            None,
        );
    }
}
