use super::{PersistentOutputCounters, PersistentOutputCountersEvidence};
use crate::orange_audio_benchmark::cli::BenchmarkExecutorMode;
use std::sync::{Arc, Barrier};
use std::thread;

fn counters(
    rendered: u64,
    repeated: u64,
    dropped: u64,
    misses: u64,
    recoveries: u64,
) -> PersistentOutputCounters {
    PersistentOutputCounters {
        rendered_quantums: rendered,
        repeated_quantums: repeated,
        dropped_quantums: dropped,
        deadline_misses: misses,
        deadline_recoveries: recoveries,
    }
}

fn persistent_evidence() -> PersistentOutputCountersEvidence {
    PersistentOutputCountersEvidence {
        observable: true,
        warmup: counters(3, 1, 0, 1, 1),
        start: counters(5, 1, 1, 2, 1),
        end: counters(7, 2, 3, 4, 3),
        delta: counters(2, 1, 2, 2, 2),
    }
}

#[test]
fn delta_and_continuity_count_include_one_carry_in_episode() {
    let evidence = persistent_evidence();
    evidence
        .validate(BenchmarkExecutorMode::PersistentTwoWorkers)
        .unwrap();
    assert_eq!(evidence.detected_continuity_events(0), 3);
    assert_eq!(evidence.detected_continuity_events(3), 3);
    assert_eq!(evidence.detected_continuity_events(4), 4);
}

#[test]
fn delta_is_calculated_from_cumulative_snapshots() {
    let mut evidence = persistent_evidence();
    evidence.delta = PersistentOutputCounters::default();
    evidence.calculate_delta().unwrap();
    assert_eq!(evidence.delta, counters(2, 1, 2, 2, 2));
}

#[test]
fn delta_accepts_one_carry_in_recovery_with_multiple_dropped_quanta() {
    let evidence = PersistentOutputCountersEvidence {
        observable: true,
        warmup: counters(3, 1, 0, 1, 1),
        start: counters(5, 1, 1, 2, 1),
        end: counters(7, 1, 4, 2, 2),
        delta: counters(2, 0, 3, 0, 1),
    };
    evidence
        .validate(BenchmarkExecutorMode::PersistentTwoWorkers)
        .unwrap();
    assert_eq!(evidence.detected_continuity_events(0), 1);
}

#[test]
fn delta_rejects_more_than_one_carry_in_recovery() {
    let evidence = PersistentOutputCountersEvidence {
        observable: true,
        warmup: counters(3, 1, 0, 1, 1),
        start: counters(5, 1, 4, 5, 2),
        end: counters(7, 1, 6, 5, 4),
        delta: counters(2, 0, 2, 0, 2),
    };
    assert!(evidence
        .validate(BenchmarkExecutorMode::PersistentTwoWorkers)
        .is_err());
}

#[test]
fn delta_rejects_a_miss_without_a_window_disposition() {
    let evidence = PersistentOutputCountersEvidence {
        observable: true,
        warmup: counters(1, 0, 1, 1, 0),
        start: counters(2, 0, 2, 1, 0),
        end: counters(3, 0, 2, 2, 0),
        delta: counters(1, 0, 0, 1, 0),
    };
    assert!(evidence
        .validate(BenchmarkExecutorMode::PersistentTwoWorkers)
        .is_err());
}

#[test]
fn output_counter_validation_rejects_inconsistent_evidence() {
    let cases: [fn(&mut PersistentOutputCountersEvidence); 6] = [
        |evidence| evidence.start.rendered_quantums = evidence.warmup.rendered_quantums - 1,
        |evidence| evidence.delta.deadline_misses += 1,
        |evidence| evidence.end.deadline_recoveries = evidence.end.deadline_misses + 1,
        |evidence| evidence.end.repeated_quantums = evidence.end.deadline_misses + 1,
        |evidence| {
            evidence.end.deadline_misses =
                evidence.end.repeated_quantums + evidence.end.dropped_quantums + 1;
        },
        |evidence| evidence.observable = false,
    ];
    for mutate in cases {
        let mut evidence = persistent_evidence();
        mutate(&mut evidence);
        assert!(evidence
            .validate(BenchmarkExecutorMode::PersistentTwoWorkers)
            .is_err());
    }
}

#[test]
fn inline_output_counter_evidence_is_exactly_zero_and_unobservable() {
    let evidence = PersistentOutputCountersEvidence::for_executor(BenchmarkExecutorMode::Inline);
    evidence.validate(BenchmarkExecutorMode::Inline).unwrap();

    let mut invalid = evidence;
    invalid.end.rendered_quantums = 1;
    assert!(invalid.validate(BenchmarkExecutorMode::Inline).is_err());
}

#[test]
fn seqlock_stress_preserves_coherent_counter_snapshots_under_concurrency() {
    const ITERATIONS: u64 = 100_000;
    let mirror = Arc::new(super::PersistentOutputCountersMirror::new());
    let barrier = Arc::new(Barrier::new(2));
    let writer_mirror = Arc::clone(&mirror);
    let writer_barrier = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        writer_barrier.wait();
        for generation in 1..=ITERATIONS {
            writer_mirror.publish(PersistentOutputCounters {
                rendered_quantums: generation,
                repeated_quantums: generation * 2,
                dropped_quantums: generation * 3,
                deadline_misses: generation * 5,
                deadline_recoveries: generation,
            });
        }
    });
    let reader_barrier = Arc::clone(&barrier);
    let reader = thread::spawn(move || {
        reader_barrier.wait();
        for _ in 0..ITERATIONS {
            let counters = mirror.snapshot();
            if counters.rendered_quantums == 0 {
                assert_eq!(counters, PersistentOutputCounters::default());
            } else {
                assert_eq!(counters.repeated_quantums, counters.rendered_quantums * 2);
                assert_eq!(counters.dropped_quantums, counters.rendered_quantums * 3);
                assert_eq!(counters.deadline_misses, counters.rendered_quantums * 5);
                assert_eq!(counters.deadline_recoveries, counters.rendered_quantums);
            }
        }
    });
    writer.join().unwrap();
    reader.join().unwrap();
}
