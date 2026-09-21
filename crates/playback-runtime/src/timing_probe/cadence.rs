use super::{ProbeHost, ProbeRunner, SendMetric};
use crate::{HostMessage, PlaybackRuntime, TimingProbeScenario};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AdvanceCorrelation {
    pub(super) pulses: u32,
    pub(super) event_producing: bool,
}

pub(super) fn observe_advance(sends: &[SendMetric], batches: &[usize]) -> AdvanceCorrelation {
    AdvanceCorrelation {
        pulses: sends
            .iter()
            .filter_map(|send| send.pulses)
            .fold(0, u32::saturating_add),
        event_producing: batches.iter().any(|batch| *batch > 0),
    }
}

pub(super) fn wake_endpoints(measured_duration_ms: u64, wake_interval_ms: u64) -> Vec<u64> {
    let mut endpoints = Vec::new();
    let mut endpoint_ms = wake_interval_ms;
    while endpoint_ms <= measured_duration_ms {
        endpoints.push(endpoint_ms);
        endpoint_ms = endpoint_ms.saturating_add(wake_interval_ms);
    }
    endpoints
}

pub(super) fn crossed_boundaries(
    previous_ms: u64,
    current_ms: u64,
    period_ms: u64,
    offset_ms: u64,
) -> Vec<u64> {
    let Some(mut boundary_ms) = (if current_ms < offset_ms || period_ms == 0 {
        None
    } else if previous_ms < offset_ms {
        Some(offset_ms)
    } else {
        let steps = previous_ms
            .saturating_sub(offset_ms)
            .checked_div(period_ms)
            .unwrap_or(0)
            .saturating_add(1);
        steps
            .checked_mul(period_ms)
            .and_then(|distance| offset_ms.checked_add(distance))
    }) else {
        return Vec::new();
    };
    let mut boundaries = Vec::new();
    while boundary_ms <= current_ms {
        boundaries.push(boundary_ms);
        let Some(next_boundary_ms) = boundary_ms.checked_add(period_ms) else {
            break;
        };
        boundary_ms = next_boundary_ms;
    }
    boundaries
}

fn pulses_stress_actions(previous_ms: u64, current_ms: u64) -> Vec<(u64, u8)> {
    let mut actions = crossed_boundaries(previous_ms, current_ms, 250, 20)
        .into_iter()
        .map(|boundary_ms| (boundary_ms, 0_u8))
        .chain(
            crossed_boundaries(previous_ms, current_ms, 250, 120)
                .into_iter()
                .map(|boundary_ms| (boundary_ms, 1_u8)),
        )
        .collect::<Vec<_>>();
    actions.sort_by_key(|(boundary_ms, action)| (*boundary_ms, *action));
    actions
}

fn encoder_delta(boundary_ms: u64) -> i32 {
    if (boundary_ms / 40).is_multiple_of(2) {
        1
    } else {
        -1
    }
}

fn play_page(boundary_ms: u64) -> usize {
    ((boundary_ms / 250) % 5) as usize
}

pub(super) fn apply_scenario(
    scenario: TimingProbeScenario,
    now_ms: u64,
    previous_ms: u64,
    snapshots: bool,
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
) -> Result<(), String> {
    match scenario {
        TimingProbeScenario::Idle => Ok(()),
        TimingProbeScenario::PulsesStress if now_ms == 0 => send_input(
            runtime,
            runner,
            host,
            serde_json::json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::PulsesStress => {
            for (_, action) in pulses_stress_actions(previous_ms, now_ms) {
                send_input(
                    runtime,
                    runner,
                    host,
                    if action == 0 {
                        serde_json::json!({ "type": "encoder_press", "id": "main" })
                    } else {
                        serde_json::json!({ "type": "button_a", "pressed": true })
                    },
                    snapshots,
                )?;
            }
            Ok(())
        }
        TimingProbeScenario::StopStart => {
            for _ in crossed_boundaries(previous_ms, now_ms, 1000, 1000) {
                send_input(
                    runtime,
                    runner,
                    host,
                    serde_json::json!({ "type": "button_s", "pressed": true }),
                    true,
                )?;
            }
            Ok(())
        }
        TimingProbeScenario::EncoderStress if now_ms == 0 => send_input(
            runtime,
            runner,
            host,
            serde_json::json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::EncoderStress => {
            for boundary_ms in crossed_boundaries(previous_ms, now_ms, 40, 0) {
                send_input(
                    runtime,
                    runner,
                    host,
                    serde_json::json!({ "type": "encoder_turn", "delta": encoder_delta(boundary_ms), "id": "main" }),
                    snapshots,
                )?;
            }
            Ok(())
        }
        TimingProbeScenario::MuteStress if now_ms == 0 => send_fn_play(runtime, runner, host),
        TimingProbeScenario::MuteStress => {
            for _ in crossed_boundaries(previous_ms, now_ms, 500, 0) {
                send_fn_play(runtime, runner, host)?;
            }
            Ok(())
        }
        TimingProbeScenario::PlayPageStress if now_ms == 0 => {
            send_play_page_input(runtime, runner, host, 0)
        }
        TimingProbeScenario::PlayPageStress => {
            for boundary_ms in crossed_boundaries(previous_ms, now_ms, 250, 0) {
                send_play_page_input(runtime, runner, host, play_page(boundary_ms))?;
            }
            Ok(())
        }
    }
}

fn send_play_page_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    y: usize,
) -> Result<(), String> {
    send_input(
        runtime,
        runner,
        host,
        serde_json::json!({ "type": "button_fn", "pressed": true }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        serde_json::json!({ "type": "grid_press", "x": 7, "y": y }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        serde_json::json!({ "type": "button_fn", "pressed": false }),
        false,
    )
}

fn send_fn_play(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
) -> Result<(), String> {
    send_input(
        runtime,
        runner,
        host,
        serde_json::json!({ "type": "button_fn", "pressed": true }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        serde_json::json!({ "type": "button_s", "pressed": true }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        serde_json::json!({ "type": "button_fn", "pressed": false }),
        false,
    )
}

fn send_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    input: Value,
    snapshots: bool,
) -> Result<(), String> {
    super::send_runtime_message(
        runtime,
        runner,
        host,
        HostMessage::DeviceInput {
            input,
            request_snapshot: Some(snapshots),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::super::timing_probe_report::intervals;
    use super::super::{ProbeHost, ProbeRunner, SendMetric};
    use super::{
        apply_scenario, crossed_boundaries, encoder_delta, play_page, pulses_stress_actions,
        wake_endpoints,
    };
    use crate::{
        HostAdapter, MusicalEvent, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
        RuntimeConfig, TimingProbeOptions, TimingProbeScenario,
    };
    use std::time::Duration;

    #[test]
    fn crossed_boundaries_replay_each_timestamp_in_order() {
        assert_eq!(crossed_boundaries(36, 42, 40, 0), vec![40]);
        assert_eq!(crossed_boundaries(42, 48, 40, 0), Vec::<u64>::new());
        assert_eq!(crossed_boundaries(258, 515, 250, 20), vec![270]);
        assert_eq!(crossed_boundaries(996, 1008, 1000, 1000), vec![1000]);
        assert_eq!(crossed_boundaries(900, 2_100, 1000, 1000), vec![1000, 2000]);
    }

    #[test]
    fn crossed_pulses_actions_interleave_press_and_button_boundaries() {
        assert_eq!(
            pulses_stress_actions(0, 520),
            vec![(20, 0), (120, 1), (270, 0), (370, 1), (520, 0)]
        );
    }

    #[test]
    fn crossed_encoder_boundaries_derive_alternation_from_each_timestamp() {
        let deltas = crossed_boundaries(0, 121, 40, 0)
            .into_iter()
            .map(encoder_delta)
            .collect::<Vec<_>>();

        assert_eq!(deltas, vec![-1, 1, -1]);
        assert_eq!(
            crossed_boundaries(0, 1000, 250, 0)
                .into_iter()
                .map(play_page)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn wake_endpoints_cover_the_requested_interval_through_measured_duration() {
        assert_eq!(wake_endpoints(13, 2), vec![2, 4, 6, 8, 10, 12]);
        assert_eq!(wake_endpoints(13, 4), vec![4, 8, 12]);
        assert_eq!(wake_endpoints(1, 2), Vec::<u64>::new());
    }

    #[test]
    fn logical_event_timestamps_and_intervals_follow_advance_endpoints() {
        let endpoints = wake_endpoints(12, 4);
        let mut host = ProbeHost::default();
        for endpoint_ms in endpoints {
            host.now_ms = endpoint_ms;
            host.handle_musical_event(&MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            })
            .unwrap();
        }

        assert_eq!(host.event_times_ms, vec![4, 8, 12]);
        assert_eq!(intervals(&host.event_times_ms), vec![4.0, 4.0]);
    }

    #[test]
    fn crossing_stress_boundary_emits_one_action_without_repeating_at_next_wake() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = ProbeRunner {
            inner: NativeRunner::new(NativeRunnerConfig::default()).unwrap(),
            sends: Vec::<SendMetric>::new(),
            batches: Vec::new(),
            advance_correlations: Vec::new(),
            playing_statuses: 0,
        };
        let mut host = ProbeHost::default();
        apply_scenario(
            TimingProbeScenario::EncoderStress,
            0,
            0,
            false,
            &mut runtime,
            &mut runner,
            &mut host,
        )
        .unwrap();
        let after_start = runner.sends.len();
        apply_scenario(
            TimingProbeScenario::EncoderStress,
            42,
            0,
            false,
            &mut runtime,
            &mut runner,
            &mut host,
        )
        .unwrap();
        let after_crossing = runner.sends.len();
        apply_scenario(
            TimingProbeScenario::EncoderStress,
            48,
            42,
            false,
            &mut runtime,
            &mut runner,
            &mut host,
        )
        .unwrap();

        assert!(after_crossing > after_start);
        assert_eq!(runner.sends.len(), after_crossing);
    }

    #[test]
    fn stress_scenarios_count_crossed_actions_without_wake_alignment() {
        assert_eq!(action_count(TimingProbeScenario::PulsesStress, 258, 515), 2);
        assert_eq!(action_count(TimingProbeScenario::StopStart, 996, 1108), 1);
        assert_eq!(action_count(TimingProbeScenario::EncoderStress, 36, 42), 1);
        assert_eq!(action_count(TimingProbeScenario::MuteStress, 492, 504), 3);
        assert_eq!(
            action_count(TimingProbeScenario::PlayPageStress, 242, 256),
            3
        );
    }

    #[test]
    fn large_wakes_replay_all_crossed_scenario_actions() {
        assert_eq!(action_count(TimingProbeScenario::PulsesStress, 0, 1000), 8);
        assert_eq!(action_count(TimingProbeScenario::StopStart, 0, 2050), 2);
        assert_eq!(
            action_count(TimingProbeScenario::EncoderStress, 0, 1000),
            25
        );
        assert_eq!(
            action_count(TimingProbeScenario::PlayPageStress, 0, 1000),
            12
        );
    }

    fn action_count(scenario: TimingProbeScenario, previous_ms: u64, current_ms: u64) -> usize {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = ProbeRunner {
            inner: NativeRunner::new(NativeRunnerConfig::default()).unwrap(),
            sends: Vec::<SendMetric>::new(),
            batches: Vec::new(),
            advance_correlations: Vec::new(),
            playing_statuses: 0,
        };
        let mut host = ProbeHost::default();
        apply_scenario(scenario, 0, 0, false, &mut runtime, &mut runner, &mut host).unwrap();
        let before = runner.sends.len();
        apply_scenario(
            scenario,
            current_ms,
            previous_ms,
            false,
            &mut runtime,
            &mut runner,
            &mut host,
        )
        .unwrap();
        runner.sends.len() - before
    }

    #[test]
    fn probe_reports_have_exact_scenario_duration_and_wake_matrix_identity() {
        let reports = super::super::run_timing_probe(&TimingProbeOptions {
            durations: vec![Duration::from_millis(13)],
            scenarios: vec![
                TimingProbeScenario::Idle,
                TimingProbeScenario::EncoderStress,
            ],
            wake_intervals_ms: vec![2, 4, 6, 8, 10, 12],
            ..TimingProbeOptions::default()
        })
        .unwrap();

        assert_eq!(reports.len(), 12);
        assert_eq!(
            reports
                .iter()
                .map(|report| {
                    (
                        report.scenario,
                        report.wake_interval_ms,
                        report.measured_duration_ms,
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (TimingProbeScenario::Idle, 2, 12),
                (TimingProbeScenario::Idle, 4, 12),
                (TimingProbeScenario::Idle, 6, 12),
                (TimingProbeScenario::Idle, 8, 8),
                (TimingProbeScenario::Idle, 10, 10),
                (TimingProbeScenario::Idle, 12, 12),
                (TimingProbeScenario::EncoderStress, 2, 12),
                (TimingProbeScenario::EncoderStress, 4, 12),
                (TimingProbeScenario::EncoderStress, 6, 12),
                (TimingProbeScenario::EncoderStress, 8, 8),
                (TimingProbeScenario::EncoderStress, 10, 10),
                (TimingProbeScenario::EncoderStress, 12, 12),
            ]
        );
        assert!(reports.iter().all(|report| report.duration_ms == 13));
    }

    #[test]
    fn six_second_idle_probe_matrix_preserves_pulses_and_event_counts() {
        let reports = super::super::run_timing_probe(&TimingProbeOptions {
            durations: vec![Duration::from_secs(6)],
            scenarios: vec![TimingProbeScenario::Idle],
            wake_intervals_ms: vec![2, 4, 6, 8, 10, 12],
            ..TimingProbeOptions::default()
        })
        .unwrap();

        assert_eq!(
            reports
                .iter()
                .map(|report| report.events)
                .collect::<Vec<_>>(),
            vec![84; 6]
        );
        assert_eq!(
            reports
                .iter()
                .map(|report| report.pulses_per_advance.count)
                .collect::<Vec<_>>(),
            vec![288; 6]
        );
        assert_eq!(
            reports
                .iter()
                .map(|report| report.playing_statuses)
                .collect::<Vec<_>>(),
            vec![288; 6]
        );
        assert!(reports.iter().all(|report| {
            report.multi_pulse_advances == 0
                && report.pulses_per_advance.min == 1.0
                && report.pulses_per_advance.max == 1.0
        }));
    }
}
