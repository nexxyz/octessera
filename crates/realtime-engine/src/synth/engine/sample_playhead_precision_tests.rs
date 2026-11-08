use super::*;
use crate::synth::{InstrumentSlotConfig, InstrumentsConfig, SampleBankConfig, SampleBuffer};

const RATE: u32 = 48_000;
const TONE_HZ: f64 = 1_000.0;

/// Rising zero crossings, placed between frames by linear interpolation.
fn rising_crossings(signal: &[f32]) -> Vec<f64> {
    signal
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
        .map(|(index, pair)| index as f64 + f64::from(pair[0]) / f64::from(pair[0] - pair[1]))
        .collect()
}

/// A minute-long sample played a semitone up must keep its pitch to the end. Past 2^21 frames
/// an f32 playhead rounds the step 1.0595 down to 1.0, a full semitone flat.
#[test]
fn a_long_sample_keeps_its_pitch_late_in_the_sample() {
    let frames = 61 * RATE as usize;
    let samples: Vec<f32> = (0..frames)
        .map(|frame| {
            (std::f64::consts::TAU * TONE_HZ * frame as f64 / f64::from(RATE)).sin() as f32
        })
        .collect();
    let mut bank = SampleBankConfig {
        tune_semis: 1.0,
        ..SampleBankConfig::default()
    };
    bank.slots[0].buffer = Some(SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: RATE,
    });
    let mut banks = vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT];
    banks[0] = bank;
    let mut engine = SynthEngine::new(RATE);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 50.0,
    });
    drop(engine.set_sample_banks(banks));
    engine.note_on(0, 36, 110, 60_000);

    let block = 512;
    let window_start = 42 * RATE as usize;
    let window_end = 43 * RATE as usize;
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut window = Vec::with_capacity(window_end - window_start);
    let mut rendered = 0;
    while rendered < window_end {
        engine.render_interleaved_block(block, &mut left, &mut right, &mut out);
        if rendered + block > window_start {
            window.extend_from_slice(&left);
        }
        rendered += block;
    }
    let lane = engine.active_sample_lane_indices_for_slot(0)[0];
    let voice = engine.sample_voice_pool.lane(lane).expect("sounding voice");
    let expected_step = 2.0_f64.powf(1.0 / 12.0);
    assert!(
        (voice.pos - rendered as f64 * expected_step).abs() < 1.0e-3,
        "playhead {} after {rendered} frames",
        voice.pos
    );

    let crossings = rising_crossings(&window);
    let periods = (crossings.len() - 1) as f64;
    let seconds = (crossings[crossings.len() - 1] - crossings[0]) / f64::from(RATE);
    let measured_hz = periods / seconds;
    let expected_hz = TONE_HZ * expected_step;
    assert!(
        ((measured_hz - expected_hz) / expected_hz).abs() < 1.0e-5,
        "measured {measured_hz} Hz, expected {expected_hz} Hz"
    );
}
