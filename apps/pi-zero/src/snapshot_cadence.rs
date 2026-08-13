use playback_runtime::NativeRunner;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SnapshotCadence {
    last_accepted_snapshot_at: Instant,
    last_observed_snapshot_revision: u64,
}

impl SnapshotCadence {
    pub(crate) fn new(now: Instant, initial_revision: u64) -> Self {
        Self {
            last_accepted_snapshot_at: now,
            last_observed_snapshot_revision: initial_revision,
        }
    }

    pub(crate) fn observe_accepted_snapshot(&mut self, now: Instant, snapshot_revision: u64) {
        if snapshot_revision == self.last_observed_snapshot_revision {
            return;
        }
        self.last_observed_snapshot_revision = snapshot_revision;
        self.last_accepted_snapshot_at = now;
    }

    pub(crate) fn periodic_due(&self, now: Instant, interval: Duration) -> bool {
        now.duration_since(self.last_accepted_snapshot_at) >= interval
    }

    pub(crate) fn timed_display_due(&self, now: Instant, runner: &NativeRunner) -> bool {
        self.next_timed_display_deadline(runner)
            .is_some_and(|deadline| now >= deadline)
    }

    pub(crate) fn next_timed_display_deadline(&self, runner: &NativeRunner) -> Option<Instant> {
        runner.next_timed_display_snapshot_deadline_after(Some(self.last_accepted_snapshot_at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playback_runtime::{
        CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunnerConfig, PlaybackRuntime,
        RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformRequest,
    };

    #[derive(Default)]
    struct TestHost;

    impl HostAdapter for TestHost {
        fn handle_musical_event(
            &mut self,
            _event: &MusicalEvent,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            _request: &RuntimePlatformRequest,
        ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
            Ok(Vec::new())
        }

        fn handle_audio_command(
            &mut self,
            _command: &RuntimeAudioCommand,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }

        fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }

        fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }

        fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }
    }

    #[test]
    fn accepted_snapshot_postpones_periodic_cadence_and_native_deadline_wins_once() {
        let start = Instant::now();
        let mut cadence = SnapshotCadence::new(start, 0);
        cadence.observe_accepted_snapshot(start + Duration::from_millis(10), 1);

        assert!(!cadence.periodic_due(start + Duration::from_millis(42), Duration::from_millis(33)));
        assert!(cadence.periodic_due(start + Duration::from_millis(44), Duration::from_millis(33)));

        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.test_set_display_time(start);
        runner.send(HostMessage::MidiRealtimeStart).unwrap();
        let deadline = cadence
            .next_timed_display_deadline(&runner)
            .expect("start should create a native display deadline");
        assert!(!cadence.timed_display_due(deadline - Duration::from_nanos(1), &runner));
        assert!(cadence.timed_display_due(deadline, &runner));

        cadence.observe_accepted_snapshot(deadline, 2);
        assert!(!cadence.timed_display_due(deadline, &runner));
    }

    #[test]
    fn failed_expiry_snapshot_keeps_deadline_due_until_retry_succeeds() {
        let start = Instant::now();
        let mut cadence = SnapshotCadence::new(start, 0);
        let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
        let mut host = TestHost;
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.test_set_display_time(start);
        playback
            .dispatch_host_message(HostMessage::MidiRealtimeStart, &mut runner, &mut host)
            .unwrap();
        cadence.observe_accepted_snapshot(start, playback.last_snapshot_revision());
        let deadline = cadence
            .next_timed_display_deadline(&runner)
            .expect("start should create a native display deadline");
        runner.test_set_display_time(deadline);
        runner.test_fail_next_snapshot();
        let accepted_revision_before_failure = playback.last_snapshot_revision();

        let expiry = HostMessage::TransportPulseStep {
            pulses: 0,
            source: playback_runtime::SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        };
        assert!(runner.send(expiry.clone()).is_err());
        cadence.observe_accepted_snapshot(deadline, playback.last_snapshot_revision());
        assert_eq!(
            playback.last_snapshot_revision(),
            accepted_revision_before_failure
        );
        assert!(cadence.timed_display_due(deadline, &runner));

        let retry = runner.send(expiry).unwrap();
        assert_eq!(
            retry
                .iter()
                .filter(|message| matches!(
                    message,
                    playback_runtime::RunnerMessage::Snapshot { .. }
                ))
                .count(),
            1
        );
        playback.ingest_runner_messages(retry, &mut host).unwrap();
        cadence.observe_accepted_snapshot(deadline, playback.last_snapshot_revision());
        assert!(playback.last_snapshot_revision() > accepted_revision_before_failure);
        assert!(!cadence.timed_display_due(deadline, &runner));
    }
}
