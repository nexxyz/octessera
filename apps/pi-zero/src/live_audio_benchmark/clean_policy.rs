use super::cli::BenchmarkConfig;
use super::metrics::CallbackMetricsSnapshot;
use super::schema::PersistentOutputCountersEvidence;

pub(super) fn result_passes(
    config: &BenchmarkConfig,
    metrics: &CallbackMetricsSnapshot,
    detected_continuity_events: u64,
) -> bool {
    #[cfg(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    ))]
    let raspberry_zero_event_policy = detected_continuity_events == 0
        && metrics.over_audio_duration_budget_count == 0
        && metrics.cpal_device_error_count == 0
        && metrics.cpal_stream_error_count == 0;
    #[cfg(not(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    )))]
    let raspberry_zero_event_policy = true;
    metrics.callback_count > 0
        && metrics.callback_frames_min > 0
        && metrics.callback_frames_max <= config.output_frames
        && metrics.callback_frame_sample_count == metrics.callback_count
        && metrics.invalid_callback_frame_count == 0
        && callback_budget_passes(
            config.measure_seconds,
            metrics.over_audio_duration_budget_count,
        )
        && (config.measure_seconds != 180
            || (detected_continuity_events == 0
                && metrics.over_audio_duration_budget_count == 0
                && metrics.cpal_device_error_count == 0
                && metrics.cpal_stream_error_count == 0))
        && metrics.pre_mute_nonzero_samples > 0
        && metrics.post_mute_nonzero_samples == 0
        && !metrics.worker_terminal
        && !metrics.terminal_error
        && raspberry_zero_event_policy
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
pub(super) fn persistent_output_counters_passes(
    _config: &BenchmarkConfig,
    evidence: &PersistentOutputCountersEvidence,
) -> bool {
    [
        evidence.warmup,
        evidence.start,
        evidence.end,
        evidence.delta,
    ]
    .into_iter()
    .all(|counters| {
        counters.repeated_quantums == 0
            && counters.dropped_quantums == 0
            && counters.deadline_misses == 0
            && counters.deadline_recoveries == 0
    })
}

#[cfg(not(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
)))]
pub(super) fn persistent_output_counters_passes(
    _config: &BenchmarkConfig,
    _evidence: &PersistentOutputCountersEvidence,
) -> bool {
    true
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
fn callback_budget_passes(_measure_seconds: u64, overrun_count: u64) -> bool {
    overrun_count == 0
}

#[cfg(not(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
)))]
fn callback_budget_passes(measure_seconds: u64, overrun_count: u64) -> bool {
    match measure_seconds {
        30 | 120 | 180 => overrun_count == 0,
        300 => overrun_count <= 5,
        600 => overrun_count <= 9,
        _ => false,
    }
}

#[cfg(all(
    test,
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
#[path = "clean_policy_tests.rs"]
mod tests;
