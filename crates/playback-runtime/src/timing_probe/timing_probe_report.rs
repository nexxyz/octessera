use super::{
    EventRecord, TimingProbeReport, TimingProbeScenario, TimingProbeStreamReport,
    TimingProbeSummary,
};
use crate::MusicalEvent;
use std::time::Duration;

pub(super) fn intervals(times: &[u64]) -> Vec<f64> {
    times
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]) as f64)
        .collect()
}

pub(super) fn event_key(event: &MusicalEvent) -> String {
    match event {
        MusicalEvent::NoteOn { channel, note, .. } => format!("note_on:{channel}:{note}"),
        MusicalEvent::NoteOff { channel, note } => format!("note_off:{channel}:{note}"),
        MusicalEvent::Cc {
            channel,
            controller,
            ..
        } => format!("cc:{channel}:{controller}"),
    }
}

pub(super) fn primary_stream_report(events: &[EventRecord]) -> Option<TimingProbeStreamReport> {
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

fn stream_report(events: &[EventRecord], key: String) -> Option<TimingProbeStreamReport> {
    let times = events
        .iter()
        .filter(|event| event.key == key)
        .map(|event| event.time_ms)
        .collect::<Vec<_>>();
    if times.len() < 2 {
        return None;
    }
    let intervals = intervals(&times);
    let window = intervals.len().min(128);
    Some(TimingProbeStreamReport {
        key,
        events: times.len(),
        intervals_ms: summarize(&intervals),
        first_window_interval_ms: summarize(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_ms: summarize(
            &intervals
                .iter()
                .rev()
                .take(window)
                .copied()
                .collect::<Vec<_>>(),
        ),
    })
}

pub(super) fn summarize_usize(values: &[usize]) -> TimingProbeSummary {
    summarize(&values.iter().map(|value| *value as f64).collect::<Vec<_>>())
}

pub(super) fn summarize(values: &[f64]) -> TimingProbeSummary {
    if values.is_empty() {
        return TimingProbeSummary::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    TimingProbeSummary {
        count: values.len(),
        min: sorted[0],
        max: *sorted.last().unwrap(),
        mean: values.iter().sum::<f64>() / values.len() as f64,
        p95: percentile(&sorted, 9500),
        p99: percentile(&sorted, 9900),
        p999: percentile(&sorted, 9990),
        p9999: percentile(&sorted, 9999),
        over_1ms: values.iter().filter(|value| **value > 1.0).count(),
        over_5ms: values.iter().filter(|value| **value > 5.0).count(),
        over_10ms: values.iter().filter(|value| **value > 10.0).count(),
        over_20ms: values.iter().filter(|value| **value > 20.0).count(),
    }
}

fn percentile(sorted: &[f64], basis_points: usize) -> f64 {
    let index = ((sorted.len() - 1) * basis_points) / 10_000;
    sorted[index]
}

pub fn print_timing_probe_summary(reports: &[TimingProbeReport]) {
    for report in reports {
        eprintln!("{:?} {}ms realtime={} events={} playing_statuses={} interval_mean={:.2}ms p95={:.2}ms send_p95={:.0}us advance_p95={:.0}us wake_late_p95={:.0}us loop_p95={:.0}us batch_max={:.0}", report.scenario, report.duration_ms, report.realtime, report.events, report.playing_statuses, report.event_intervals_ms.mean, report.event_intervals_ms.p95, report.runner_send_us.p95, report.advance_us.p95, report.wake_late_us.p95, report.loop_us.p95, report.event_batches.max);
    }
}

pub fn parse_timing_probe_scenarios(value: &str) -> Result<Vec<TimingProbeScenario>, String> {
    value
        .split(',')
        .map(|item| match item.trim() {
            "idle" => Ok(TimingProbeScenario::Idle),
            "sense" | "sense-stress" => Ok(TimingProbeScenario::SenseStress),
            "stop-start" => Ok(TimingProbeScenario::StopStart),
            "encoder" | "encoder-stress" => Ok(TimingProbeScenario::EncoderStress),
            "mute" | "mute-stress" | "fn-play" => Ok(TimingProbeScenario::MuteStress),
            "dance-page" | "dance-pages" | "dance-page-stress" => {
                Ok(TimingProbeScenario::DancePageStress)
            }
            other => Err(format!("unknown scenario {other}")),
        })
        .collect()
}

pub fn parse_timing_probe_durations(value: &str) -> Result<Vec<Duration>, String> {
    value.split(',').map(parse_duration).collect()
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let trimmed = value.trim();
    let (number, multiplier) = trimmed
        .strip_suffix('m')
        .map(|n| (n, 60))
        .or_else(|| trimmed.strip_suffix('s').map(|n| (n, 1)))
        .ok_or_else(|| format!("duration must end in s or m: {trimmed}"))?;
    Ok(Duration::from_secs(
        number
            .parse::<u64>()
            .map_err(|_| format!("invalid duration {trimmed}"))?
            * multiplier,
    ))
}
