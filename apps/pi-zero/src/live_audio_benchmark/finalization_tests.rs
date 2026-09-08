use super::*;
use crate::live_audio_benchmark::cli::{parse, BenchmarkExecutorMode, WorkerTimingMode};
use rodio_engine_source::PersistentOutputCounters;
use std::time::Duration;

fn config() -> BenchmarkConfig {
    let raspberry = super::super::geometry::is_raspberry_diagnostic();
    let mut config = parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        if raspberry { "128" } else { "1024" }.into(),
        "--engine-block-frames".into(),
        if raspberry { "32" } else { "256" }.into(),
        "--executor".into(),
        "inline".into(),
        "--worker-timing".into(),
        "disabled".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap();
    if raspberry {
        config.output_frames = 256;
        config.expected_alsa_period_frames = 64;
        config.internal_frames = 64;
        config.executor_mode = BenchmarkExecutorMode::RoutingTreePersistent;
        config.worker_timing_mode = WorkerTimingMode::Enabled;
    } else {
        config.executor_mode = BenchmarkExecutorMode::PersistentTwoWorkers;
        config.worker_timing_mode = WorkerTimingMode::Enabled;
    }
    config
}

#[cfg(feature = "routing-tree-benchmark")]
fn continuation_config() -> BenchmarkConfig {
    let mut config = config();
    config.scenario = "capacity_analogue_16".into();
    config.output_frames = 256;
    config.expected_alsa_period_frames = 64;
    config.internal_frames = 64;
    config.executor_mode = BenchmarkExecutorMode::RoutingTreePersistent;
    config.worker_timing_mode = WorkerTimingMode::Disabled;
    config.measure_seconds = 120;
    config.continue_on_recovered_miss = true;
    config
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
fn analogue_inline_config() -> BenchmarkConfig {
    let mut config = config();
    config.scenario = "capacity_analogue_1".into();
    config.output_frames = 128;
    config.expected_alsa_period_frames = 32;
    config.internal_frames = if super::super::geometry::is_raspberry_diagnostic() {
        32
    } else {
        64
    };
    config.executor_mode = BenchmarkExecutorMode::Inline;
    config.worker_timing_mode = WorkerTimingMode::Disabled;
    config
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

    let mut preview = snapshot;
    preview.active_preview_sample_voices = 1;
    assert!(validate_profile_state(
        &preview,
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
    let raspberry = super::super::geometry::is_raspberry_diagnostic();
    let config = crate::live_audio_benchmark::cli::parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        if raspberry { "64" } else { "256" }.into(),
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
        config.executor_mode,
        WorkerTimingMode::Enabled,
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

fn worker_thread_names() -> [String; 2] {
    if super::super::geometry::is_raspberry_diagnostic() {
        super::super::stream::expected_routing_worker_thread_names()
    } else {
        super::super::stream::expected_worker_thread_names()
    }
}

#[test]
fn run_state_captures_mirrored_cumulative_output_counters_at_each_boundary() {
    let config = config();
    let mut state = RunState::new(
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
        BenchmarkExecutorMode::PersistentTwoWorkers,
        WorkerTimingMode::Disabled,
    );
    let warmup_generation = state.phase_control.request(MeasurementPhase::Disabled);
    let warmup_capture = state.phase_control.capture_at_callback_entry();
    assert!(state
        .phase_control
        .acknowledgement(warmup_generation, MeasurementPhase::Disabled)
        .is_none());
    state.metrics.publish_phase_boundary(
        warmup_generation,
        PersistentOutputCounters {
            rendered_quantums: 2,
            ..Default::default()
        },
    );
    state.phase_control.acknowledge(warmup_capture);
    state.persistent_output_counters.warmup =
        state.phase_boundary_counters(warmup_generation).unwrap();
    let start_generation = state.phase_control.request(MeasurementPhase::Measuring);
    let start_capture = state.phase_control.capture_at_callback_entry();
    state.metrics.publish_phase_boundary(
        start_generation,
        PersistentOutputCounters {
            rendered_quantums: 3,
            dropped_quantums: 1,
            deadline_misses: 1,
            ..Default::default()
        },
    );
    state.phase_control.acknowledge(start_capture);
    state.persistent_output_counters.start =
        state.phase_boundary_counters(start_generation).unwrap();
    let end_generation = state.phase_control.request(MeasurementPhase::Disabled);
    let end_capture = state.phase_control.capture_at_callback_entry();
    state.metrics.publish_phase_boundary(
        end_generation,
        PersistentOutputCounters {
            rendered_quantums: 4,
            dropped_quantums: 2,
            deadline_misses: 1,
            ..Default::default()
        },
    );
    state.phase_control.acknowledge(end_capture);
    state.persistent_output_counters.end = state.phase_boundary_counters(end_generation).unwrap();
    state.persistent_output_counters.calculate_delta().unwrap();

    assert_eq!(state.persistent_output_counters.warmup.rendered_quantums, 2);
    assert_eq!(state.persistent_output_counters.start.dropped_quantums, 1);
    assert_eq!(state.persistent_output_counters.delta.rendered_quantums, 1);
    assert_eq!(state.persistent_output_counters.delta.dropped_quantums, 1);
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[path = "finalization_geometry_tests.rs"]
mod geometry_tests;
#[path = "finalization_result_tests.rs"]
mod result_tests;
