use super::live_probe::{LiveProbeHost, LiveProbeRunner, LiveSendRecord};
use super::{send_device_input, send_fn_play, send_play_page_input};
use playback_runtime::{PlaybackRuntime, TimingProbeScenario};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AdvanceCorrelation {
    pub(super) pulses: u32,
    pub(super) event_producing: bool,
}

pub(super) fn observe_advance(sends: &[LiveSendRecord], batches: &[usize]) -> AdvanceCorrelation {
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

pub(super) struct LiveCadence {
    epoch: Instant,
    last_tick: Instant,
}

impl LiveCadence {
    pub(super) fn new() -> Self {
        let epoch = Instant::now();
        Self {
            epoch,
            last_tick: epoch,
        }
    }

    pub(super) fn target(&self, endpoint_ms: u64) -> Instant {
        self.epoch + Duration::from_millis(endpoint_ms)
    }

    pub(super) fn elapsed_until(&mut self, now: Instant) -> Duration {
        let elapsed = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        elapsed
    }

    #[cfg(test)]
    fn at(epoch: Instant) -> Self {
        Self {
            epoch,
            last_tick: epoch,
        }
    }
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

pub(super) fn pulses_stress_actions(previous_ms: u64, current_ms: u64) -> Vec<(u64, u8)> {
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

fn stop_start_actions(previous_ms: u64, current_ms: u64) -> Vec<(u64, bool)> {
    let mut actions = crossed_boundaries(previous_ms, current_ms, 1000, 1000)
        .into_iter()
        .map(|boundary_ms| (boundary_ms, true))
        .chain(
            crossed_boundaries(previous_ms, current_ms, 1000, 1100)
                .into_iter()
                .map(|boundary_ms| (boundary_ms, false)),
        )
        .collect::<Vec<_>>();
    actions.sort_by_key(|(boundary_ms, is_stop)| (*boundary_ms, !*is_stop));
    actions
}

pub(super) fn encoder_delta(boundary_ms: u64) -> i32 {
    if (boundary_ms / 40).is_multiple_of(2) {
        1
    } else {
        -1
    }
}

pub(super) fn play_page(boundary_ms: u64) -> usize {
    ((boundary_ms / 250) % 5) as usize
}

pub(super) fn apply_scenario(
    scenario: TimingProbeScenario,
    now_ms: u64,
    previous_ms: u64,
    snapshots: bool,
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
) -> Result<(), String> {
    match scenario {
        TimingProbeScenario::Idle => Ok(()),
        TimingProbeScenario::PulsesStress if now_ms == 0 => send_device_input(
            playback,
            runner,
            host,
            serde_json::json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::PulsesStress => {
            for (_, action) in pulses_stress_actions(previous_ms, now_ms) {
                send_device_input(
                    playback,
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
            for (_, is_stop) in stop_start_actions(previous_ms, now_ms) {
                super::send_runtime_message(
                    playback,
                    runner,
                    host,
                    if is_stop {
                        playback_runtime::HostMessage::MidiRealtimeStop
                    } else {
                        playback_runtime::HostMessage::MidiRealtimeStart
                    },
                )?;
            }
            Ok(())
        }
        TimingProbeScenario::EncoderStress if now_ms == 0 => send_device_input(
            playback,
            runner,
            host,
            serde_json::json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::EncoderStress => {
            for boundary_ms in crossed_boundaries(previous_ms, now_ms, 40, 0) {
                send_device_input(
                    playback,
                    runner,
                    host,
                    serde_json::json!({ "type": "encoder_turn", "delta": encoder_delta(boundary_ms), "id": "main" }),
                    snapshots,
                )?;
            }
            Ok(())
        }
        TimingProbeScenario::MuteStress if now_ms == 0 => send_fn_play(playback, runner, host),
        TimingProbeScenario::MuteStress => {
            for _ in crossed_boundaries(previous_ms, now_ms, 500, 0) {
                send_fn_play(playback, runner, host)?;
            }
            Ok(())
        }
        TimingProbeScenario::PlayPageStress if now_ms == 0 => {
            send_play_page_input(playback, runner, host, 0)
        }
        TimingProbeScenario::PlayPageStress => {
            for boundary_ms in crossed_boundaries(previous_ms, now_ms, 250, 0) {
                send_play_page_input(playback, runner, host, play_page(boundary_ms))?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        crossed_boundaries, encoder_delta, play_page, pulses_stress_actions, stop_start_actions,
        wake_endpoints, LiveCadence,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn wake_endpoints_cover_the_requested_interval_through_measured_duration() {
        assert_eq!(wake_endpoints(13, 2), vec![2, 4, 6, 8, 10, 12]);
        assert_eq!(wake_endpoints(13, 4), vec![4, 8, 12]);
        assert_eq!(wake_endpoints(1, 2), Vec::<u64>::new());
    }

    #[test]
    fn live_cadence_ignores_setup_time_and_reaches_the_final_endpoint() {
        let setup_started = Instant::now();
        let cadence_epoch = setup_started + Duration::from_millis(137);
        let endpoints = wake_endpoints(13, 4);
        let mut cadence = LiveCadence::at(cadence_epoch);
        let mut previous_endpoint = 0;
        let mut cumulative_elapsed = Duration::ZERO;

        for endpoint_ms in endpoints {
            let target = cadence.target(endpoint_ms);
            assert_eq!(
                target.duration_since(cadence_epoch),
                Duration::from_millis(endpoint_ms)
            );
            let elapsed = cadence.elapsed_until(target);
            assert_eq!(
                elapsed,
                Duration::from_millis(endpoint_ms - previous_endpoint)
            );
            cumulative_elapsed += elapsed;
            previous_endpoint = endpoint_ms;
        }

        assert_eq!(cumulative_elapsed, Duration::from_millis(12));
        assert_eq!(previous_endpoint, 12);
    }

    #[test]
    fn crossed_boundaries_replay_each_timestamp_in_order() {
        assert_eq!(crossed_boundaries(36, 42, 40, 0), vec![40]);
        assert_eq!(crossed_boundaries(258, 515, 250, 20), vec![270]);
        assert_eq!(crossed_boundaries(258, 515, 250, 120), vec![370]);
        assert_eq!(crossed_boundaries(492, 504, 500, 0), vec![500]);
        assert_eq!(crossed_boundaries(996, 1008, 1000, 1000), vec![1000]);
        assert_eq!(
            crossed_boundaries(1008, 1014, 1000, 1000),
            Vec::<u64>::new()
        );
        assert_eq!(crossed_boundaries(1096, 1108, 1000, 1100), vec![1100]);
    }

    #[test]
    fn crossed_actions_preserve_pulses_interleaving_and_encoder_alternation() {
        assert_eq!(
            pulses_stress_actions(0, 520),
            vec![(20, 0), (120, 1), (270, 0), (370, 1), (520, 0)]
        );
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
    fn stop_start_actions_remain_chronological_when_a_wake_crosses_both_phases() {
        assert_eq!(
            stop_start_actions(1000, 2_000),
            vec![(1100, false), (2000, true)]
        );
    }
}
