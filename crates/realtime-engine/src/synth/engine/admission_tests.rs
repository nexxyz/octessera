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
            fm: None,
            pluck: None,
            drum: None,
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
        engine.note_on(0, ((36 + lane) % 128) as u8, 100, 50_000);
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
            fm: None,
            pluck: None,
            drum: None,
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

fn steady_level_across_a_steal(kind: &str, block: bool) -> (f32, f32) {
    let mut engine = SynthEngine::new(48_000);
    let mut synth = default_synth_config();
    synth.amp_env.attack_ms = 0.0;
    synth.amp_env.decay_ms = 0.0;
    synth.amp_env.sustain_pct = 100.0;
    synth.osc1.waveform = WaveformId::Sine;
    synth.osc2.level_pct = 0.0;
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: kind.into(),
            synth,
            mixer: None,
        },
    );
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![0.1; 48_000].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    engine.set_sample_banks(vec![bank; INSTRUMENT_SLOT_COUNT]);
    let voices = if kind == "sampler" {
        MAX_SAMPLE_VOICES_PER_SLOT
    } else {
        MAX_SYNTH_VOICES_PER_SLOT
    };
    for _ in 0..voices {
        engine.note_on(0, 0, 100, 50_000);
    }
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut render = |engine: &mut SynthEngine, frames: usize| {
        if block {
            engine.render_interleaved_block(frames, &mut left, &mut right, &mut out);
            *left.last().unwrap()
        } else {
            (0..frames).fold(0.0, |_, _| engine.next_stereo_sample().0)
        }
    };
    let before = render(&mut engine, 4_800);
    engine.note_on(0, 0, 100, 50_000);
    let after = render(&mut engine, 1);
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
    engine.assert_voice_pool_invariants();
    (before, after)
}

#[test]
fn stolen_lanes_ramp_from_the_old_voice_instead_of_stepping() {
    for kind in ["sampler", "synth"] {
        for block in [false, true] {
            let (before, after) = steady_level_across_a_steal(kind, block);
            let one_voice = before / 8.0;
            assert!(
                before.abs() > 0.05,
                "{kind} block={block}: steady level {before}"
            );
            // A new sample starts at full level, a new low sine at zero; neither may lose
            // the stolen voice's share in one step.
            let kept = if kind == "sampler" {
                after > before - one_voice * 0.1
            } else {
                (after - before).abs() < one_voice.abs() * 0.1
            };
            assert!(
                kept,
                "{kind} block={block}: stolen share dropped out: {before} -> {after}"
            );
        }
    }
}
