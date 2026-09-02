use super::*;

use super::source_worker_test_fixtures::{
    assert_worker_matches_inline, dynamic_engine, full_mixed_engine,
};

const SUPPORTED_QUANTA: [usize; 5] = [32, 64, 128, 256, 2048];
const TEST_DEADLINE: std::time::Duration = std::time::Duration::from_secs(1);

#[test]
fn persistent_workers_match_inline_at_supported_quanta_with_transitions() {
    for frames in SUPPORTED_QUANTA {
        let mut worker = dynamic_engine();
        let mut inline = dynamic_engine();
        for engine in [&mut worker, &mut inline] {
            engine.note_on(0, 36, 111, 5_000);
            engine.note_on(1, 60, 97, 5_000);
            engine.set_sample_bank_param(0, "sample.filter.cutoffHz", 1_700.0);
            engine.set_sample_bank_param(0, "sample.filter.resonance", 61.0);
            engine.set_synth_param(1, "synth.filter.cutoffHz", 2_300.0);
            engine.cc(1, 74, 91);
            engine.cc(1, 71, 83);
        }
        let (lifecycle, mut runtime) =
            SourceWorkerLifecycle::start_prewarmed(&mut worker).expect("worker runtime");
        runtime.set_deadline_for_test(TEST_DEADLINE);
        assert_eq!(worker.synth_render_revisions[1], 2);
        assert_worker_matches_inline(&mut runtime, &mut worker, &mut inline, frames);

        assert!(runtime
            .with_controls_ready(&mut worker, |engine| engine.note_off(0, 36))
            .is_some());
        inline.note_off(0, 36);
        assert!(runtime
            .with_controls_ready(&mut worker, |engine| engine.note_off(1, 60))
            .is_some());
        inline.note_off(1, 60);
        assert_worker_matches_inline(&mut runtime, &mut worker, &mut inline, frames);
        let retirement = runtime.retire();
        assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    }
}

#[test]
fn persistent_workers_match_inline_with_full_mixed_pools() {
    for frames in SUPPORTED_QUANTA {
        let mut worker = full_mixed_engine();
        let mut inline = full_mixed_engine();
        for slot in 0..8 {
            for note in 0..8 {
                worker.note_on(slot as u8, 48 + note, 96, 5_000);
                inline.note_on(slot as u8, 48 + note, 96, 5_000);
            }
        }
        for slot in 0..8 {
            let sampler = InstrumentSlotConfig {
                kind: "sampler".into(),
                synth: default_synth_config(),
                mixer: None,
            };
            worker.set_instrument_slot(slot, sampler.clone());
            inline.set_instrument_slot(slot, sampler);
            for _ in 0..8 {
                worker.note_on(slot as u8, 36, 96, 5_000);
                inline.note_on(slot as u8, 36, 96, 5_000);
            }
        }
        assert_eq!(worker.profile_snapshot().active_synth_voices, 64);
        assert_eq!(worker.profile_snapshot().active_sample_voices, 64);
        let (lifecycle, mut runtime) =
            SourceWorkerLifecycle::start_prewarmed(&mut worker).expect("worker runtime");
        runtime.set_deadline_for_test(TEST_DEADLINE);
        assert_worker_matches_inline(&mut runtime, &mut worker, &mut inline, frames);
        let retirement = runtime.retire();
        assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    }
}

#[test]
fn sparse_reduction_visits_only_active_synth_lanes() {
    let mut worker = full_mixed_engine();
    let mut inline = full_mixed_engine();
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in [48, 55] {
            worker.note_on(slot as u8, note, 96, 5_000);
            inline.note_on(slot as u8, note, 96, 5_000);
        }
    }
    assert_eq!(worker.profile_snapshot().active_synth_voices, 16);
    assert_eq!(worker.profile_snapshot().active_sample_voices, 0);

    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut worker).expect("worker runtime");
    runtime.set_deadline_for_test(TEST_DEADLINE);
    assert_worker_matches_inline(&mut runtime, &mut worker, &mut inline, 64);
    assert_eq!(
        runtime.reduction_lane_counts_for_test(),
        [(64, 0), (64, 16)]
    );
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn reverse_completion_order_reduces_in_canonical_lane_order() {
    let mut worker = dynamic_engine();
    let mut inline = dynamic_engine();
    for engine in [&mut worker, &mut inline] {
        engine.note_on(0, 36, 100, 5_000);
        engine.note_on(1, 60, 100, 5_000);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut worker).expect("worker runtime");
    runtime.set_deadline_for_test(TEST_DEADLINE);
    lifecycle.set_reverse_completion_for_test(true);
    assert_worker_matches_inline(&mut runtime, &mut worker, &mut inline, 256);
    assert_eq!(worker.active_synth_slots, inline.active_synth_slots);
    assert_eq!(worker.active_sample_slots, inline.active_sample_slots);
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
