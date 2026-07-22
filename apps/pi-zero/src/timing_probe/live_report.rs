use super::live_probe::{
    LiveEventRecord, LiveSendRecord, LiveStreamReport, LiveSummary, LiveTimingProbeReport,
    SlowSendReport,
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
        intervals_us: summarize(&intervals),
        first_window_interval_us: summarize(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_us: summarize(
            &intervals
                .iter()
                .rev()
                .take(window)
                .copied()
                .collect::<Vec<_>>(),
        ),
    })
}

pub(super) fn summarize_usize(values: &[usize]) -> LiveSummary {
    summarize(&values.iter().map(|value| *value as f64).collect::<Vec<_>>())
}

pub(super) fn summarize(values: &[f64]) -> LiveSummary {
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
            "{:?} {}ms live-audio events={} interval_p95={:.0}us wake_late_p95={:.0}us loop_p95={:.0}us audio_send_p95={:.0}us send_p95={:.0}us batch_max={:.0}",
            report.scenario,
            report.duration_ms,
            report.events,
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
