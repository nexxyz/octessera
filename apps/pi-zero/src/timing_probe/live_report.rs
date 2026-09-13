use super::cadence::AdvanceCorrelation;
use super::live_probe::{
    LiveCountSummary, LiveEventRecord, LiveSendRecord, LiveStreamReport, LiveSummary,
    LiveTimingProbeReport, SlowSendReport,
};

pub(super) fn intervals_u128(times: &[u128]) -> Vec<f64> {
    times
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]) as f64)
        .collect()
}

pub(super) fn primary_stream_report(events: &[LiveEventRecord]) -> Option<LiveStreamReport> {
    let mut keys = Vec::<String>::new();
    for event in events {
        if !keys.iter().any(|key| key == &event.key) {
            keys.push(event.key.clone());
        }
    }
    keys.into_iter()
        .filter_map(|key| stream_report(events, key))
        .max_by_key(|report| report.events)
}

fn stream_report(events: &[LiveEventRecord], key: String) -> Option<LiveStreamReport> {
    let times = events
        .iter()
        .filter(|event| event.key == key)
        .map(|event| event.at_us)
        .collect::<Vec<_>>();
    if times.len() < 2 {
        return None;
    }
    let intervals = intervals_u128(&times);
    let window = intervals.len().min(128);
    Some(LiveStreamReport {
        key,
        events: times.len(),
        intervals_us: summarize_us(&intervals),
        first_window_interval_us: summarize_us(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_us: summarize_us(
            &intervals
                .iter()
                .rev()
                .take(window)
                .copied()
                .collect::<Vec<_>>(),
        ),
    })
}

pub(super) fn summarize_counts(values: &[usize]) -> LiveCountSummary {
    summarize_count_values(&values.iter().map(|value| *value as f64).collect::<Vec<_>>())
}

#[derive(Default)]
pub(super) struct AdvanceCorrelationSummary {
    pub(super) event_producing_advances: usize,
    pub(super) multi_pulse_advances: usize,
    pub(super) multi_pulse_event_producing_advances: usize,
    pub(super) pulses_on_event_producing_advances: LiveCountSummary,
}

pub(super) fn summarize_advance_correlations(
    values: &[AdvanceCorrelation],
) -> AdvanceCorrelationSummary {
    let event_producing_pulses = values
        .iter()
        .filter(|value| value.event_producing)
        .map(|value| value.pulses as f64)
        .collect::<Vec<_>>();
    AdvanceCorrelationSummary {
        event_producing_advances: event_producing_pulses.len(),
        multi_pulse_advances: values.iter().filter(|value| value.pulses > 1).count(),
        multi_pulse_event_producing_advances: values
            .iter()
            .filter(|value| value.event_producing && value.pulses > 1)
            .count(),
        pulses_on_event_producing_advances: summarize_count_values(&event_producing_pulses),
    }
}

pub(super) fn summarize_us(values: &[f64]) -> LiveSummary {
    if values.is_empty() {
        return LiveSummary::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    LiveSummary {
        count: values.len(),
        min: sorted[0],
        max: *sorted.last().unwrap(),
        mean: values.iter().sum::<f64>() / values.len() as f64,
        p95: percentile(&sorted, 9500),
        p99: percentile(&sorted, 9900),
        p999: percentile(&sorted, 9990),
        p9999: percentile(&sorted, 9999),
        over_1ms: values.iter().filter(|value| **value > 1_000.0).count(),
        over_5ms: values.iter().filter(|value| **value > 5_000.0).count(),
        over_10ms: values.iter().filter(|value| **value > 10_000.0).count(),
        over_20ms: values.iter().filter(|value| **value > 20_000.0).count(),
    }
}

fn summarize_count_values(values: &[f64]) -> LiveCountSummary {
    if values.is_empty() {
        return LiveCountSummary::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    LiveCountSummary {
        count: values.len(),
        min: sorted[0],
        max: *sorted.last().unwrap(),
        mean: values.iter().sum::<f64>() / values.len() as f64,
        p95: percentile(&sorted, 9500),
        p99: percentile(&sorted, 9900),
        p999: percentile(&sorted, 9990),
        p9999: percentile(&sorted, 9999),
    }
}

fn percentile(sorted: &[f64], basis_points: usize) -> f64 {
    let index = ((sorted.len() - 1) * basis_points) / 10_000;
    sorted[index]
}

pub(super) fn slow_sends(sends: &[LiveSendRecord]) -> Vec<SlowSendReport> {
    let mut sorted = sends.to_vec();
    sorted.sort_by(|a, b| b.duration_us.total_cmp(&a.duration_us));
    sorted
        .into_iter()
        .take(8)
        .map(|send| SlowSendReport {
            label: send.label,
            duration_us: send.duration_us,
        })
        .collect()
}

pub(super) fn print_live_summary(reports: &[LiveTimingProbeReport]) {
    for report in reports {
        eprintln!(
            "{:?} {}ms measured={}ms wake={}ms live-audio output={} internal={} events={} event_advances={} multi_pulse_advances={} multi_pulse_event_advances={} event_pulses_mean={:.2} interval_p95={:.0}us wake_late_p95={:.0}us loop_p95={:.0}us audio_send_p95={:.0}us send_p95={:.0}us batch_max={:.0}",
            report.scenario,
            report.duration_ms,
            report.measured_duration_ms,
            report.wake_interval_ms,
            report.output_buffer_frames,
            report.internal_block_frames,
            report.events,
            report.event_producing_advances,
            report.multi_pulse_advances,
            report.multi_pulse_event_producing_advances,
            report.pulses_on_event_producing_advances.mean,
            report.event_intervals_us.p95,
            report.wake_late_us.p95,
            report.loop_us.p95,
            report.audio_send_us.p95,
            report.runner_send_us.p95,
            report.event_batches.max
        );
        if let Some(stream) = &report.primary_stream {
            eprintln!(
                "  primary={} events={} interval_p95={:.0}us p99={:.0}us p999={:.0}us max={:.0}us first_p95={:.0}us last_p95={:.0}us",
                stream.key,
                stream.events,
                stream.intervals_us.p95,
                stream.intervals_us.p99,
                stream.intervals_us.p999,
                stream.intervals_us.max,
                stream.first_window_interval_us.p95,
                stream.last_window_interval_us.p95
            );
        }
        eprintln!(
            "  wake_late p99={:.0}us p999={:.0}us p9999={:.0}us max={:.0}us >5ms={} >10ms={} >20ms={}",
            report.wake_late_us.p99,
            report.wake_late_us.p999,
            report.wake_late_us.p9999,
            report.wake_late_us.max,
            report.wake_late_us.over_5ms,
            report.wake_late_us.over_10ms,
            report.wake_late_us.over_20ms
        );
        eprintln!(
            "  loop p99={:.0}us p999={:.0}us p9999={:.0}us max={:.0}us >5ms={} >10ms={} >20ms={}",
            report.loop_us.p99,
            report.loop_us.p999,
            report.loop_us.p9999,
            report.loop_us.max,
            report.loop_us.over_5ms,
            report.loop_us.over_10ms,
            report.loop_us.over_20ms
        );
        for send in &report.slow_sends {
            eprintln!("  slow_send={} {:.0}us", send.label, send.duration_us);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{summarize_advance_correlations, summarize_us, LiveCountSummary};
    use crate::timing_probe::cadence::AdvanceCorrelation;

    #[test]
    fn time_summary_thresholds_use_microseconds() {
        let summary = summarize_us(&[500.0, 1_500.0, 5_500.0, 10_500.0, 20_500.0]);

        assert_eq!(summary.over_1ms, 4);
        assert_eq!(summary.over_5ms, 3);
        assert_eq!(summary.over_10ms, 2);
        assert_eq!(summary.over_20ms, 1);
    }

    #[test]
    fn pulse_count_summary_json_has_no_time_unit_fields() {
        let summary = summarize_advance_correlations(&[AdvanceCorrelation {
            pulses: 2,
            event_producing: true,
        }]);
        let object = serde_json::to_value(summary.pulses_on_event_producing_advances)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();

        assert_eq!(object.len(), 8);
        for key in ["count", "min", "max", "mean", "p95", "p99", "p999", "p9999"] {
            assert!(object.contains_key(key), "missing {key}");
        }
        for key in ["over_1ms", "over_5ms", "over_10ms", "over_20ms"] {
            assert!(!object.contains_key(key), "unexpected {key}");
        }
        let _: LiveCountSummary = summary.pulses_on_event_producing_advances;
    }
}
