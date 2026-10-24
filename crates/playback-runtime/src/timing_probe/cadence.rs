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
mod tests;
