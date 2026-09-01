use super::*;

fn synth_slot_state(engine: &SynthEngine, slot: usize) -> Vec<(usize, String)> {
    engine
        .synth_voice_pool
        .slot_lanes(slot)
        .unwrap()
        .iter()
        .copied()
        .map(|lane| (lane, format!("{:?}", engine.synth_voice_pool.lane(lane))))
        .collect()
}

fn sample_slot_state(engine: &SynthEngine, slot: usize) -> Vec<(usize, String)> {
    engine
        .sample_voice_pool
        .slot_lanes(slot)
        .unwrap()
        .iter()
        .copied()
        .map(|lane| (lane, format!("{:?}", engine.sample_voice_pool.lane(lane))))
        .collect()
}

fn sample_bank() -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![1.0; 1024].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    bank
}

#[test]
fn none_allows_ninth_same_slot_synth_admission_without_stealing() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);

    for note in 0..9 {
        engine.note_on(0, 36 + note, 100, 50_000);
    }

    let snapshot = engine.profile_snapshot();
    assert_eq!(snapshot.active_synth_voices, 9);
    assert_eq!(snapshot.cumulative_voice_steals, 0);
    assert_eq!(snapshot.cumulative_voice_admission_drops, 0);
    engine.assert_voice_pool_invariants();
}

#[test]
fn none_allows_ninth_same_slot_sample_admission_without_stealing() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.set_sample_banks(vec![sample_bank(); INSTRUMENT_SLOT_COUNT]);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);

    for _ in 0..9 {
        engine.note_on(0, 36, 100, 50_000);
    }

    let snapshot = engine.profile_snapshot();
    assert_eq!(snapshot.active_sample_voices, 9);
    assert_eq!(snapshot.cumulative_voice_steals, 0);
    assert_eq!(snapshot.cumulative_voice_admission_drops, 0);
    engine.assert_voice_pool_invariants();
}

#[test]
fn none_rejects_synth_admission_at_physical_capacity_without_mutation() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
        engine.note_on(0, (36 + lane as u8) % 128, 100, 50_000);
    }
    let before = synth_slot_state(&engine, 0);
    let before_steals = engine.profile_snapshot().cumulative_voice_steals;

    engine.note_on(0, 127, 1, 50_000);

    let snapshot = engine.profile_snapshot();
    assert_eq!(synth_slot_state(&engine, 0), before);
    assert_eq!(snapshot.active_synth_voices, SYNTH_VOICE_LANE_CAPACITY);
    assert_eq!(snapshot.cumulative_voice_steals, before_steals);
    assert_eq!(snapshot.cumulative_voice_admission_drops, 1);
    engine.assert_voice_pool_invariants();

    engine.cumulative_voice_admission_drops = u64::MAX;
    engine.note_on(0, 126, 1, 50_000);
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_admission_drops,
        u64::MAX
    );
}

#[test]
fn none_rejects_sample_admission_at_physical_capacity_without_mutation() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.set_sample_banks(vec![sample_bank(); INSTRUMENT_SLOT_COUNT]);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    for _ in 0..SAMPLE_VOICE_LANE_CAPACITY {
        engine.note_on(0, 36, 100, 50_000);
    }
    let before = sample_slot_state(&engine, 0);
    let before_steals = engine.profile_snapshot().cumulative_voice_steals;

    engine.note_on(0, 36, 1, 50_000);

    let snapshot = engine.profile_snapshot();
    assert_eq!(sample_slot_state(&engine, 0), before);
    assert_eq!(snapshot.active_sample_voices, SAMPLE_VOICE_LANE_CAPACITY);
    assert_eq!(snapshot.cumulative_voice_steals, before_steals);
    assert_eq!(snapshot.cumulative_voice_admission_drops, 1);
    engine.assert_voice_pool_invariants();

    engine.cumulative_voice_admission_drops = u64::MAX;
    engine.note_on(0, 36, 1, 50_000);
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_admission_drops,
        u64::MAX
    );
}
