use super::*;
use crate::synth::{VoiceStealingMode, VOICES_PER_SLOT};

#[test]
fn maintains_eight_voices_per_instrument_slot() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..8 {
        engine.note_on(0, 60 + i, 100, 2_000);
        engine.note_on(1, 72 + i, 100, 2_000);
    }

    assert_eq!(engine.active_voice_count_for_slot(0), 8);
    assert_eq!(engine.active_voice_count_for_slot(1), 8);
}

#[test]
fn voice_steal_is_scoped_to_instrument_slot() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..8 {
        engine.note_on(0, 60 + i, 100, 2_000);
        engine.note_on(1, 72 + i, 100, 2_000);
    }
    engine.note_on(0, 90, 100, 2_000);

    assert_eq!(engine.active_voice_count_for_slot(0), 8);
    assert_eq!(engine.active_voice_count_for_slot(1), 8);
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
fn cc_updates_mod_slots_and_reset_cc_clears_them() {
    let mut engine = SynthEngine::new(48_000);
    engine.cc(0, 74, 127);
    engine.cc(0, 71, 64);
    let (cutoff, resonance) = engine.mod_values_for_slot(0);
    assert!(cutoff > 0.99);
    assert!(resonance > 0.49 && resonance < 0.51);

    engine.cc(0, 123, 0);
    let (cutoff_after, resonance_after) = engine.mod_values_for_slot(0);
    assert_eq!(cutoff_after, 0.0);
    assert_eq!(resonance_after, 0.0);
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
            engine.cc(slot, 120, 0);
        }
        for _ in 0..128 {
            let s = engine.next_sample();
            assert!(s.is_finite());
            assert!((-1.0..=1.0).contains(&s));
        }
    }
}

#[test]
fn aggressive_global_voice_budget_reduces_polyphony_under_load() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::Aggressive);
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

    assert!(active < INSTRUMENT_SLOT_COUNT * VOICES_PER_SLOT);
    assert!(status.voice_steal);
    assert!(status.ratio > 1.0);
    assert!(!engine.audio_load_status().voice_steal);
}

#[test]
fn disabled_global_voice_budget_preserves_full_polyphony() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_voice_stealing_mode(VoiceStealingMode::Off);
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

    assert_eq!(active, INSTRUMENT_SLOT_COUNT * VOICES_PER_SLOT);
}
