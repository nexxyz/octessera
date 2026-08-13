use realtime_engine::synth::{
    SampleBankConfig, SampleBuffer, SampleSlotConfig, INSTRUMENT_SLOT_COUNT,
};
use std::sync::Arc;

pub fn all_sample_banks(sample_rate: u32) -> Vec<SampleBankConfig> {
    (0..INSTRUMENT_SLOT_COUNT)
        .map(|_| sample_bank(sample_rate, sample_buffer_data().into_boxed_slice().into()))
        .collect()
}

#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub fn long_sample_banks(sample_rate: u32, duration_seconds: u32) -> Vec<SampleBankConfig> {
    let frames = sample_rate as usize * duration_seconds as usize + sample_rate as usize;
    let samples: Arc<[f32]> = sample_buffer_data_for_frames(frames)
        .into_boxed_slice()
        .into();
    (0..INSTRUMENT_SLOT_COUNT)
        .map(|_| sample_bank(sample_rate, samples.clone()))
        .collect()
}

fn sample_bank(sample_rate: u32, samples: Arc<[f32]>) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples,
            channels: 1,
            sample_rate,
        }),
    };
    bank
}

fn sample_buffer_data() -> Vec<f32> {
    sample_buffer_data_for_frames(16_384)
}

fn sample_buffer_data_for_frames(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| ((i as f32 / 11.0).sin() * 0.2) + ((i as f32 / 37.0).cos() * 0.1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::long_sample_banks;
    use std::sync::Arc;

    #[test]
    fn long_sample_banks_share_one_backing_arc() {
        let banks = long_sample_banks(44_100, 125);
        let first = banks[0].slots[0].buffer.as_ref().unwrap();
        let second = banks[1].slots[0].buffer.as_ref().unwrap();
        assert!(Arc::ptr_eq(&first.samples, &second.samples));
        assert!(first.samples.len() >= 44_100 * 125);
    }
}
