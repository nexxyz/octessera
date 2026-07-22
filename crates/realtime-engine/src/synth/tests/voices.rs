use super::*;
use crate::synth::{
    MomentaryFxTarget, VoiceStealingMode, MAX_SAMPLE_VOICES_PER_SLOT, MAX_SYNTH_VOICES,
    MAX_SYNTH_VOICES_PER_SLOT, VOICES_PER_SLOT,
};
use serde_json::json;
use std::collections::BTreeMap;

mod profile_snapshot;

#[test]
fn synth_voice_cap_limits_per_slot_to_eight() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..MAX_SYNTH_VOICES_PER_SLOT {
        engine.note_on(0, (60 + i) as u8, 100, 2_000);
    }

    assert_eq!(
        engine.active_voice_count_for_slot(0),
        MAX_SYNTH_VOICES_PER_SLOT
    );
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 0);
}

#[test]
fn voice_steal_is_scoped_to_instrument_slot() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    for i in 0..MAX_SYNTH_VOICES_PER_SLOT {
        engine.note_on(0, (60 + i) as u8, 100, 2_000);
        engine.note_on(1, (72 + i) as u8, 100, 2_000);
    }
    engine.note_on(0, 90, 100, 2_000);

    assert_eq!(
        engine.active_voice_count_for_slot(0),
        MAX_SYNTH_VOICES_PER_SLOT
    );
    assert_eq!(
        engine.active_voice_count_for_slot(1),
        MAX_SYNTH_VOICES_PER_SLOT
    );
}

#[test]
fn note_off_releases_matching_slot_note() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 50_000);
    for _ in 0..64 {
        let _ = engine.next_sample();
    }
    engine.note_off(0, 60);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn all_notes_off_releases_all_slots() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..4 {
        engine.note_on(0, 60 + i, 100, 50_000);
        engine.note_on(1, 72 + i, 100, 50_000);
    }
    engine.all_notes_off();
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(1), 0);
}

#[test]
fn all_notes_off_clears_synth_sample_and_preview_voices() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(
        1,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.set_sample_banks(vec![sample_bank(vec![1.0; 16_384]); INSTRUMENT_SLOT_COUNT]);
    engine.note_on(0, 60, 100, 50_000);
    engine.note_on(1, 36, 100, 50_000);
    engine.preview_sample(
        1,
        SampleBuffer {
            samples: vec![0.25; 256].into_boxed_slice().into(),
            channels: 1,
            sample_rate: 48_000,
        },
        100,
    );

    assert_eq!(engine.profile_snapshot().active_synth_voices, 1);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 1);
    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 1);

    engine.all_notes_off();
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }

    let snapshot = engine.profile_snapshot();
    assert_eq!(snapshot.active_synth_voices, 0);
    assert_eq!(snapshot.active_sample_voices, 0);
    assert_eq!(snapshot.active_preview_sample_voices, 0);
}

#[test]
fn cc_updates_mod_slots_without_all_notes_off_semantics() {
    let mut engine = SynthEngine::new(48_000);
    engine.cc(0, 74, 127);
    engine.cc(0, 71, 64);
    let (cutoff, resonance) = engine.mod_values_for_slot(0);
    assert!(cutoff > 0.99);
    assert!(resonance > 0.49 && resonance < 0.51);

    engine.note_on(0, 60, 100, 50_000);
    engine.cc(0, 74, 0);
    assert_eq!(engine.active_voice_count_for_slot(0), 1);
}

#[test]
fn note_on_clamps_slot_and_velocity() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(200, 60, 0, 1_000);
    assert_eq!(
        engine.active_voice_count_for_slot(INSTRUMENT_SLOT_COUNT - 1),
        1
    );
    for _ in 0..100 {
        let s = engine.next_sample();
        assert!(s.is_finite());
    }
}

#[test]
fn zero_duration_note_releases_after_minimum_samples() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 0);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn long_running_event_stream_stays_finite() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..200 {
        let slot = (i % INSTRUMENT_SLOT_COUNT) as u8;
        let note = 36 + (i % 48) as u8;
        let vel = 1 + (i % 127) as u8;
        engine.note_on(slot, note, vel, 50 + (i % 200) as u32);
        engine.cc(slot, 74, (i % 128) as u8);
        engine.cc(slot, 71, ((i * 3) % 128) as u8);
        if i % 11 == 0 {
            engine.cc(slot, 74, 0);
        }
        for _ in 0..128 {
            let s = engine.next_sample();
            assert!(s.is_finite());
            assert!((-1.0..=1.0).contains(&s));
        }
    }
}

#[test]
fn auto_hard_global_voice_budget_reduces_polyphony_under_load() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::AutoHard);
    for _ in 0..20 {
        engine.set_runtime_load_ratio(2.0);
    }

    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in 0..VOICES_PER_SLOT {
            engine.note_on(slot as u8, 36 + note as u8, 100, 5_000);
        }
    }

    let active: usize = (0..INSTRUMENT_SLOT_COUNT)
        .map(|slot| engine.active_voice_count_for_slot(slot))
        .sum();
    let status = engine.audio_load_status();

    assert!(active <= 29);
    assert!(active > MAX_SYNTH_VOICES);
    assert!(status.voice_steal);
    assert!(status.ratio > 1.0);
    assert!(!engine.audio_load_status().voice_steal);
}

#[test]
fn disabled_global_voice_budget_preserves_per_slot_cap() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    for _ in 0..20 {
        engine.set_runtime_load_ratio(2.0);
    }

    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in 0..VOICES_PER_SLOT {
            engine.note_on(slot as u8, 48 + note as u8, 100, 5_000);
        }
    }

    let active: usize = (0..INSTRUMENT_SLOT_COUNT)
        .map(|slot| engine.active_voice_count_for_slot(slot))
        .sum();

    assert_eq!(active, INSTRUMENT_SLOT_COUNT * MAX_SYNTH_VOICES_PER_SLOT);
}

#[test]
fn fair_global_voice_stealing_balances_two_hot_slots() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::Fixed12);
    for _ in 0..20 {
        engine.set_runtime_load_ratio(2.0);
    }

    for note in 0..8 {
        engine.note_on(0, (60 + note) as u8, 100, 5_000);
        engine.note_on(1, (72 + note) as u8, 100, 5_000);
    }

    assert_eq!(engine.active_voice_count_for_slot(0), 6);
    assert_eq!(engine.active_voice_count_for_slot(1), 6);
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 4);
}

#[test]
fn overload_across_many_slots_preserves_one_voice_per_slot() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::Fixed12);
    for _ in 0..20 {
        engine.set_runtime_load_ratio(2.0);
    }

    for slot in 0..8 {
        for note in 0..2 {
            engine.note_on(slot as u8, (48 + note) as u8, 100, 5_000);
        }
    }

    let active_per_slot: Vec<_> = (0..8)
        .map(|slot| engine.active_voice_count_for_slot(slot))
        .collect();
    assert!(active_per_slot.iter().all(|count| *count >= 1));
    assert_eq!(active_per_slot.iter().sum::<usize>(), 12);
}

#[test]
fn sample_voice_cap_limits_per_slot_to_eight() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".into(),
                pan_pos: 0,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(vec![sample_bank(vec![1.0; 16_384]); INSTRUMENT_SLOT_COUNT]);

    for _ in 0..(MAX_SAMPLE_VOICES_PER_SLOT + 1) {
        engine.note_on(0, 36, 100, 2_000);
    }

    assert_eq!(
        engine.profile_snapshot().active_sample_voices,
        MAX_SAMPLE_VOICES_PER_SLOT
    );
    assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
}
