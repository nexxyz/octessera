use super::{
    timing_probe_cadence::AdvanceCorrelation, EventRecord, TimingProbeCountSummary,
    TimingProbeReport, TimingProbeScenario, TimingProbeStreamReport, TimingProbeSummary,
};
use crate::MusicalEvent;
use std::time::Duration;

const MAX_WAKE_INTERVAL_MS: u64 = 1_000;

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
        intervals_ms: summarize_ms(&intervals),
        first_window_interval_ms: summarize_ms(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_ms: summarize_ms(
            &intervals
                .iter()
                .rev()
                .take(window)
                .copied()
                .collect::<Vec<_>>(),
        ),
    })
}

pub(super) fn summarize_counts(values: &[usize]) -> TimingProbeCountSummary {
    summarize_count_values(&values.iter().map(|value| *value as f64).collect::<Vec<_>>())
}

pub(super) fn summarize_pulse_counts(values: &[f64]) -> TimingProbeCountSummary {
    summarize_count_values(values)
}

#[derive(Default)]
pub(super) struct AdvanceCorrelationSummary {
    pub(super) event_producing_advances: usize,
    pub(super) multi_pulse_advances: usize,
    pub(super) multi_pulse_event_producing_advances: usize,
    pub(super) pulses_on_event_producing_advances: TimingProbeCountSummary,
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

pub(super) fn summarize_ms(values: &[f64]) -> TimingProbeSummary {
    summarize_time(values, [1.0, 5.0, 10.0, 20.0])
}

pub(super) fn summarize_us(values: &[f64]) -> TimingProbeSummary {
    summarize_time(values, [1_000.0, 5_000.0, 10_000.0, 20_000.0])
}

fn summarize_time(values: &[f64], thresholds: [f64; 4]) -> TimingProbeSummary {
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
        over_1ms: values
            .iter()
            .filter(|value| **value > thresholds[0])
            .count(),
        over_5ms: values
            .iter()
            .filter(|value| **value > thresholds[1])
            .count(),
        over_10ms: values
            .iter()
            .filter(|value| **value > thresholds[2])
            .count(),
        over_20ms: values
            .iter()
            .filter(|value| **value > thresholds[3])
            .count(),
    }
}

fn summarize_count_values(values: &[f64]) -> TimingProbeCountSummary {
    if values.is_empty() {
        return TimingProbeCountSummary::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    TimingProbeCountSummary {
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

pub fn print_timing_probe_summary(reports: &[TimingProbeReport]) {
    for report in reports {
        eprintln!("{:?} {}ms measured={}ms wake={}ms realtime={} events={} playing_statuses={} event_advances={} multi_pulse_advances={} multi_pulse_event_advances={} event_pulses_mean={:.2} interval_mean={:.2}ms p95={:.2}ms send_p95={:.0}us advance_p95={:.0}us wake_late_p95={:.0}us loop_p95={:.0}us batch_max={:.0}", report.scenario, report.duration_ms, report.measured_duration_ms, report.wake_interval_ms, report.realtime, report.events, report.playing_statuses, report.event_producing_advances, report.multi_pulse_advances, report.multi_pulse_event_producing_advances, report.pulses_on_event_producing_advances.mean, report.event_intervals_ms.mean, report.event_intervals_ms.p95, report.runner_send_us.p95, report.advance_us.p95, report.wake_late_us.p95, report.loop_us.p95, report.event_batches.max);
    }
}

pub fn parse_timing_probe_scenarios(value: &str) -> Result<Vec<TimingProbeScenario>, String> {
    value
        .split(',')
        .map(|item| match item.trim() {
            "idle" => Ok(TimingProbeScenario::Idle),
            "pulses-stress" => Ok(TimingProbeScenario::PulsesStress),
            "stop-start" => Ok(TimingProbeScenario::StopStart),
            "encoder" | "encoder-stress" => Ok(TimingProbeScenario::EncoderStress),
            "mute" | "mute-stress" | "fn-play" => Ok(TimingProbeScenario::MuteStress),
            "play-page" | "play-pages" | "play-page-stress" => {
                Ok(TimingProbeScenario::PlayPageStress)
            }
            other => Err(format!("unknown scenario {other}")),
        })
        .collect()
}

pub fn parse_timing_probe_durations(value: &str) -> Result<Vec<Duration>, String> {
    value.split(',').map(parse_duration).collect()
}

pub fn parse_timing_probe_wake_intervals_ms(value: &str) -> Result<Vec<u64>, String> {
    if value.trim().is_empty() {
        return Err("wake intervals must not be empty".into());
    }
    value
        .split(',')
        .enumerate()
        .map(|(index, item)| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return Err(format!("wake interval {} is empty", index + 1));
            }
            let interval = trimmed.parse::<u64>().map_err(|_| {
                format!(
                    "invalid wake interval {trimmed:?} at position {}; expected an integer",
                    index + 1
                )
            })?;
            if interval == 0 {
                return Err(format!("wake interval {} must be positive", index + 1));
            }
            if interval > MAX_WAKE_INTERVAL_MS {
                return Err(format!(
                    "wake interval {} is {interval}ms; maximum is {MAX_WAKE_INTERVAL_MS}ms",
                    index + 1
                ));
            }
            Ok(interval)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::super::timing_probe_cadence::AdvanceCorrelation;
    use super::{
        parse_timing_probe_wake_intervals_ms, summarize_advance_correlations, summarize_counts,
        summarize_ms, summarize_us,
    };

    #[test]
    fn time_summary_thresholds_follow_the_input_unit() {
        let milliseconds = summarize_ms(&[0.5, 1.5, 5.5, 10.5, 20.5]);
        let microseconds = summarize_us(&[500.0, 1_500.0, 5_500.0, 10_500.0, 20_500.0]);

        for summary in [milliseconds, microseconds] {
            assert_eq!(summary.over_1ms, 4);
            assert_eq!(summary.over_5ms, 3);
            assert_eq!(summary.over_10ms, 2);
            assert_eq!(summary.over_20ms, 1);
        }
    }

    #[test]
    fn count_summary_has_no_time_threshold_fields() {
        let object = serde_json::to_value(summarize_counts(&[1, 1_001]))
            .unwrap()
            .as_object()
            .unwrap()
            .clone();

        assert_eq!(object.len(), 8);
        for key in ["over_1ms", "over_5ms", "over_10ms", "over_20ms"] {
            assert!(!object.contains_key(key), "unexpected {key}");
        }
    }

    #[test]
    fn wake_interval_parser_accepts_trimmed_positive_bounded_integers() {
        assert_eq!(
            parse_timing_probe_wake_intervals_ms(" 2,4, 6 ").unwrap(),
            vec![2, 4, 6]
        );
    }

    #[test]
    fn wake_interval_parser_rejects_empty_nonpositive_invalid_and_unbounded_values() {
        for (value, message) in [
            ("", "must not be empty"),
            ("2,,4", "wake interval 2 is empty"),
            ("0", "must be positive"),
            ("fast", "expected an integer"),
            ("1001", "maximum is 1000ms"),
        ] {
            let error = parse_timing_probe_wake_intervals_ms(value).unwrap_err();
            assert!(error.contains(message), "{value:?}: {error}");
        }
    }

    #[test]
    fn correlation_summary_counts_event_and_multi_pulse_advances() {
        let summary = summarize_advance_correlations(&[
            AdvanceCorrelation {
                pulses: 1,
                event_producing: false,
            },
            AdvanceCorrelation {
                pulses: 2,
                event_producing: true,
            },
            AdvanceCorrelation {
                pulses: 3,
                event_producing: true,
            },
            AdvanceCorrelation {
                pulses: 4,
                event_producing: false,
            },
        ]);

        assert_eq!(summary.event_producing_advances, 2);
        assert_eq!(summary.multi_pulse_advances, 3);
        assert_eq!(summary.multi_pulse_event_producing_advances, 2);
        assert_eq!(summary.pulses_on_event_producing_advances.count, 2);
        assert_eq!(summary.pulses_on_event_producing_advances.min, 2.0);
        assert_eq!(summary.pulses_on_event_producing_advances.mean, 2.5);
        assert_eq!(summary.pulses_on_event_producing_advances.max, 3.0);
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
    }
}
