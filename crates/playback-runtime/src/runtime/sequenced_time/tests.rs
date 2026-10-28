use crate::tests::support::FakeHost;
use crate::{
    CoreRunner, HostMessage, MusicalEvent, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimeStatus, RuntimeStatusState, RuntimeStoreResult, RuntimeTransportState, SyncSource,
};
use std::time::{Duration, Instant};

const D: Duration = Duration::from_millis(8);
const NOTE: MusicalEvent = MusicalEvent::NoteOn {
    channel: 0,
    note: 60,
    velocity: 100,
    duration_ms: None,
};

#[derive(Default)]
struct PulseRunner {
    pulse: u64,
}

impl CoreRunner for PulseRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let (pulses, source) = match message {
            HostMessage::TransportPulseStep { pulses, .. } => (pulses, SyncSource::Internal),
            HostMessage::MidiRealtimeClock { pulses } => (pulses, SyncSource::External),
            _ => return Ok(Vec::new()),
        };
        self.pulse += u64::from(pulses);
        let events = if pulses > 0 { vec![NOTE] } else { Vec::new() };
        Ok(vec![
            RunnerMessage::MusicalEvents { events },
            status_from(RuntimeTransportState::Playing, self.pulse, source),
        ])
    }

    fn send_system_store_result(
        &mut self,
        _message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>), String> {
        Ok((Vec::new(), None))
    }
}

fn status(transport: RuntimeTransportState, pulse: u64) -> RunnerMessage {
    status_from(transport, pulse, SyncSource::Internal)
}

fn status_from(
    transport: RuntimeTransportState,
    pulse: u64,
    sync_source: SyncSource,
) -> RunnerMessage {
    RunnerMessage::RuntimeStatus {
        status: RuntimeStatus {
            state: RuntimeStatusState::Running,
            transport,
            current_ppqn_pulse: pulse,
            pending_resync: false,
            sync_source,
            message: None,
            error: None,
        },
    }
}

fn started(midi: bool) -> (PlaybackRuntime, FakeHost, Instant) {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: midi,
        midi_out_enabled: midi,
    });
    let origin = Instant::now();
    let mut host = FakeHost {
        clock: Some(origin),
        ..FakeHost::default()
    };
    runtime
        .ingest_runner_messages(vec![status(RuntimeTransportState::Stopped, 0)], &mut host)
        .unwrap();
    runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::MusicalEvents { events: vec![NOTE] },
                status(RuntimeTransportState::Playing, 0),
            ],
            &mut host,
        )
        .unwrap();
    (runtime, host, origin)
}

fn note_offsets_after(chunks: &[u64]) -> Vec<Duration> {
    let (mut runtime, mut host, origin) = started(false);
    let mut runner = PulseRunner::default();
    for chunk in chunks {
        runtime
            .advance_duration(Duration::from_micros(*chunk), &mut runner, &mut host)
            .unwrap();
    }
    host.musical_event_at
        .into_iter()
        .map(|at| at.unwrap() - origin)
        .collect()
}

fn pulse_offset(pulse: u64) -> Duration {
    Duration::from_nanos((pulse * 125_000_000).div_ceil(6)) + D
}

#[test]
fn pulse_instants_sit_on_the_exact_grid_for_any_advance_chunking() {
    let fine = note_offsets_after(&[1_000; 300]);
    let coarse = note_offsets_after(&[7_300, 21_100, 3_000, 50_000, 64_000, 8_000, 80_000, 66_600]);

    assert_eq!(fine, coarse);
    assert_eq!(fine, (0..15).map(pulse_offset).collect::<Vec<_>>());
}

#[test]
fn start_downbeat_and_clock_are_stamped_on_the_pulse_grid() {
    let (mut runtime, mut host, origin) = started(true);
    let mut runner = PulseRunner::default();

    assert_eq!(host.musical_event_at, vec![Some(origin + D)]);
    assert_eq!(
        host.midi_messages[..3],
        [vec![0x90, 60, 100], vec![0xFA], vec![0xF8]]
    );
    assert_eq!(host.midi_message_at[..3], [Some(origin + D); 3]);

    host.midi_messages.clear();
    host.midi_message_at.clear();
    for _ in 0..10 {
        runtime
            .advance_duration(Duration::from_millis(9), &mut runner, &mut host)
            .unwrap();
    }
    let clocks: Vec<_> = host
        .midi_messages
        .iter()
        .zip(&host.midi_message_at)
        .filter(|(bytes, _)| bytes.as_slice() == [0xF8])
        .map(|(_, at)| at.unwrap() - origin)
        .collect();
    assert_eq!(clocks, (1..=4).map(pulse_offset).collect::<Vec<_>>());
}

#[test]
fn output_outside_pulses_and_starts_is_untimed() {
    let (mut runtime, mut host, _) = started(true);
    host.musical_event_at.clear();
    host.midi_message_at.clear();

    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::MusicalEvents { events: vec![NOTE] }],
            &mut host,
        )
        .unwrap();

    assert_eq!(host.musical_event_at, vec![None]);
    assert_eq!(host.midi_message_at, vec![None]);
}

#[test]
fn stop_flushes_timed_midi_before_sending_stop_now() {
    let (mut runtime, mut host, _) = started(true);
    host.midi_messages.clear();
    host.midi_message_at.clear();

    runtime
        .ingest_runner_messages(vec![status(RuntimeTransportState::Stopped, 0)], &mut host)
        .unwrap();

    assert_eq!(host.midi_flushes, 1);
    assert_eq!(host.midi_messages, vec![vec![0xFC]]);
    assert_eq!(host.midi_message_at, vec![None]);
}

#[test]
fn catch_up_reaches_the_runner_as_one_step_stamped_like_its_last_pulse() {
    assert_eq!(
        note_offsets_after(&[500_000]),
        vec![pulse_offset(0), pulse_offset(24)]
    );
}

#[test]
fn time_until_next_pulse_follows_the_phase_while_playing() {
    let (mut runtime, mut host, _) = started(false);
    let mut runner = PulseRunner::default();
    runtime
        .advance_duration(Duration::from_millis(51), &mut runner, &mut host)
        .unwrap();

    assert_eq!(
        runtime.time_until_next_pulse(),
        Some(Duration::from_micros(11_500))
    );
    runtime
        .ingest_runner_messages(vec![status(RuntimeTransportState::Stopped, 0)], &mut host)
        .unwrap();
    assert_eq!(runtime.time_until_next_pulse(), None);
}

#[test]
fn external_clock_output_is_stamped_from_each_clock_arrival() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        sync_source: SyncSource::External,
        ..RuntimeConfig::default()
    });
    let mut runner = PulseRunner::default();
    let mut host = FakeHost::default();
    let first = Instant::now();
    let second = first + Duration::from_micros(20_833);

    for received_at in [Some(first), Some(second), None] {
        runtime
            .handle_midi_realtime_bytes_with_output(&[0xF8], received_at, &mut runner, &mut host)
            .unwrap();
    }

    assert_eq!(
        host.musical_event_at,
        vec![Some(first + D), Some(second + D), None]
    );
}
