use super::*;

fn sample_bank() -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![1.0; 16_384].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    bank
}

#[test]
fn deterministic_voice_pool_stress_preserves_invariants() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(
        1,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.set_sample_banks(vec![sample_bank(); INSTRUMENT_SLOT_COUNT]);
    engine.set_voice_stealing_mode(VoiceStealingMode::Fixed12);

    for round in 0..24 {
        for step in 0..24 {
            for slot in [0, 2, 3] {
                engine.note_on(slot, 36 + ((round + step) % 48) as u8, 100, 5_000);
            }
            engine.note_on(1, 36, 100, 5_000);
        }
        for slot in [0, 1, 2, 3] {
            engine.note_off(slot, 36 + (round % 48) as u8);
        }
        for _ in 0..128 {
            let _ = engine.next_sample();
        }
        engine.assert_voice_pool_invariants();
    }

    engine.set_sample_bank(1, sample_bank());
    engine.assert_voice_pool_invariants();
    engine.all_notes_off();
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    engine.assert_voice_pool_invariants();

    engine.note_on(0, 60, 100, 5_000);
    engine.note_on(1, 36, 100, 5_000);
    for _ in 0..128 {
        let _ = engine.next_sample();
    }
    engine.assert_voice_pool_invariants();
}
