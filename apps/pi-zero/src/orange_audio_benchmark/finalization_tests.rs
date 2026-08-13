use super::*;
use crate::orange_audio_benchmark::cli::parse;
use std::time::Duration;

fn config() -> BenchmarkConfig {
    parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "1024".into(),
        "--engine-block-frames".into(),
        "256".into(),
        "--workers".into(),
        "2".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap()
}

fn state_with_profiles(
    start: SynthProfileSnapshot,
    end: SynthProfileSnapshot,
    workers_effective: bool,
) -> RunState {
    let mut state = RunState::new(
        ExpectedLiveState {
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_momentary_fx: 0,
            expected_voice_steals: 0,
        },
        44_100,
        256,
        1024,
    );
    state.stream_evidence = Some(StreamEvidence {
        sample_format: "F32".into(),
        channels: 2,
        sample_rate: 44_100,
        workers_effective,
        engine_block_frames: 256,
    });
    state.profile_start = Some(start);
    state.profile_end = Some(end);
    state
}

#[test]
fn c2_policy_failure_keeps_the_measured_delta_without_terminal_error() {
    let state = state_with_profiles(
        SynthProfileSnapshot::default(),
        SynthProfileSnapshot {
            synth_parallel_dispatches: 4_639,
            synth_parallel_backoff_skips: 1_395,
            synth_parallel_timing_backoffs: 22,
            ..SynthProfileSnapshot::default()
        },
        true,
    );
    let evidence = resolve_worker_evidence(&config(), &state);
    assert_eq!(
        evidence.worker_delta,
        Some(BenchmarkWorkerDelta {
            synth_parallel_dispatches: 4_639,
            synth_parallel_light_skips: 0,
            synth_parallel_backoff_skips: 1_395,
            synth_parallel_timing_backoffs: 22,
            synth_parallel_failures: 0,
            synth_parallel_unhealthy: false,
        })
    );
    assert!(evidence.policy_error.is_some());
    assert!(evidence.terminal_error.is_none());
}

#[test]
fn clean_dispatch_is_complete_evidence() {
    let state = state_with_profiles(
        SynthProfileSnapshot::default(),
        SynthProfileSnapshot {
            synth_parallel_dispatches: 1,
            ..SynthProfileSnapshot::default()
        },
        true,
    );
    let evidence = resolve_worker_evidence(&config(), &state);
    assert_eq!(
        evidence.worker_delta,
        Some(BenchmarkWorkerDelta {
            synth_parallel_dispatches: 1,
            ..BenchmarkWorkerDelta::default()
        })
    );
    assert!(evidence.policy_error.is_none());
    assert!(evidence.terminal_error.is_none());
}

#[test]
fn candidate_spacing_uses_the_alsa_period_not_the_engine_block() {
    let config = crate::orange_audio_benchmark::cli::parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        "256".into(),
        "--workers".into(),
        "2".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap();
    let state = RunState::new(
        ExpectedLiveState {
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_momentary_fx: 0,
            expected_voice_steals: 0,
        },
        44_100,
        config.expected_alsa_period_frames,
        config.output_frames,
    );
    state.metrics.enable_measurement();
    state
        .metrics
        .record_callback(256, Duration::from_nanos(1), 0, 0.0, 0, Some(2_000_000));
    assert_eq!(
        state.metrics.snapshot().callback_lateness_max_ns,
        2_000_000 - (64_u64 * 1_000_000_000 / 44_100)
    );
}

#[test]
fn missing_required_dispatch_keeps_delta_as_policy_failure() {
    let state = state_with_profiles(
        SynthProfileSnapshot::default(),
        SynthProfileSnapshot::default(),
        true,
    );
    let evidence = resolve_worker_evidence(&config(), &state);
    assert_eq!(evidence.worker_delta, Some(BenchmarkWorkerDelta::default()));
    assert!(evidence.policy_error.is_some());
    assert!(evidence.terminal_error.is_none());
}

#[test]
fn light_skip_failure_and_unhealthy_state_keep_complete_deltas() {
    let light = SynthProfileSnapshot {
        synth_parallel_dispatches: 1,
        synth_parallel_light_skips: 1,
        ..SynthProfileSnapshot::default()
    };
    let failure = SynthProfileSnapshot {
        synth_parallel_dispatches: 1,
        synth_parallel_failures: 1,
        ..SynthProfileSnapshot::default()
    };
    let unhealthy = SynthProfileSnapshot {
        synth_parallel_dispatches: 1,
        synth_parallel_unhealthy: true,
        ..SynthProfileSnapshot::default()
    };

    let cases = [
        (
            light,
            BenchmarkWorkerDelta {
                synth_parallel_dispatches: 1,
                synth_parallel_light_skips: 1,
                ..BenchmarkWorkerDelta::default()
            },
        ),
        (
            failure,
            BenchmarkWorkerDelta {
                synth_parallel_dispatches: 1,
                synth_parallel_failures: 1,
                ..BenchmarkWorkerDelta::default()
            },
        ),
        (
            unhealthy,
            BenchmarkWorkerDelta {
                synth_parallel_dispatches: 1,
                synth_parallel_unhealthy: true,
                ..BenchmarkWorkerDelta::default()
            },
        ),
    ];

    for (end, expected_delta) in cases {
        let evidence = resolve_worker_evidence(
            &config(),
            &state_with_profiles(SynthProfileSnapshot::default(), end, true),
        );
        assert_eq!(evidence.worker_delta, Some(expected_delta));
        assert!(evidence.policy_error.is_some());
        assert!(evidence.terminal_error.is_none());
    }
}

#[test]
fn missing_profiles_and_counter_regressions_are_terminal_with_null_delta() {
    let config = config();
    let mut missing = state_with_profiles(
        SynthProfileSnapshot::default(),
        SynthProfileSnapshot::default(),
        true,
    );
    missing.profile_start = None;
    let evidence = resolve_worker_evidence(&config, &missing);
    assert_eq!(evidence.worker_delta, None);
    assert!(evidence.policy_error.is_none());
    assert!(evidence.terminal_error.is_some());

    for counter in 0..5 {
        let mut start = SynthProfileSnapshot::default();
        let end = SynthProfileSnapshot::default();
        match counter {
            0 => start.synth_parallel_dispatches = 1,
            1 => start.synth_parallel_light_skips = 1,
            2 => start.synth_parallel_backoff_skips = 1,
            3 => start.synth_parallel_timing_backoffs = 1,
            4 => start.synth_parallel_failures = 1,
            _ => unreachable!(),
        }
        let state = state_with_profiles(start, end, true);
        let evidence = resolve_worker_evidence(&config, &state);
        assert_eq!(evidence.worker_delta, None);
        assert!(evidence.policy_error.is_none());
        assert!(evidence.terminal_error.is_some());
    }
}

#[test]
fn worker_effectiveness_mismatch_is_terminal_with_null_delta() {
    let state = state_with_profiles(
        SynthProfileSnapshot::default(),
        SynthProfileSnapshot {
            synth_parallel_dispatches: 1,
            ..SynthProfileSnapshot::default()
        },
        false,
    );
    let evidence = resolve_worker_evidence(&config(), &state);
    assert_eq!(evidence.worker_delta, None);
    assert!(evidence.policy_error.is_none());
    assert!(evidence.terminal_error.is_some());
}

#[test]
fn policy_error_cannot_produce_pass_status() {
    let config = config();
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        ..CallbackMetricsSnapshot::default()
    };
    let delta = BenchmarkWorkerDelta {
        synth_parallel_dispatches: 1,
        ..BenchmarkWorkerDelta::default()
    };
    let gates = FinalizationGates {
        no_terminal_errors: true,
        scheduler_qualified: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
    };
    assert_eq!(
        result_status(&config, &metrics, gates, Some(&delta), None,),
        "pass"
    );
    assert_eq!(
        result_status(
            &config,
            &metrics,
            gates,
            Some(&delta),
            Some("worker telemetry reported a skip, failure, or unhealthy state"),
        ),
        "fail"
    );
}
