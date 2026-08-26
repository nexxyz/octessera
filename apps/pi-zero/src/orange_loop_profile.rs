use std::ffi::OsStr;
use std::time::{Duration, Instant};

const PROFILE_ENV: &str = "OCTESSERA_ORANGE_LOOP_PROFILE";
const REPORT_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Default)]
struct TimedAggregate {
    count: u64,
    total: Duration,
}

impl TimedAggregate {
    fn record(&mut self, duration: Duration) {
        self.count += 1;
        self.total += duration;
    }
}

#[derive(Default)]
struct Aggregate {
    loop_count: u64,
    loop_active: Duration,
    audio: TimedAggregate,
    midi: TimedAggregate,
    first_host_work: TimedAggregate,
    inputs: TimedAggregate,
    input_messages: u64,
    input_events: u64,
    runtime: TimedAggregate,
    runtime_messages: u64,
    runtime_follow_ups: u64,
    second_host_work: TimedAggregate,
    snapshot_dispatch: TimedAggregate,
    snapshot_requests: u64,
    snapshot_publish: TimedAggregate,
    snapshot_accepted_revisions: u64,
    render_publications: u64,
    sleep: TimedAggregate,
    wall: TimedAggregate,
}

impl Aggregate {
    fn report_line(&self, window: Duration) -> String {
        format!(
            "octessera-orange-loop-profile v1 window_ms={} loop_n={} loop_active_us={} audio_n={} audio_us={} midi_n={} midi_us={} host1_n={} host1_us={} input_n={} input_us={} input_msg_n={} input_evt_n={} runtime_n={} runtime_us={} runtime_msg_n={} runtime_followup_n={} host2_n={} host2_us={} snap_dispatch_n={} snap_dispatch_us={} snap_request_n={} snap_publish_n={} snap_publish_us={} snap_revision_n={} render_pub_n={} sleep_n={} sleep_us={} wall_n={} wall_us={}",
            window.as_millis(),
            self.loop_count,
            self.loop_active.as_micros(),
            self.audio.count,
            self.audio.total.as_micros(),
            self.midi.count,
            self.midi.total.as_micros(),
            self.first_host_work.count,
            self.first_host_work.total.as_micros(),
            self.inputs.count,
            self.inputs.total.as_micros(),
            self.input_messages,
            self.input_events,
            self.runtime.count,
            self.runtime.total.as_micros(),
            self.runtime_messages,
            self.runtime_follow_ups,
            self.second_host_work.count,
            self.second_host_work.total.as_micros(),
            self.snapshot_dispatch.count,
            self.snapshot_dispatch.total.as_micros(),
            self.snapshot_requests,
            self.snapshot_publish.count,
            self.snapshot_publish.total.as_micros(),
            self.snapshot_accepted_revisions,
            self.render_publications,
            self.sleep.count,
            self.sleep.total.as_micros(),
            self.wall.count,
            self.wall.total.as_micros(),
        )
    }
}

pub(crate) struct OrangeLoopProfile {
    enabled: bool,
    last_report: Instant,
    aggregate: Aggregate,
}

impl OrangeLoopProfile {
    pub(crate) fn from_env() -> Self {
        Self::new(env_profile_enabled(
            std::env::var_os(PROFILE_ENV).as_deref(),
        ))
    }

    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_report: Instant::now(),
            aggregate: Aggregate::default(),
        }
    }

    pub(crate) fn start_loop(&mut self, wall: Duration) -> Option<Instant> {
        if !self.enabled {
            return None;
        }
        self.aggregate.loop_count += 1;
        self.aggregate.wall.record(wall);
        Some(Instant::now())
    }

    pub(crate) fn start_phase(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    pub(crate) fn record_audio(&mut self, started: Option<Instant>) {
        record_duration(&mut self.aggregate.audio, started);
    }

    pub(crate) fn record_midi(&mut self, started: Option<Instant>) {
        record_duration(&mut self.aggregate.midi, started);
    }

    pub(crate) fn record_first_host_work(&mut self, started: Option<Instant>) {
        record_duration(&mut self.aggregate.first_host_work, started);
    }

    pub(crate) fn record_inputs(
        &mut self,
        started: Option<Instant>,
        messages: usize,
        events: usize,
    ) {
        record_duration(&mut self.aggregate.inputs, started);
        if self.enabled {
            self.aggregate.input_messages += messages as u64;
            self.aggregate.input_events += events as u64;
        }
    }

    pub(crate) fn record_runtime(
        &mut self,
        started: Option<Instant>,
        messages: usize,
        follow_ups: usize,
    ) {
        record_duration(&mut self.aggregate.runtime, started);
        if self.enabled {
            self.aggregate.runtime_messages += messages as u64;
            self.aggregate.runtime_follow_ups += follow_ups as u64;
        }
    }

    pub(crate) fn record_second_host_work(&mut self, started: Option<Instant>) {
        record_duration(&mut self.aggregate.second_host_work, started);
    }

    pub(crate) fn record_snapshot_request(&mut self) {
        if self.enabled {
            self.aggregate.snapshot_requests += 1;
        }
    }

    pub(crate) fn record_snapshot_dispatch(&mut self, started: Option<Instant>) {
        record_duration(&mut self.aggregate.snapshot_dispatch, started);
    }

    pub(crate) fn record_snapshot_publish(&mut self, started: Option<Instant>, accepted: bool) {
        record_duration(&mut self.aggregate.snapshot_publish, started);
        if self.enabled && accepted {
            self.aggregate.snapshot_accepted_revisions += 1;
            self.aggregate.render_publications += 1;
        }
    }

    pub(crate) fn record_sleep(&mut self, started: Option<Instant>) {
        record_duration(&mut self.aggregate.sleep, started);
    }

    pub(crate) fn finish_loop(&mut self, started: Option<Instant>) {
        if let Some(started) = started {
            self.aggregate.loop_active += started.elapsed();
        }
    }

    pub(crate) fn maybe_report(&mut self) {
        if let Some(line) = self.maybe_report_at(Instant::now()) {
            eprintln!("{line}");
        }
    }

    fn maybe_report_at(&mut self, now: Instant) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let window = now.saturating_duration_since(self.last_report);
        if window < REPORT_INTERVAL {
            return None;
        }
        let line = self.aggregate.report_line(window);
        self.last_report = now;
        self.aggregate = Aggregate::default();
        Some(line)
    }
}

fn env_profile_enabled(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value == OsStr::new("1"))
}

fn record_duration(aggregate: &mut TimedAggregate, started: Option<Instant>) {
    if let Some(started) = started {
        aggregate.record(started.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::{env_profile_enabled, Aggregate, OrangeLoopProfile};
    use std::ffi::OsStr;
    use std::time::{Duration, Instant};

    #[test]
    fn env_profile_requires_the_exact_one_value() {
        assert!(env_profile_enabled(Some(OsStr::new("1"))));
        for value in ["0", "true", " 1", "1 ", "01", "yes", ""] {
            assert!(!env_profile_enabled(Some(OsStr::new(value))));
        }
        assert!(!env_profile_enabled(None));
    }

    #[test]
    fn disabled_profile_does_not_start_timers_or_reports() {
        let now = Instant::now();
        let mut profile = OrangeLoopProfile::new(false);

        assert!(profile.start_loop(Duration::from_millis(10)).is_none());
        assert!(profile.start_phase().is_none());
        assert!(profile
            .maybe_report_at(now + Duration::from_secs(10))
            .is_none());
    }

    #[test]
    fn report_rollover_resets_aggregate_after_ten_seconds() {
        let now = Instant::now();
        let mut profile = OrangeLoopProfile {
            enabled: true,
            last_report: now,
            aggregate: Aggregate {
                loop_count: 2,
                loop_active: Duration::from_micros(30),
                ..Aggregate::default()
            },
        };

        assert!(profile
            .maybe_report_at(now + Duration::from_secs(9))
            .is_none());
        let line = profile
            .maybe_report_at(now + Duration::from_secs(10))
            .expect("profile should report at the interval");

        assert_eq!(
            line,
            "octessera-orange-loop-profile v1 window_ms=10000 loop_n=2 loop_active_us=30 audio_n=0 audio_us=0 midi_n=0 midi_us=0 host1_n=0 host1_us=0 input_n=0 input_us=0 input_msg_n=0 input_evt_n=0 runtime_n=0 runtime_us=0 runtime_msg_n=0 runtime_followup_n=0 host2_n=0 host2_us=0 snap_dispatch_n=0 snap_dispatch_us=0 snap_request_n=0 snap_publish_n=0 snap_publish_us=0 snap_revision_n=0 render_pub_n=0 sleep_n=0 sleep_us=0 wall_n=0 wall_us=0"
        );
        assert_eq!(profile.aggregate.loop_count, 0);
        assert!(profile
            .maybe_report_at(now + Duration::from_secs(11))
            .is_none());
    }

    #[test]
    fn report_format_preserves_input_and_snapshot_counts() {
        let aggregate = Aggregate {
            input_messages: 3,
            input_events: 4,
            runtime_messages: 5,
            runtime_follow_ups: 6,
            snapshot_requests: 2,
            snapshot_accepted_revisions: 1,
            render_publications: 1,
            ..Aggregate::default()
        };

        let line = aggregate.report_line(Duration::from_millis(10_000));

        assert!(line.contains("input_msg_n=3 input_evt_n=4"));
        assert!(line.contains("runtime_msg_n=5 runtime_followup_n=6"));
        assert!(line.contains(
            "snap_request_n=2 snap_publish_n=0 snap_publish_us=0 snap_revision_n=1 render_pub_n=1"
        ));
    }
}
