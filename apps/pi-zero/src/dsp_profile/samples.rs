use realtime_engine::synth::{
    SampleBankConfig, SampleBuffer, SampleSlotConfig, INSTRUMENT_SLOT_COUNT,
};

pub fn all_sample_banks(sample_rate: u32) -> Vec<SampleBankConfig> {
    (0..INSTRUMENT_SLOT_COUNT)
        .map(|_| sample_bank(sample_rate))
        .collect()
}

fn sample_bank(sample_rate: u32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: sample_buffer_data().into_boxed_slice().into(),
            channels: 1,
            sample_rate,
        }),
    };
    bank
}

fn sample_buffer_data() -> Vec<f32> {
    let frames = 16_384;
    (0..frames)
        .map(|i| ((i as f32 / 11.0).sin() * 0.2) + ((i as f32 / 37.0).cos() * 0.1))
        .collect()
}
