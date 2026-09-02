use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

const HISTOGRAM_BUCKETS: usize = 400;
const HISTOGRAM_STEP: f64 = 0.01;

#[derive(Clone, Copy, Debug)]
pub struct CallbackPrefix {
    pub entry_ns: u64,
    pub measured: bool,
    pub frames: u32,
    pub pre_mute_nonzero: u64,
    pub pre_mute_peak: f32,
    pub post_mute_nonzero: u64,
    pub spacing_ns: Option<u64>,
}

pub struct CallbackMetrics {
    sample_rate: u32,
    expected_period_frames: u32,
    max_callback_frames: u32,
    measurement_enabled: AtomicBool,
    lifetime_callback_count: AtomicU64,
    lifetime_frames_min: AtomicU32,
    lifetime_frames_max: AtomicU32,
    lifetime_frame_sample_count: AtomicU64,
    lifetime_frame_size_change_count: AtomicU64,
    lifetime_invalid_frame_count: AtomicU64,
    last_lifetime_frames: AtomicU32,
    measured_callback_count: AtomicU64,
    measured_frames_min: AtomicU32,
    measured_frames_max: AtomicU32,
    measured_frame_sample_count: AtomicU64,
    measured_frame_size_change_count: AtomicU64,
    measured_invalid_frame_count: AtomicU64,
    last_measured_frames: AtomicU32,
    first_measured_callback_ns: AtomicU64,
    last_measured_callback_ns: AtomicU64,
    rendered_frames: AtomicU64,
    render_audio_duration_ns: AtomicU64,
    ratio_histogram: [AtomicU64; HISTOGRAM_BUCKETS],
    render_audio_duration_ratio_max_bits: AtomicU64,
    over_audio_duration_budget_count: AtomicU64,
    callback_spacing_min_ns: AtomicU64,
    callback_spacing_max_ns: AtomicU64,
    callback_lateness_max_ns: AtomicU64,
    callback_timestamp_count: AtomicU64,
    pre_mute_nonzero_samples: AtomicU64,
    pre_mute_peak_bits: AtomicU32,
    post_mute_nonzero_samples: AtomicU64,
    cpal_device_errors: AtomicU64,
    cpal_stream_errors: AtomicU64,
    worker_terminal: AtomicBool,
    terminal_error: AtomicBool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct CallbackMetricsSnapshot {
    pub lifetime_callback_count: u64,
    pub callback_count: u64,
    pub first_measured_callback_ns: u64,
    pub last_measured_callback_ns: u64,
    pub measured_elapsed_ns: u64,
    pub callback_frames_min: u32,
    pub callback_frames_max: u32,
    pub callback_frame_sample_count: u64,
    pub callback_frame_size_change_count: u64,
    pub invalid_callback_frame_count: u64,
    pub lifetime_callback_frames_min: u32,
    pub lifetime_callback_frames_max: u32,
    pub lifetime_callback_frame_sample_count: u64,
    pub lifetime_callback_frame_size_change_count: u64,
    pub lifetime_invalid_callback_frame_count: u64,
    pub rendered_frames: u64,
    pub render_audio_duration_ns: u64,
    pub render_audio_duration_ratio_p50: f64,
    pub render_audio_duration_ratio_p95: f64,
    pub render_audio_duration_ratio_p99: f64,
    pub render_audio_duration_ratio_p99_9: f64,
    pub render_audio_duration_ratio_max: f64,
    pub over_audio_duration_budget_count: u64,
    pub callback_spacing_min_ns: u64,
    pub callback_spacing_max_ns: u64,
    pub callback_lateness_max_ns: u64,
    pub callback_timestamp_observed: bool,
    pub pre_mute_nonzero_samples: u64,
    pub pre_mute_peak: f32,
    pub post_mute_nonzero_samples: u64,
    pub cpal_device_error_count: u64,
    pub cpal_stream_error_count: u64,
    pub worker_terminal: bool,
    pub terminal_error: bool,
}

impl CallbackMetrics {
    pub fn new(sample_rate: u32, expected_period_frames: u32, max_callback_frames: u32) -> Self {
        Self {
            sample_rate,
            expected_period_frames,
            max_callback_frames,
            measurement_enabled: AtomicBool::new(false),
            lifetime_callback_count: AtomicU64::new(0),
            lifetime_frames_min: AtomicU32::new(u32::MAX),
            lifetime_frames_max: AtomicU32::new(0),
            lifetime_frame_sample_count: AtomicU64::new(0),
            lifetime_frame_size_change_count: AtomicU64::new(0),
            lifetime_invalid_frame_count: AtomicU64::new(0),
            last_lifetime_frames: AtomicU32::new(0),
            measured_callback_count: AtomicU64::new(0),
            measured_frames_min: AtomicU32::new(u32::MAX),
            measured_frames_max: AtomicU32::new(0),
            measured_frame_sample_count: AtomicU64::new(0),
            measured_frame_size_change_count: AtomicU64::new(0),
            measured_invalid_frame_count: AtomicU64::new(0),
            last_measured_frames: AtomicU32::new(0),
            first_measured_callback_ns: AtomicU64::new(u64::MAX),
            last_measured_callback_ns: AtomicU64::new(0),
            rendered_frames: AtomicU64::new(0),
            render_audio_duration_ns: AtomicU64::new(0),
            ratio_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            render_audio_duration_ratio_max_bits: AtomicU64::new(0),
            over_audio_duration_budget_count: AtomicU64::new(0),
            callback_spacing_min_ns: AtomicU64::new(u64::MAX),
            callback_spacing_max_ns: AtomicU64::new(0),
            callback_lateness_max_ns: AtomicU64::new(0),
            callback_timestamp_count: AtomicU64::new(0),
            pre_mute_nonzero_samples: AtomicU64::new(0),
            pre_mute_peak_bits: AtomicU32::new(0),
            post_mute_nonzero_samples: AtomicU64::new(0),
            cpal_device_errors: AtomicU64::new(0),
            cpal_stream_errors: AtomicU64::new(0),
            worker_terminal: AtomicBool::new(false),
            terminal_error: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    pub fn record_callback(
        &self,
        frames: u32,
        duration: Duration,
        pre_mute_nonzero: u64,
        pre_mute_peak: f32,
        post_mute_nonzero: u64,
        spacing_ns: Option<u64>,
    ) {
        let measured = self.record_prefix(CallbackPrefix {
            entry_ns: 1,
            measured: true,
            frames,
            pre_mute_nonzero,
            pre_mute_peak,
            post_mute_nonzero,
            spacing_ns,
        });
        self.publish_timing(measured, frames, duration);
    }

    pub fn record_prefix(&self, prefix: CallbackPrefix) -> bool {
        self.lifetime_callback_count.fetch_add(1, Ordering::Relaxed);
        let valid = self.record_geometry(prefix.frames);
        let measurement_enabled = self.measurement_enabled.load(Ordering::Acquire);
        if !valid {
            if prefix.measured && measurement_enabled {
                self.measured_invalid_frame_count
                    .fetch_add(1, Ordering::Relaxed);
            }
            return false;
        }
        if !prefix.measured || !measurement_enabled {
            return false;
        }
        record_valid_frame(
            prefix.frames,
            &self.measured_frames_min,
            &self.measured_frames_max,
            &self.measured_frame_sample_count,
            &self.measured_frame_size_change_count,
            &self.last_measured_frames,
        );
        self.measured_callback_count.fetch_add(1, Ordering::Relaxed);
        atomic_min_u64(&self.first_measured_callback_ns, prefix.entry_ns);
        atomic_max_u64(&self.last_measured_callback_ns, prefix.entry_ns);
        self.rendered_frames
            .fetch_add(u64::from(prefix.frames), Ordering::Relaxed);
        if let Some(spacing_ns) = prefix.spacing_ns {
            self.callback_timestamp_count
                .fetch_add(1, Ordering::Relaxed);
            atomic_min_u64(&self.callback_spacing_min_ns, spacing_ns);
            atomic_max_u64(&self.callback_spacing_max_ns, spacing_ns);
            atomic_max_u64(
                &self.callback_lateness_max_ns,
                spacing_ns.saturating_sub(self.period_duration_ns()),
            );
        }
        self.pre_mute_nonzero_samples
            .fetch_add(prefix.pre_mute_nonzero, Ordering::Relaxed);
        atomic_max_u32(&self.pre_mute_peak_bits, prefix.pre_mute_peak.to_bits());
        self.post_mute_nonzero_samples
            .fetch_add(prefix.post_mute_nonzero, Ordering::Relaxed);
        true
    }

    fn record_geometry(&self, frames: u32) -> bool {
        let valid = frames > 0 && frames <= self.max_callback_frames;
        if !valid {
            self.lifetime_invalid_frame_count
                .fetch_add(1, Ordering::Relaxed);
            self.terminal_error.store(true, Ordering::Relaxed);
            return false;
        }
        record_valid_frame(
            frames,
            &self.lifetime_frames_min,
            &self.lifetime_frames_max,
            &self.lifetime_frame_sample_count,
            &self.lifetime_frame_size_change_count,
            &self.last_lifetime_frames,
        );
        true
    }

    pub fn publish_timing(&self, measured: bool, frames: u32, duration: Duration) {
        if !measured {
            return;
        }
        let duration_ns = duration.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.render_audio_duration_ns
            .fetch_add(duration_ns, Ordering::Relaxed);
        let deadline_ns = self.audio_duration_ns(frames);
        let ratio = if deadline_ns == 0 {
            0.0
        } else {
            duration_ns as f64 / deadline_ns as f64
        };
        let bucket = ((ratio / HISTOGRAM_STEP).floor() as usize).min(HISTOGRAM_BUCKETS - 1);
        self.ratio_histogram[bucket].fetch_add(1, Ordering::Relaxed);
        atomic_max_u64(&self.render_audio_duration_ratio_max_bits, ratio.to_bits());
        if ratio > 1.0 {
            self.over_audio_duration_budget_count
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    fn audio_duration_ns(&self, frames: u32) -> u64 {
        (u64::from(frames) * 1_000_000_000) / u64::from(self.sample_rate)
    }

    fn period_duration_ns(&self) -> u64 {
        self.audio_duration_ns(self.expected_period_frames)
    }

    pub fn record_cpal_device_error(&self) {
        self.cpal_device_errors.fetch_add(1, Ordering::Relaxed);
        self.terminal_error.store(true, Ordering::Relaxed);
    }

    pub fn record_cpal_stream_error(&self) {
        self.cpal_stream_errors.fetch_add(1, Ordering::Relaxed);
        self.terminal_error.store(true, Ordering::Relaxed);
    }

    pub fn mark_terminal(&self) {
        self.terminal_error.store(true, Ordering::Relaxed);
    }

    pub fn mark_worker_terminal(&self) {
        self.worker_terminal.store(true, Ordering::Release);
        self.terminal_error.store(true, Ordering::Release);
    }

    pub fn enable_measurement(&self) {
        self.reset_measurement();
        self.measurement_enabled.store(true, Ordering::Release);
    }

    pub fn disable_measurement(&self) {
        self.measurement_enabled.store(false, Ordering::Release);
    }

    pub fn reset_measurement(&self) {
        self.measured_callback_count.store(0, Ordering::Relaxed);
        self.measured_frames_min.store(u32::MAX, Ordering::Relaxed);
        self.measured_frames_max.store(0, Ordering::Relaxed);
        self.measured_frame_sample_count.store(0, Ordering::Relaxed);
        self.measured_frame_size_change_count
            .store(0, Ordering::Relaxed);
        self.measured_invalid_frame_count
            .store(0, Ordering::Relaxed);
        self.last_measured_frames.store(0, Ordering::Relaxed);
        self.first_measured_callback_ns
            .store(u64::MAX, Ordering::Relaxed);
        self.last_measured_callback_ns.store(0, Ordering::Relaxed);
        self.rendered_frames.store(0, Ordering::Relaxed);
        self.render_audio_duration_ns.store(0, Ordering::Relaxed);
        for bucket in &self.ratio_histogram {
            bucket.store(0, Ordering::Relaxed);
        }
        self.render_audio_duration_ratio_max_bits
            .store(0, Ordering::Relaxed);
        self.over_audio_duration_budget_count
            .store(0, Ordering::Relaxed);
        self.callback_spacing_min_ns
            .store(u64::MAX, Ordering::Relaxed);
        self.callback_spacing_max_ns.store(0, Ordering::Relaxed);
        self.callback_lateness_max_ns.store(0, Ordering::Relaxed);
        self.callback_timestamp_count.store(0, Ordering::Relaxed);
        self.pre_mute_nonzero_samples.store(0, Ordering::Relaxed);
        self.pre_mute_peak_bits.store(0, Ordering::Relaxed);
        self.post_mute_nonzero_samples.store(0, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CallbackMetricsSnapshot {
        let count = self.measured_callback_count.load(Ordering::Relaxed);
        let timestamp_count = self.callback_timestamp_count.load(Ordering::Relaxed);
        let first = self.first_measured_callback_ns.load(Ordering::Relaxed);
        let last = self.last_measured_callback_ns.load(Ordering::Relaxed);
        CallbackMetricsSnapshot {
            lifetime_callback_count: self.lifetime_callback_count.load(Ordering::Relaxed),
            callback_count: count,
            first_measured_callback_ns: if count == 0 { 0 } else { first },
            last_measured_callback_ns: if count == 0 { 0 } else { last },
            measured_elapsed_ns: if count == 0 {
                0
            } else {
                last.saturating_sub(first)
            },
            callback_frames_min: min_or_zero(&self.measured_frames_min, count),
            callback_frames_max: self.measured_frames_max.load(Ordering::Relaxed),
            callback_frame_sample_count: self.measured_frame_sample_count.load(Ordering::Relaxed),
            callback_frame_size_change_count: self
                .measured_frame_size_change_count
                .load(Ordering::Relaxed),
            invalid_callback_frame_count: self.measured_invalid_frame_count.load(Ordering::Relaxed),
            lifetime_callback_frames_min: min_or_zero(
                &self.lifetime_frames_min,
                self.lifetime_frame_sample_count.load(Ordering::Relaxed),
            ),
            lifetime_callback_frames_max: self.lifetime_frames_max.load(Ordering::Relaxed),
            lifetime_callback_frame_sample_count: self
                .lifetime_frame_sample_count
                .load(Ordering::Relaxed),
            lifetime_callback_frame_size_change_count: self
                .lifetime_frame_size_change_count
                .load(Ordering::Relaxed),
            lifetime_invalid_callback_frame_count: self
                .lifetime_invalid_frame_count
                .load(Ordering::Relaxed),
            rendered_frames: self.rendered_frames.load(Ordering::Relaxed),
            render_audio_duration_ns: self.render_audio_duration_ns.load(Ordering::Relaxed),
            render_audio_duration_ratio_p50: self.percentile(0.50, count),
            render_audio_duration_ratio_p95: self.percentile(0.95, count),
            render_audio_duration_ratio_p99: self.percentile(0.99, count),
            render_audio_duration_ratio_p99_9: self.percentile(0.999, count),
            render_audio_duration_ratio_max: f64::from_bits(
                self.render_audio_duration_ratio_max_bits
                    .load(Ordering::Relaxed),
            ),
            over_audio_duration_budget_count: self
                .over_audio_duration_budget_count
                .load(Ordering::Relaxed),
            callback_spacing_min_ns: if timestamp_count == 0 {
                0
            } else {
                self.callback_spacing_min_ns.load(Ordering::Relaxed)
            },
            callback_spacing_max_ns: self.callback_spacing_max_ns.load(Ordering::Relaxed),
            callback_lateness_max_ns: self.callback_lateness_max_ns.load(Ordering::Relaxed),
            callback_timestamp_observed: timestamp_count > 0,
            pre_mute_nonzero_samples: self.pre_mute_nonzero_samples.load(Ordering::Relaxed),
            pre_mute_peak: f32::from_bits(self.pre_mute_peak_bits.load(Ordering::Relaxed)),
            post_mute_nonzero_samples: self.post_mute_nonzero_samples.load(Ordering::Relaxed),
            cpal_device_error_count: self.cpal_device_errors.load(Ordering::Relaxed),
            cpal_stream_error_count: self.cpal_stream_errors.load(Ordering::Relaxed),
            worker_terminal: self.worker_terminal.load(Ordering::Acquire),
            terminal_error: self.terminal_error.load(Ordering::Relaxed),
        }
    }

    fn percentile(&self, percentile: f64, count: u64) -> f64 {
        if count == 0 {
            return 0.0;
        }
        let target = ((count as f64 * percentile).ceil() as u64).max(1);
        let mut accumulated = 0;
        for (index, bucket) in self.ratio_histogram.iter().enumerate() {
            accumulated += bucket.load(Ordering::Relaxed);
            if accumulated >= target {
                return (index as f64 + 1.0) * HISTOGRAM_STEP;
            }
        }
        HISTOGRAM_BUCKETS as f64 * HISTOGRAM_STEP
    }
}

fn record_valid_frame(
    frames: u32,
    minimum: &AtomicU32,
    maximum: &AtomicU32,
    samples: &AtomicU64,
    changes: &AtomicU64,
    last: &AtomicU32,
) {
    atomic_min_u32(minimum, frames);
    atomic_max_u32(maximum, frames);
    samples.fetch_add(1, Ordering::Relaxed);
    let previous = last.swap(frames, Ordering::Relaxed);
    if previous != 0 && previous != frames {
        changes.fetch_add(1, Ordering::Relaxed);
    }
}

fn min_or_zero(value: &AtomicU32, count: u64) -> u32 {
    if count == 0 {
        0
    } else {
        value.load(Ordering::Relaxed)
    }
}

fn atomic_min_u32(target: &AtomicU32, value: u32) {
    let mut current = target.load(Ordering::Relaxed);
    while value < current {
        match target.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn atomic_max_u32(target: &AtomicU32, value: u32) {
    let mut current = target.load(Ordering::Relaxed);
    while value > current {
        match target.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn atomic_min_u64(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while value < current {
        match target.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn atomic_max_u64(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while value > current {
        match target.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
