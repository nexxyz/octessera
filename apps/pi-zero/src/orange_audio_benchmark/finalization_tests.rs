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
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap()
}

#[test]
fn profile_validation_proves_max_fx_state() {
    let expected = crate::dsp_scenarios::expected_live_state(
        "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
    )
    .unwrap();
    let snapshot = SynthProfileSnapshot {
        active_synth_voices: 8,
        active_sample_voices: 8,
        active_momentary_fx: 2,
        active_bus_fx_slots: 12,
        active_global_fx_slots: 2,
        ..SynthProfileSnapshot::default()
    };
    validate_profile_state(
        &snapshot,
        expected,
        expected.expected_voice_admission_drops_start,
    )
    .unwrap();
    let mut invalid = snapshot;
    invalid.active_bus_fx_slots = 11;
    assert!(validate_profile_state(
        &invalid,
        expected,
        expected.expected_voice_admission_drops_start
    )
    .is_err());

    let mut admission_mismatch = snapshot;
    admission_mismatch.cumulative_voice_admission_drops = 1;
    let error = validate_profile_state(
        &admission_mismatch,
        expected,
        expected.expected_voice_admission_drops_start,
    )
    .unwrap_err();
    assert!(error.contains("voice admission drops"));
    assert!(!error.contains("voice steals"));

    let mut declared = expected;
    declared.expected_voice_admission_drops_start = 1;
    let mut admitted = snapshot;
    admitted.cumulative_voice_admission_drops = 1;
    validate_profile_state(
        &admitted,
        declared,
        declared.expected_voice_admission_drops_start,
    )
    .unwrap();
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
            active_bus_fx_slots: 0,
            active_global_fx_slots: 0,
            expected_voice_steals: 0,
            expected_voice_admission_drops_start: 0,
            expected_voice_admission_drops_end: 0,
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
fn result_status_requires_clean_runtime_evidence() {
    let config = config();
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        ..CallbackMetricsSnapshot::default()
    };
    let gates = FinalizationGates {
        no_terminal_errors: true,
        scheduler_qualified: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        worker_health: realtime_engine::synth::SourceWorkerHealth::Healthy,
        joined_workers: 2,
        retirement_error: true,
    };
    assert_eq!(result_status(&config, &metrics, gates), "pass");

    let invalid = CallbackMetricsSnapshot {
        over_audio_duration_budget_count: 1,
        ..metrics
    };
    assert_eq!(result_status(&config, &invalid, gates), "fail");
}

#[test]
fn injected_deadline_or_panic_worker_health_fails_benchmark_finalization() {
    let config = config();
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        worker_terminal: true,
        terminal_error: true,
        ..CallbackMetricsSnapshot::default()
    };
    for worker_health in [
        realtime_engine::synth::SourceWorkerHealth::DeadlineMiss,
        realtime_engine::synth::SourceWorkerHealth::WorkerExited,
    ] {
        let gates = FinalizationGates {
            no_terminal_errors: true,
            scheduler_qualified: true,
            measurement_stop_acknowledged: true,
            stream_stopped: true,
            final_progress_write_succeeded: true,
            worker_health,
            joined_workers: 2,
            retirement_error: true,
        };
        assert_eq!(result_status(&config, &metrics, gates), "fail");
    }
}
