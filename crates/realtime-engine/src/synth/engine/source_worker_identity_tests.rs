use super::*;

fn load(ns_per_unit_ewma: [u64; 2]) -> SourceWorkerLoadSnapshot {
    SourceWorkerLoadSnapshot {
        quantum_ns: 1_000_000,
        ewma_coefficient_ppm: 1_000_000,
        busy_ns_ewma: [0, 0],
        ns_per_unit_ewma,
        observed_active_cost_units: [0, 0],
        has_useful_measurement: [true, true],
        utilization_ppm: None,
        observed: [true, true],
    }
}

fn sample_bank(seed: f32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![seed; 1024].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    bank
}

fn sample_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.set_sample_banks(vec![sample_bank(1.0); INSTRUMENT_SLOT_COUNT]);
    engine
}

fn active_synth_notes(engine: &SynthEngine, slot: usize) -> Vec<u8> {
    engine
        .synth_voice_pool
        .slot_lanes(slot)
        .unwrap_or(&[])
        .iter()
        .filter_map(|lane| engine.synth_voice_pool.lane(*lane))
        .filter(|voice| voice.active)
        .map(|voice| voice.midi_note)
        .collect()
}

#[test]
fn repeated_relocated_synth_steals_match_canonical_identity_for_each_stealing_mode() {
    for mode in [
        VoiceStealingMode::Fixed12,
        VoiceStealingMode::Fixed16,
        VoiceStealingMode::AutoSoft,
        VoiceStealingMode::AutoBalanced,
        VoiceStealingMode::AutoHard,
    ] {
        let mut canonical = SynthEngine::new(48_000);
        let mut relocated = SynthEngine::new(48_000);
        canonical.set_voice_stealing_mode(mode);
        relocated.set_voice_stealing_mode(mode);
        for note in 0..MAX_SYNTH_VOICES_PER_SLOT {
            canonical.note_on(0, 36 + note as u8, 100, 5_000);
            relocated.note_on(0, 36 + note as u8, 100, 5_000);
        }
        for admission in 0..32 {
            relocated.source_worker_load = Some(load(if admission % 2 == 0 {
                [100, 1]
            } else {
                [1, 100]
            }));
            let note = 90 + admission as u8;
            canonical.note_on(0, note, 100, 5_000);
            relocated.note_on(0, note, 100, 5_000);
            assert_eq!(
                relocated.active_synth_canonical_lane_indices_for_slot(0),
                canonical.active_synth_canonical_lane_indices_for_slot(0)
            );
            let mut canonical_notes = active_synth_notes(&canonical, 0);
            let mut relocated_notes = active_synth_notes(&relocated, 0);
            canonical_notes.sort_unstable();
            relocated_notes.sort_unstable();
            assert_eq!(relocated_notes, canonical_notes);
            assert_eq!(
                relocated.active_synth_lane_indices_for_slot(0),
                if admission % 2 == 0 {
                    vec![1, 2, 3, 4, 5, 6, 7, 9]
                } else {
                    (0..MAX_SYNTH_VOICES_PER_SLOT).collect()
                }
            );
        }
    }
}

#[test]
fn repeated_relocated_sample_steals_match_canonical_identity_for_each_stealing_mode() {
    for mode in [
        VoiceStealingMode::Fixed12,
        VoiceStealingMode::Fixed16,
        VoiceStealingMode::AutoSoft,
        VoiceStealingMode::AutoBalanced,
        VoiceStealingMode::AutoHard,
    ] {
        let mut canonical = sample_engine();
        let mut relocated = sample_engine();
        canonical.set_voice_stealing_mode(mode);
        relocated.set_voice_stealing_mode(mode);
        for _ in 0..MAX_SAMPLE_VOICES_PER_SLOT {
            canonical.note_on(0, 36, 100, 5_000);
            relocated.note_on(0, 36, 100, 5_000);
        }
        for admission in 0..32 {
            relocated.source_worker_load = Some(load(if admission % 2 == 0 {
                [100, 1]
            } else {
                [1, 100]
            }));
            canonical.note_on(0, 36, 100, 5_000);
            relocated.note_on(0, 36, 100, 5_000);
            assert_eq!(
                relocated.active_sample_canonical_lane_indices_for_slot(0),
                canonical.active_sample_canonical_lane_indices_for_slot(0)
            );
            assert_eq!(
                relocated.active_sample_lane_indices_for_slot(0),
                if admission % 2 == 0 {
                    vec![1, 2, 3, 4, 5, 6, 7, 9]
                } else {
                    (0..SAMPLE_VOICE_LANE_CAPACITY).take(8).collect()
                }
            );
        }
    }
}

#[test]
fn released_logical_identity_recycles_in_legacy_order() {
    let mut engine = SynthEngine::new(48_000);
    engine.source_worker_load = Some(load([100, 1]));
    for note in 0..4 {
        engine.note_on(0, 36 + note, 100, 5_000);
    }
    let released_lane = (0..SYNTH_VOICE_LANE_CAPACITY)
        .find(|lane| engine.synth_voice_pool.canonical_lane(*lane) == Some(1))
        .expect("canonical lane 1");
    assert!(engine.synth_voice_pool.deactivate_lane(released_lane));
    engine.synth_voice_pool.compact_slot_lanes(0);
    assert_eq!(engine.synth_voice_pool.first_free_canonical_lane(), Some(1));
    engine.note_on(0, 90, 100, 5_000);
    assert_eq!(
        engine.active_synth_canonical_lane_indices_for_slot(0),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn global_budget_steals_match_logical_identity_after_relocation() {
    let mut canonical = SynthEngine::new(48_000);
    let mut relocated = SynthEngine::new(48_000);
    canonical.set_voice_stealing_mode(VoiceStealingMode::AutoHard);
    relocated.set_voice_stealing_mode(VoiceStealingMode::AutoHard);
    for slot in 0..4 {
        for note in 0..MAX_SYNTH_VOICES_PER_SLOT {
            canonical.note_on(slot, 36 + note as u8, 100, 5_000);
            relocated.note_on(slot, 36 + note as u8, 100, 5_000);
        }
    }
    for admission in 0..8 {
        relocated.source_worker_load = Some(load(if admission % 2 == 0 {
            [100, 1]
        } else {
            [1, 100]
        }));
        canonical.note_on(0, 90 + admission, 100, 5_000);
        relocated.note_on(0, 90 + admission, 100, 5_000);
    }
    canonical.smoothed_load_ratio = 1.2;
    relocated.smoothed_load_ratio = 1.2;
    canonical.enforce_global_voice_budget();
    relocated.enforce_global_voice_budget();
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        assert_eq!(
            relocated.active_synth_canonical_lane_indices_for_slot(slot),
            canonical.active_synth_canonical_lane_indices_for_slot(slot)
        );
    }
}

#[test]
fn synth_release_returns_identity_without_changing_it_during_release() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 1);
    let lane = engine.synth_voice_pool.slot_lanes(0).expect("slot lanes")[0];
    assert_eq!(engine.synth_voice_pool.canonical_lane(lane), Some(0));
    engine.note_off(0, 60);
    assert_eq!(engine.synth_voice_pool.canonical_lane(lane), Some(0));
    for _ in 0..12_000 {
        engine.next_stereo_sample();
    }
    assert_eq!(engine.synth_voice_pool.canonical_lane(lane), None);
    assert_eq!(engine.synth_voice_pool.first_free_canonical_lane(), Some(0));
}

#[test]
fn sample_note_off_returns_logical_identity_immediately() {
    let mut engine = sample_engine();
    engine.source_worker_load = Some(load([100, 1]));
    engine.note_on(0, 36, 100, 5_000);
    let lane = engine.sample_voice_pool.slot_lanes(0).expect("slot lanes")[0];
    assert_eq!(engine.sample_voice_pool.canonical_lane(lane), Some(0));
    engine.note_off(0, 36);
    assert_eq!(engine.sample_voice_pool.canonical_lane(lane), None);
    assert_eq!(
        engine.sample_voice_pool.first_free_canonical_lane(),
        Some(0)
    );
}

#[test]
fn none_mode_rejects_without_mutating_canonical_identity() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
        engine.note_on(0, ((36 + lane) % 128) as u8, 100, 5_000);
    }
    let before = engine.active_synth_canonical_lane_indices_for_slot(0);
    engine.source_worker_load = Some(load([1, 100]));
    engine.note_on(0, 127, 1, 5_000);
    assert_eq!(
        engine.active_synth_canonical_lane_indices_for_slot(0),
        before
    );
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_admission_drops,
        1
    );
}
