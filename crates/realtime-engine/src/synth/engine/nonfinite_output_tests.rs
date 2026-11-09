use super::*;
use crate::synth::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, SampleBankConfig, SampleBuffer,
};
use serde_json::json;
use std::collections::BTreeMap;

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const BLOCKS: usize = 24;
const NAN_FRAME: usize = 64;

/// A sampler through a feedback delay bus into a master EQ: every stage keeps signal history
/// that one NaN would poison for good.
fn feedback_delay_config() -> InstrumentsConfig {
    let slot = |kind: &str, params: &[(&str, f32)]| FxBusSlotConfig::Config {
        kind: kind.into(),
        params: params
            .iter()
            .map(|(name, value)| (name.to_string(), json!(value)))
            .collect::<BTreeMap<_, _>>(),
    };
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: MixerRoute::FxBus(0),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![slot(
                    "delay",
                    &[("timeMs", 20.0), ("feedback", 0.6), ("mixPct", 50.0)],
                )],
                ..FxBusConfig::default()
            }],
            master: Some(MasterFxConfig {
                slots: vec![slot("eq", &[("lowGainDb", 3.0), ("highGainDb", -3.0)])],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

/// A steady signal long enough to outlast the test, with one NaN early in the first block.
fn nan_engine() -> SynthEngine {
    let mut samples = vec![0.25_f32; RATE as usize];
    samples[NAN_FRAME] = f32::NAN;
    let mut bank = SampleBankConfig::default();
    bank.slots[0].buffer = Some(SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: RATE,
    });
    let mut banks = vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT];
    banks[0] = bank;
    let mut engine = SynthEngine::new(RATE);
    engine.set_instruments(feedback_delay_config());
    drop(engine.set_sample_banks(banks));
    engine.note_on(0, 36, 110, 1_000);
    engine
}

/// The block plays until the NaN arrives and is silent from there to its end.
fn assert_silenced_from_the_nan(out: &[f32], label: &str) {
    let first_silent = out
        .iter()
        .position(|sample| *sample == 0.0)
        .expect("silenced frames");
    assert!(first_silent > 0, "{label} played nothing before the NaN");
    assert!(
        out[first_silent..].iter().all(|sample| *sample == 0.0),
        "{label} block was not silent after the NaN"
    );
}

fn assert_recovered(blocks: &[Vec<f32>], label: &str) {
    for (block, out) in blocks.iter().enumerate() {
        for (index, sample) in out.iter().enumerate() {
            assert!(
                sample.is_finite() && sample.abs() > 1.0e-4,
                "{label} block {block} sample {index}: {sample}"
            );
        }
    }
}

#[test]
fn block_render_silences_a_nan_and_clears_the_fx_it_reached() {
    let mut engine = nan_engine();
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
    assert_silenced_from_the_nan(&out, "block");
    assert!(engine.take_output_clipped());

    let blocks: Vec<Vec<f32>> = (0..BLOCKS)
        .map(|_| {
            engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
            out.clone()
        })
        .collect();
    assert_recovered(&blocks, "block");
    let (_, delay_energy) = engine.delay_state_probe(0, 0).expect("delay state");
    assert!(delay_energy.is_finite() && delay_energy > 0.0);
}

#[test]
fn serial_frames_silence_a_nan_and_clear_the_fx_it_reached() {
    let mut engine = nan_engine();
    let first: Vec<(f32, f32)> = (0..BLOCK).map(|_| engine.next_stereo_sample()).collect();
    let silenced = first
        .iter()
        .position(|frame| *frame == (0.0, 0.0))
        .expect("silenced frame");
    assert!(silenced > 0);
    assert!(engine.take_output_clipped());

    let frames: Vec<f32> = (0..BLOCK * BLOCKS)
        .flat_map(|_| {
            let (left, right) = engine.next_stereo_sample();
            [left, right]
        })
        .collect();
    assert_recovered(&[frames], "serial");
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_workers_clear_the_bus_fx_a_nan_reached() {
    let mut engine = nan_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, BLOCK)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = vec![0.0; BLOCK];
    let mut right = vec![0.0; BLOCK];
    let mut out = vec![0.0; BLOCK * 2];
    let mut render = |engine: &mut SynthEngine| {
        assert_eq!(
            engine.render_interleaved_block_with_source_runtime_ready(
                &mut runtime,
                BLOCK,
                &mut left,
                &mut right,
                &mut out,
            ),
            SourceWorkerRenderDisposition::Fresh
        );
        out.clone()
    };
    let first = render(&mut engine);
    assert_silenced_from_the_nan(&first, "routing tree");
    assert!(engine.take_output_clipped());

    // The workers render one block ahead, so the block already in flight still carries the
    // poisoned bus; the reset reaches them with the next dispatch.
    let _in_flight = render(&mut engine);
    let blocks: Vec<Vec<f32>> = (0..BLOCKS).map(|_| render(&mut engine)).collect();
    assert_recovered(&blocks, "routing tree");
    lifecycle.shutdown(runtime.retire());
}
