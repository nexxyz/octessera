use super::*;
use crate::synth::{InstrumentSlotConfig, InstrumentsConfig, SampleBankConfig, SampleBuffer};

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const BLOCKS: usize = 24;

/// A synth with an engaged lowpass, so a poisoned filter history feeds every later sample.
fn filtered_slot(kind: &str) -> InstrumentSlotConfig {
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 1_200.0;
    synth.filter.resonance = 20.0;
    InstrumentSlotConfig {
        fm: None,
        pluck: None,
        drum: None,
        kind: kind.into(),
        synth,
        mixer: None,
    }
}

fn engine_with(instruments: Vec<InstrumentSlotConfig>) -> SynthEngine {
    let mut engine = SynthEngine::new(RATE);
    engine.set_instruments(InstrumentsConfig {
        instruments,
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 110, 60_000);
    engine.note_on(1, 67, 110, 60_000);
    engine
}

fn two_synth_engine() -> SynthEngine {
    engine_with(vec![filtered_slot("synth"), filtered_slot("synth")])
}

fn render(engine: &mut SynthEngine) -> Vec<f32> {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
    out
}

fn poison_slot_filter(engine: &mut SynthEngine, slot: usize, value: f32) {
    let lane = engine
        .synth_voice_pool
        .slot_lanes(slot)
        .expect("slot lanes")[0];
    let voice = engine.synth_voice_pool.lane_mut(lane).expect("voice");
    assert!(voice.active);
    voice.filt.y1 = value;
    voice.filt.y2 = value;
}

fn slot_voice_active(engine: &SynthEngine, slot: usize) -> bool {
    engine
        .synth_voice_pool
        .slot_lanes(slot)
        .into_iter()
        .flatten()
        .any(|lane| {
            engine
                .synth_voice_pool
                .lane(*lane)
                .is_some_and(|voice| voice.active)
        })
}

/// Finite, still sounding (the other voice plays on) and no longer tripping the guard.
fn assert_recovered_and_sounding(blocks: &[Vec<f32>], clipped: &[bool], label: &str) {
    for (block, out) in blocks.iter().enumerate() {
        assert!(
            out.iter().all(|sample| sample.is_finite()),
            "{label} block {block}"
        );
        let energy: f32 = out.iter().map(|sample| sample * sample).sum();
        assert!(
            energy > 1.0e-3,
            "{label} block {block} went silent ({energy})"
        );
        assert!(
            !clipped[block],
            "{label} block {block} tripped the guard again"
        );
    }
}

#[test]
fn a_voice_with_a_non_finite_filter_ends_and_the_other_voice_keeps_sounding() {
    for poison in [f32::NAN, f32::INFINITY] {
        let mut engine = two_synth_engine();
        render(&mut engine);
        assert!(slot_voice_active(&engine, 0) && slot_voice_active(&engine, 1));
        poison_slot_filter(&mut engine, 0, poison);

        render(&mut engine);
        assert!(
            engine.take_output_clipped(),
            "{poison} did not trip the guard"
        );
        assert!(
            !slot_voice_active(&engine, 0),
            "{poison} voice kept playing"
        );
        assert!(
            slot_voice_active(&engine, 1),
            "{poison} took the healthy voice"
        );

        let mut clipped = Vec::new();
        let blocks: Vec<Vec<f32>> = (0..BLOCKS)
            .map(|_| {
                let out = render(&mut engine);
                clipped.push(engine.take_output_clipped());
                out
            })
            .collect();
        assert_recovered_and_sounding(&blocks, &clipped, &format!("inline {poison}"));
    }
}

#[test]
fn an_infinite_sample_at_the_master_recovers_like_a_nan() {
    let mut samples = vec![0.25_f32; RATE as usize];
    samples[64] = f32::INFINITY;
    let mut banks = vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT];
    banks[2].slots[0].buffer = Some(SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: RATE,
    });
    let mut engine = engine_with(vec![
        filtered_slot("synth"),
        filtered_slot("synth"),
        filtered_slot("sampler"),
    ]);
    drop(engine.set_sample_banks(banks));
    engine.note_on(2, 36, 110, 1_000);

    render(&mut engine);
    assert!(engine.take_output_clipped());
    let mut clipped = Vec::new();
    let blocks: Vec<Vec<f32>> = (0..BLOCKS)
        .map(|_| {
            let out = render(&mut engine);
            clipped.push(engine.take_output_clipped());
            out
        })
        .collect();
    assert_recovered_and_sounding(&blocks, &clipped, "master inf");
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_workers_end_a_voice_with_a_non_finite_filter() {
    for poison in [f32::NAN, f32::INFINITY] {
        let mut engine = two_synth_engine();
        render(&mut engine);
        poison_slot_filter(&mut engine, 0, poison);
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
            (out.clone(), engine.take_output_clipped())
        };
        // The poisoned block, then the block the workers had already rendered ahead.
        let _ = render(&mut engine);
        let _ = render(&mut engine);
        let _ = render(&mut engine);
        let (blocks, clipped): (Vec<_>, Vec<_>) = (0..BLOCKS).map(|_| render(&mut engine)).unzip();
        assert_recovered_and_sounding(&blocks, &clipped, &format!("routing tree {poison}"));
        lifecycle.shutdown(runtime.retire());
    }
}
