use realtime_engine::synth::{SampleBankConfig, SampleBuffer, SampleSlotConfig};
use std::sync::Arc;

const SAMPLE_RATE: u32 = 32_000;
const CHANNELS: u16 = 2;
const SECONDS: usize = 22;

pub(crate) fn sample_bank() -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(sample_buffer()),
    };
    bank.gain_pct = 100.0;
    bank.velocity_sensitivity_pct = 0.0;
    bank
}

pub(crate) fn sample_buffer() -> SampleBuffer {
    SampleBuffer {
        samples: generated_samples(),
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
    }
}

fn generated_samples() -> Arc<[f32]> {
    let frames = SAMPLE_RATE as usize * SECONDS;
    let mut samples = Vec::with_capacity(frames * CHANNELS as usize);
    for frame in 0..frames {
        let t = frame as f32 / SAMPLE_RATE as f32;
        let env = (1.0 - t / SECONDS as f32).max(0.2);
        let left = ((t * 220.0 * std::f32::consts::TAU).sin() * 0.35
            + (t * 330.0 * std::f32::consts::TAU).sin() * 0.15)
            * env;
        let right = ((t * 221.5 * std::f32::consts::TAU).sin() * 0.32
            + (t * 440.0 * std::f32::consts::TAU).sin() * 0.12)
            * env;
        samples.push(left);
        samples.push(right);
    }
    Arc::from(samples.into_boxed_slice())
}
