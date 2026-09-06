use super::support::{set_runtime_playing, FakeHost, FakeRunner};
use crate::{
    HostMessage, PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimeStatus, RuntimeStatusState,
    RuntimeTransportState, SyncSource,
};

fn status(transport: RuntimeTransportState, pulse: u64, source: SyncSource) -> RunnerMessage {
    RunnerMessage::RuntimeStatus {
        status: RuntimeStatus {
            state: match transport {
                RuntimeTransportState::Playing => RuntimeStatusState::Running,
                RuntimeTransportState::Paused => RuntimeStatusState::Paused,
                RuntimeTransportState::Stopped => RuntimeStatusState::Idle,
            },
            transport,
            current_ppqn_pulse: pulse,
            pending_resync: false,
            sync_source: source,
            message: None,
            error: None,
        },
    }
}

fn pulse_steps(messages: &[HostMessage]) -> usize {
    messages
        .iter()
        .filter(|message| matches!(message, HostMessage::TransportPulseStep { pulses, .. } if *pulses > 0))
        .count()
}

fn summed_internal_pulses(messages: &[HostMessage]) -> u64 {
    messages
        .iter()
        .filter_map(|message| match message {
            HostMessage::TransportPulseStep {
                pulses,
                source: SyncSource::Internal,
                ..
            } => Some(u64::from(*pulses)),
            _ => None,
        })
        .sum()
}

#[test]
fn stop_and_origin_start_reset_fractional_remainder_but_pause_continue_preserves_it() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: false,
        ..RuntimeConfig::default()
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();
    runtime
        .dispatch_host_message(HostMessage::TransportStop, &mut runner, &mut host)
        .unwrap();
    set_runtime_playing(&mut runtime, &mut host);
    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();
    assert_eq!(pulse_steps(&runner.seen), 0);

    runtime
        .ingest_runner_messages(
            vec![status(
                RuntimeTransportState::Paused,
                0,
                SyncSource::Internal,
            )],
            &mut host,
        )
        .unwrap();
    runtime
        .ingest_runner_messages(
            vec![status(
                RuntimeTransportState::Playing,
                0,
                SyncSource::Internal,
            )],
            &mut host,
        )
        .unwrap();
    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();
    assert_eq!(pulse_steps(&runner.seen), 1);
}

#[test]
fn external_resync_ppqn_regression_resets_fractional_remainder() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: false,
        ..RuntimeConfig::default()
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages(
            vec![status(
                RuntimeTransportState::Playing,
                95,
                SyncSource::Internal,
            )],
            &mut host,
        )
        .unwrap();
    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();
    runtime
        .ingest_runner_messages(
            vec![status(
                RuntimeTransportState::Playing,
                0,
                SyncSource::External,
            )],
            &mut host,
        )
        .unwrap();
    runtime
        .ingest_runner_messages(
            vec![status(
                RuntimeTransportState::Playing,
                0,
                SyncSource::Internal,
            )],
            &mut host,
        )
        .unwrap();
    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();

    assert!(runner.seen.iter().all(|message| !matches!(
        message,
        HostMessage::TransportPulseStep { pulses, .. } if *pulses > 0
    )));
}

#[test]
fn explicit_midi_start_resets_fractional_remainder_without_clearing_duration_queue() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);
    runtime.advance(500, &mut runner, &mut host).unwrap();
    let pulse_count_before_start = pulse_steps(&runner.seen);
    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();
    runtime
        .dispatch_host_message(HostMessage::MidiRealtimeStart, &mut runner, &mut host)
        .unwrap();
    runtime
        .advance_duration(std::time::Duration::from_millis(11), &mut runner, &mut host)
        .unwrap();

    assert_eq!(pulse_steps(&runner.seen), pulse_count_before_start);
    assert!(runtime.has_scheduled_midi());
}

#[test]
fn fragmented_internal_clock_does_not_drift_over_ten_minutes() {
    let config = RuntimeConfig {
        bpm: 120.0,
        midi_out_enabled: false,
        ..RuntimeConfig::default()
    };
    let expected_duration = std::time::Duration::from_secs(10 * 60);
    let expected_pulses =
        (config.bpm * crate::runtime::PPQN * expected_duration.as_secs_f64() / 60.0) as u64;
    let mut runtime = PlaybackRuntime::new(config);
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    let chunks = [
        std::time::Duration::from_micros(8_900),
        std::time::Duration::from_micros(8_700),
        std::time::Duration::from_micros(8_500),
        std::time::Duration::from_micros(8_300),
        std::time::Duration::from_micros(8_100),
    ];
    let mut elapsed = std::time::Duration::ZERO;
    let mut chunk_index = 0;
    while elapsed < expected_duration {
        let remaining = expected_duration - elapsed;
        let candidate = chunks[chunk_index % chunks.len()];
        let chunk = if candidate < remaining {
            candidate
        } else {
            remaining
        };
        runtime
            .advance_duration(chunk, &mut runner, &mut host)
            .unwrap();
        elapsed += chunk;
        chunk_index += 1;
    }

    assert_eq!(elapsed, expected_duration);
    assert_eq!(summed_internal_pulses(&runner.seen), expected_pulses);
}
