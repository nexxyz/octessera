use super::bus_chain_owner::BusChainOwner;
use super::*;
use crate::synth::fx::fx_bus_state_from_params;
use crate::synth::fx_param::FxParamId;
use crate::synth::fx_param_glide::fx_param_glide_frames;
use crate::synth::SynthParamId;
use std::hint::black_box;

const BLOCK: usize = 128;
const SLOTS: usize = 4;

fn voice_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    let mut synth = default_synth_config();
    synth.filter.env_amount_pct = 0.0;
    engine.set_instruments(InstrumentsConfig {
        instruments: (0..SLOTS)
            .map(|_| InstrumentSlotConfig {
                fm: None,
                pluck: None,
                drum: None,
                kind: "synth".into(),
                synth,
                mixer: None,
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    for slot in 0..SLOTS {
        for note in [48, 55, 60, 67] {
            engine.note_on(slot as u8, note, 100, 600_000);
        }
    }
    engine
}

/// With `ramping`, every block moves the cutoff of all four slots, so a glide is always running.
fn voice_block_cost(ramping: bool) -> f64 {
    const BLOCKS: usize = 4_000;
    let mut engine = voice_engine();
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
    let started = std::time::Instant::now();
    for block in 0..BLOCKS {
        if ramping {
            let cutoff = if block.is_multiple_of(2) {
                900.0
            } else {
                1_400.0
            };
            for slot in 0..SLOTS {
                engine.set_synth_param_typed(slot, SynthParamId::FilterCutoffHz, cutoff);
            }
        }
        engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
        black_box(&out);
    }
    started.elapsed().as_nanos() as f64 / BLOCKS as f64
}

/// Moves Osc 1 level of all four slots every `period` blocks, so with a 10 ms glide a period of 2
/// keeps a level glide running on every block and a period of 8 on about half of them.
fn level_block_cost(period: usize) -> f64 {
    const BLOCKS: usize = 4_000;
    let mut engine = voice_engine();
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
    let started = std::time::Instant::now();
    for block in 0..BLOCKS {
        if period > 0 && block.is_multiple_of(period) {
            let level = if (block / period).is_multiple_of(2) {
                40.0
            } else {
                80.0
            };
            for slot in 0..SLOTS {
                engine.set_synth_param_typed(slot, SynthParamId::Osc1LevelPct, level);
            }
        }
        engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
        black_box(&out);
    }
    started.elapsed().as_nanos() as f64 / BLOCKS as f64
}

/// Prints what a 128-frame block of 16 held synth voices costs settled, with a level glide on
/// every block and with one on about half of the blocks; run with
/// `cargo test --release -p realtime-engine --lib live_level_glide_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn live_level_glide_cost() {
    for _ in 0..5 {
        println!(
            "level: settled {:.0} ns, ramping 100% {:.0} ns, ramping 50% {:.0} ns per block",
            level_block_cost(0),
            level_block_cost(2),
            level_block_cost(8),
        );
    }
}

fn bus_chain() -> BusChainOwner {
    let params = [
        compile_fx_bus_params(&FxBusSlotConfig::Kind("delay".into())),
        compile_fx_bus_params(&FxBusSlotConfig::Kind("reverb".into())),
        compile_fx_bus_params(&FxBusSlotConfig::Kind("eq".into())),
    ];
    let state = params.map(|params| fx_bus_state_from_params(&params, 48_000));
    BusChainOwner::new(0, params, state, [2, 3, 3])
}

/// With `ramping`, every 128 frames moves the Delay and Reverb mix and the EQ mid gain.
fn bus_frame_cost(ramping: bool) -> f64 {
    const FRAMES: usize = 2_000_000;
    let mut chain = bus_chain();
    let duck = [0.0; BUS_SLOTS_PER_BUS];
    let mut sum = 0.0_f32;
    let started = std::time::Instant::now();
    for frame in 0..FRAMES {
        if ramping && frame % BLOCK == 0 {
            let up = (frame / BLOCK).is_multiple_of(2);
            let glide = fx_param_glide_frames(48_000);
            chain.set_slot_param(0, FxParamId::MixPct, if up { 60.0 } else { 30.0 }, glide);
            chain.set_slot_param(1, FxParamId::MixPct, if up { 40.0 } else { 20.0 }, glide);
            chain.set_slot_param(2, FxParamId::MidGainDb, if up { 6.0 } else { -6.0 }, glide);
        }
        let input = black_box((frame % 97) as f32 * 0.01 - 0.48);
        sum += chain.process(input, &duck, 48_000).mono;
    }
    black_box(sum);
    started.elapsed().as_nanos() as f64 / FRAMES as f64
}

/// Prints what a 128-frame block of 16 held synth voices and one frame of a Delay, Reverb and
/// EQ bus chain cost, settled and while a live parameter glides; run with
/// `cargo test --release -p realtime-engine --lib live_param_glide_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn live_param_glide_cost() {
    for _ in 0..2 {
        println!(
            "voices: settled {:.0} ns, ramping {:.0} ns per block; bus chain: settled {:.2} ns, ramping {:.2} ns per frame",
            voice_block_cost(false),
            voice_block_cost(true),
            bus_frame_cost(false),
            bus_frame_cost(true),
        );
    }
}
