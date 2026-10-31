use super::support::FakeHost;
use crate::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimeIngest, RuntimeTransportState, SyncSource,
};
use serde_json::Value;
use std::time::Instant;

fn stopped_default_playback() -> (PlaybackRuntime, NativeRunner, FakeHost) {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let startup = runner.messages_with_snapshot().unwrap();
    runtime
        .dispatch_runner_messages(startup, &mut runner, &mut host)
        .unwrap();
    assert_eq!(
        runtime.last_status().unwrap().transport,
        RuntimeTransportState::Stopped
    );
    (runtime, runner, host)
}

/// The message the Pi sends for a stopped display deadline (toast expiry, dim, sleep, save flash).
fn deadline_snapshot(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
) -> RuntimeIngest {
    runtime
        .dispatch_host_message(
            HostMessage::TransportPulseStep {
                pulses: 0,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(true),
            },
            runner,
            host,
        )
        .unwrap()
}

fn runner_snapshot(messages: &[RunnerMessage]) -> &Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .expect("deadline snapshot")
}

/// Prints what one stopped display-deadline snapshot costs through the runtime; run with
/// `cargo test --release -p playback-runtime --lib stopped_deadline_snapshot_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn stopped_deadline_snapshot_cost() {
    const ROUNDS: u32 = 400;
    let (mut runtime, mut runner, mut host) = stopped_default_playback();
    for _ in 0..20 {
        deadline_snapshot(&mut runtime, &mut runner, &mut host);
    }
    let started = Instant::now();
    for _ in 0..ROUNDS {
        deadline_snapshot(&mut runtime, &mut runner, &mut host);
    }
    let elapsed = started.elapsed();
    let output = deadline_snapshot(&mut runtime, &mut runner, &mut host);
    let bytes = runner_snapshot(&output.messages).to_string().len();
    println!(
        "stopped deadline snapshot: {:?} per snapshot, {bytes} JSON bytes",
        elapsed / ROUNDS
    );
}
