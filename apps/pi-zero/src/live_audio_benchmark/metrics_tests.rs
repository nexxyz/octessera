use super::CallbackMetrics;
use crate::live_audio_benchmark::output_counters::PersistentOutputProvenanceEvidence;
use rodio_engine_source::PersistentOutputCounters;
use std::time::Duration;

fn metrics() -> CallbackMetrics {
    CallbackMetrics::new(44_100, 64, 256)
}

#[test]
fn histogram_uses_actual_callback_frames_and_conservative_buckets() {
    let metrics = metrics();
    metrics.enable_measurement();
    for _ in 0..999 {
        metrics.record_callback(256, Duration::from_micros(10_000), 1, 0.5, 0, None);
    }
    metrics.record_callback(256, Duration::MAX, 1, 0.5, 0, None);
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.callback_count, 1_000);
    assert_eq!(snapshot.over_audio_duration_budget_count, 1_000);
    assert_eq!(snapshot.render_audio_duration_ratio_p50, 1.73);
    assert_eq!(snapshot.render_audio_duration_ratio_p99_9, 1.73);
    assert!(snapshot.render_audio_duration_ratio_max >= 4.0);
}

#[test]
fn variable_callback_batches_are_valid_and_counted() {
    let metrics = metrics();
    metrics.enable_measurement();
    for frames in [256, 64, 70, 128, 64] {
        metrics.record_callback(frames, Duration::from_nanos(1), 1, 0.5, 0, None);
    }
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.callback_frames_min, 64);
    assert_eq!(snapshot.callback_frames_max, 256);
    assert_eq!(snapshot.callback_frame_sample_count, 5);
    assert_eq!(snapshot.callback_frame_size_change_count, 4);
    assert_eq!(snapshot.invalid_callback_frame_count, 0);
    assert_eq!(snapshot.lifetime_callback_frames_min, 64);
    assert_eq!(snapshot.lifetime_callback_frames_max, 256);
    assert!(!snapshot.terminal_error);
}

#[test]
fn zero_and_over_buffer_batches_are_terminal() {
    for frames in [0, 257] {
        let metrics = metrics();
        metrics.enable_measurement();
        metrics.record_callback(frames, Duration::from_nanos(1), 0, 0.0, 0, None);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.invalid_callback_frame_count, 1);
        assert!(snapshot.terminal_error);
    }
}

#[test]
fn spacing_lateness_uses_fixed_alsa_period() {
    let metrics = metrics();
    metrics.enable_measurement();
    metrics.record_callback(256, Duration::from_nanos(1), 0, 0.0, 0, None);
    metrics.record_callback(64, Duration::from_nanos(1), 0, 0.0, 0, Some(2_000_000));
    let expected = 2_000_000 - (64_u64 * 1_000_000_000 / 44_100);
    assert_eq!(metrics.snapshot().callback_lateness_max_ns, expected);
}

#[test]
fn reset_excludes_warmup_samples_but_keeps_lifetime_geometry() {
    let metrics = metrics();
    metrics.record_callback(256, Duration::from_nanos(1), 1, 0.5, 0, None);
    metrics.enable_measurement();
    metrics.record_callback(64, Duration::from_nanos(1), 1, 0.5, 0, None);
    metrics.disable_measurement();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.callback_count, 1);
    assert_eq!(snapshot.lifetime_callback_count, 2);
    assert_eq!(snapshot.callback_frames_min, 64);
    assert_eq!(snapshot.lifetime_callback_frames_min, 64);
    assert_eq!(snapshot.lifetime_callback_frames_max, 256);
}

#[test]
fn post_mute_proof_consumes_variable_batches() {
    let metrics = metrics();
    metrics.enable_measurement();
    for frames in [256, 64, 128] {
        metrics.record_callback(frames, Duration::from_nanos(1), frames as u64, 0.5, 0, None);
    }
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.rendered_frames, 448);
    assert_eq!(snapshot.pre_mute_nonzero_samples, 448);
    assert_eq!(snapshot.post_mute_nonzero_samples, 0);
}

#[test]
fn phase_boundary_counters_are_mirrored_without_changing_callback_metrics() {
    let metrics = metrics();
    let counters = PersistentOutputCounters {
        rendered_quantums: 8,
        repeated_quantums: 1,
        dropped_quantums: 2,
        deadline_misses: 3,
        deadline_recoveries: 1,
    };
    metrics.publish_phase_boundary(7, counters);

    assert_eq!(metrics.phase_boundary_snapshot(7), Some(counters));
    assert_eq!(
        metrics.snapshot(),
        super::CallbackMetricsSnapshot::default()
    );
}

#[test]
fn measurement_provenance_is_cumulative_and_resets_after_warmup() {
    let metrics = metrics();
    metrics.record_persistent_output_provenance(PersistentOutputProvenanceEvidence {
        repeated_quantum_incidents: 9,
        repeated_pcm_frames: 64,
        silent_quantum_incidents: 2,
        silent_pcm_frames: 128,
        ..Default::default()
    });
    metrics.enable_measurement();
    metrics.record_prefix(super::CallbackPrefix {
        entry_ns: 1,
        measured: true,
        frames: 64,
        pre_mute_nonzero: 64,
        pre_mute_peak: 0.5,
        post_mute_nonzero: 0,
        spacing_ns: None,
    });
    metrics.record_persistent_output_provenance(PersistentOutputProvenanceEvidence {
        repeated_quantum_incidents: 1,
        repeated_pcm_frames: 32,
        silent_quantum_incidents: 1,
        silent_pcm_frames: 64,
        ..Default::default()
    });
    assert_eq!(
        metrics.persistent_output_provenance(),
        PersistentOutputProvenanceEvidence {
            repeated_quantum_incidents: 1,
            repeated_pcm_frames: 32,
            silent_quantum_incidents: 1,
            silent_pcm_frames: 64,
            ..Default::default()
        }
    );
}
