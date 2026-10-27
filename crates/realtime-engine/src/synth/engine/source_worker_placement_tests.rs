use super::*;
use std::sync::Arc;

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
            fm: None,
            pluck: None,
            drum: None,
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
fn equal_projected_load_alternates_new_synth_voices() {
    let mut engine = SynthEngine::new(48_000);
    engine.source_worker_load = Some(load([10, 10]));
    for note in 0..4 {
        engine.note_on(0, 36 + note, 100, 5_000);
    }
    assert_eq!(
        engine.active_synth_lane_indices_for_slot(0),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn mixed_costs_follow_measured_worker_skew_and_existing_lanes_do_not_migrate() {
    let mut engine = sample_engine();
    engine.source_worker_load = Some(load([100, 1]));
    engine.note_on(0, 36, 100, 5_000);
    engine.note_on(1, 60, 100, 5_000);
    assert_eq!(engine.active_sample_lane_indices_for_slot(0), vec![1]);
    assert_eq!(engine.active_synth_lane_indices_for_slot(1), vec![1]);

    let before = engine.active_synth_lane_indices_for_slot(1);
    engine.source_worker_load = Some(load([1, 100]));
    engine.note_on(1, 61, 100, 5_000);
    assert_eq!(engine.active_synth_lane_indices_for_slot(1), vec![0, 1]);
    engine.source_worker_load = Some(load([100, 1]));
    assert_eq!(engine.active_sample_lane_indices_for_slot(0), vec![1]);
    assert_eq!(before, vec![1]);
}

#[test]
fn a_full_partition_forces_the_other_worker_without_rejecting() {
    let mut engine = SynthEngine::new(48_000);
    engine.source_worker_load = Some(load([1, 1_000]));
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in 0..(SYNTH_VOICE_LANE_CAPACITY / INSTRUMENT_SLOT_COUNT) {
            engine.note_on(slot as u8, 36 + note as u8, 100, 5_000);
        }
    }
    let mut parity_counts = [0, 0];
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for lane in engine.active_synth_lane_indices_for_slot(slot) {
            parity_counts[lane % 2] += 1;
        }
    }
    assert_eq!(
        parity_counts,
        [SYNTH_VOICE_LANE_CAPACITY / 2, SYNTH_VOICE_LANE_CAPACITY / 2]
    );
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_admission_drops,
        0
    );
}

#[test]
fn canonical_per_slot_victim_is_unchanged_across_stealing_modes() {
    for mode in [
        VoiceStealingMode::Fixed12,
        VoiceStealingMode::Fixed16,
        VoiceStealingMode::AutoSoft,
        VoiceStealingMode::AutoBalanced,
        VoiceStealingMode::AutoHard,
    ] {
        let mut canonical = SynthEngine::new(48_000);
        let mut placed = SynthEngine::new(48_000);
        canonical.set_voice_stealing_mode(mode);
        placed.set_voice_stealing_mode(mode);
        for note in 0..MAX_SYNTH_VOICES_PER_SLOT {
            canonical.note_on(0, 36 + note as u8, 100, 5_000);
            placed.note_on(0, 36 + note as u8, 100, 5_000);
        }
        placed.source_worker_load = Some(load([100, 1]));
        canonical.note_on(0, 90, 100, 5_000);
        placed.note_on(0, 90, 100, 5_000);
        let mut canonical_notes = active_synth_notes(&canonical, 0);
        let mut placed_notes = active_synth_notes(&placed, 0);
        canonical_notes.sort_unstable();
        placed_notes.sort_unstable();
        assert_eq!(canonical_notes, placed_notes);
        assert_eq!(canonical.profile_snapshot().cumulative_voice_steals, 1);
        assert_eq!(placed.profile_snapshot().cumulative_voice_steals, 1);
    }
}

#[test]
fn cross_worker_synth_steal_fades_the_victim_in_its_lane_and_is_one_steal() {
    let mut engine = SynthEngine::new(48_000);
    for note in 0..MAX_SYNTH_VOICES_PER_SLOT {
        engine.note_on(0, 36 + note as u8, 100, 5_000);
    }
    engine.source_worker_load = Some(load([100, 1]));
    engine.note_on(0, 90, 100, 5_000);
    assert_eq!(
        engine.active_synth_lane_indices_for_slot(0),
        [0, 1, 2, 3, 4, 5, 6, 7, 9]
    );
    assert!(engine
        .synth_voice_pool
        .lane(0)
        .is_some_and(|victim| victim.declick.is_fading()));
    engine.synth_voice_pool.assert_invariants();
    engine.render_through_steal_fades();
    assert_eq!(
        engine.active_synth_lane_indices_for_slot(0),
        [1, 2, 3, 4, 5, 6, 7, 9]
    );
    engine.synth_voice_pool.assert_invariants();
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
}

#[test]
fn prepared_many_note_render_preserves_synth_ownership_and_audio() {
    let mut engine = SynthEngine::new(48_000);
    let prepared = prepare_audio_config(
        InstrumentsConfig {
            instruments: (0..INSTRUMENT_SLOT_COUNT)
                .map(|_| InstrumentSlotConfig {
                    fm: None,
                    pluck: None,
                    drum: None,
                    kind: "synth".into(),
                    synth: default_synth_config(),
                    mixer: None,
                })
                .collect(),
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        None,
        None,
        48_000,
    );
    let _ = engine.apply_prepared_audio_config(prepared);
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in 0..MAX_SYNTH_VOICES_PER_SLOT {
            engine.note_on(slot as u8, 48 + note as u8, 96, 5_000);
        }
    }
    engine.source_worker_load = Some(load([100, 1]));
    engine.note_on(0, 90, 96, 5_000);

    let snapshot = engine.profile_snapshot();
    assert_eq!(
        snapshot.active_synth_voices,
        INSTRUMENT_SLOT_COUNT * MAX_SYNTH_VOICES_PER_SLOT
    );
    assert_eq!(snapshot.cumulative_voice_steals, 1);
    engine.synth_voice_pool.assert_invariants();

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
    assert!(out.iter().any(|sample| sample.abs() > f32::EPSILON));
    engine.synth_voice_pool.assert_invariants();
}

#[test]
fn none_mode_still_drops_at_full_capacity_with_load_state() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    engine.source_worker_load = Some(load([1, 1]));
    for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
        engine.note_on(0, ((36 + lane) % 128) as u8, 100, 5_000);
    }
    let before = engine.active_synth_lane_indices_for_slot(0);
    engine.note_on(0, 127, 1, 5_000);
    assert_eq!(engine.active_synth_lane_indices_for_slot(0), before);
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_admission_drops,
        1
    );
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 0);
}

#[test]
fn cross_worker_sample_steal_retires_the_parked_arc_and_fades_the_victim_without_callback_drop() {
    let mut engine = sample_engine();
    for _ in 0..MAX_SAMPLE_VOICES_PER_SLOT {
        engine.note_on(0, 36, 100, 5_000);
    }
    let target_samples: Arc<[f32]> = vec![0.5; 16].into();
    {
        let target = engine.sample_voice_pool.lane_mut(9).expect("lane 9");
        target.buffer = Some(SampleBuffer {
            samples: Arc::clone(&target_samples),
            channels: 1,
            sample_rate: 48_000,
        });
        target.sample_slot = 0;
        target.instrument_slot = 0;
        target.active = false;
    }
    let victim_samples = engine
        .sample_voice_pool
        .lane(0)
        .expect("victim lane")
        .buffer
        .as_ref()
        .expect("victim buffer")
        .samples
        .clone();
    engine.source_worker_load = Some(load([100, 1]));
    let (_, _, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.note_on(0, 36, 100, 5_000);
        });
    assert_eq!(deallocations, 0);
    assert_eq!(
        engine.active_sample_lane_indices_for_slot(0),
        [0, 1, 2, 3, 4, 5, 6, 7, 9]
    );
    assert_eq!(engine.pending_render_retired.sample_voices.len(), 1);
    assert!(Arc::ptr_eq(
        engine
            .pending_render_retired
            .sample_voices
            .get(0)
            .expect("target retirement"),
        &target_samples,
    ));
    let victim = engine.sample_voice_pool.lane(0).expect("victim lane");
    assert!(victim.declick.is_fading());
    assert!(Arc::ptr_eq(
        &victim.buffer.as_ref().expect("victim buffer").samples,
        &victim_samples,
    ));
    engine.sample_voice_pool.assert_invariants();
    engine.render_through_steal_fades();
    assert_eq!(
        engine.active_sample_lane_indices_for_slot(0),
        [1, 2, 3, 4, 5, 6, 7, 9]
    );
    engine.sample_voice_pool.assert_invariants();
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
}

#[test]
fn sample_cross_worker_steal_preflights_its_retirement_addition() {
    let mut engine = sample_engine();
    for _ in 0..MAX_SAMPLE_VOICES_PER_SLOT {
        engine.note_on(0, 36, 100, 5_000);
    }
    {
        let target = engine.sample_voice_pool.lane_mut(9).expect("lane 9");
        target.buffer = Some(sample_bank(0.5).slots[0].buffer.clone().expect("buffer"));
        target.sample_slot = 0;
        target.instrument_slot = 0;
    }
    for index in 0..SAMPLE_VOICE_RETIREMENT_CAPACITY {
        let mut voice = SampleVoice::off();
        voice.buffer = Some(
            sample_bank(index as f32).slots[0]
                .buffer
                .clone()
                .expect("buffer"),
        );
        assert!(engine.pending_render_retired.sample_voices.push(&mut voice));
    }
    let before_lanes = engine.active_sample_lane_indices_for_slot(0);
    let before_victim = engine
        .sample_voice_pool
        .lane(0)
        .expect("victim lane")
        .buffer
        .is_some();
    engine.source_worker_load = Some(load([100, 1]));
    engine.note_on(0, 36, 100, 5_000);
    assert_eq!(engine.active_sample_lane_indices_for_slot(0), before_lanes);
    assert_eq!(
        engine
            .sample_voice_pool
            .lane(0)
            .expect("victim lane")
            .buffer
            .is_some(),
        before_victim
    );
    assert!(!engine
        .sample_voice_pool
        .lane(0)
        .is_some_and(|victim| victim.declick.is_fading()));
    assert_eq!(
        engine.pending_render_retired.sample_voices.len(),
        SAMPLE_VOICE_RETIREMENT_CAPACITY
    );
    engine.sample_voice_pool.assert_invariants();
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_admission_drops,
        1
    );
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 0);
}
