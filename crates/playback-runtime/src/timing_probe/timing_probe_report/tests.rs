use super::super::cadence::AdvanceCorrelation;
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
