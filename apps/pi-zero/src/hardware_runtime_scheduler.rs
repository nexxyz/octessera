use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimeTransportState, SyncSource,
};
use std::time::{Duration, Instant};

pub(crate) const PLAYBACK_TICK: Duration = Duration::from_millis(8);
pub(crate) const SNAPSHOT_TICK: Duration = Duration::from_millis(33);
pub(crate) const MAINTENANCE_TICK: Duration = Duration::from_millis(50);
pub(crate) const SLEEP_MAX: Duration = Duration::from_millis(8);
const SNAPSHOT_RETRY_DELAY: Duration = Duration::from_millis(4);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DisplaySnapshotDue {
    pub(crate) one_shot: bool,
    pub(crate) continuous: bool,
}

impl DisplaySnapshotDue {
    pub(crate) fn any(self) -> bool {
        self.one_shot || self.continuous
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeAdvance {
    pub(crate) elapsed: Duration,
    pub(crate) lateness: Duration,
    pub(crate) request_snapshot: bool,
}

pub(crate) struct HardwareRuntimeScheduler {
    last_tick_at: Instant,
    last_accepted_snapshot_at: Instant,
    last_snapshot_attempt_at: Instant,
    last_render_attempt_at: Instant,
    last_accepted_snapshot_revision: u64,
    published_snapshot_revision: u64,
    playing_snapshot_revision: Option<u64>,
    playing_error_revision: Option<u64>,
    realtime_active: bool,
    snapshot_retry_at: Option<Instant>,
}

impl HardwareRuntimeScheduler {
    pub(crate) fn new(now: Instant, initial_published_revision: u64) -> Self {
        Self {
            last_tick_at: now,
            last_accepted_snapshot_at: now,
            last_snapshot_attempt_at: now,
            last_render_attempt_at: now.checked_sub(SNAPSHOT_TICK).unwrap_or(now),
            last_accepted_snapshot_revision: initial_published_revision,
            published_snapshot_revision: initial_published_revision,
            playing_snapshot_revision: None,
            playing_error_revision: None,
            realtime_active: false,
            snapshot_retry_at: None,
        }
    }

    pub(crate) fn next_runtime_advance(
        &mut self,
        now: Instant,
        playback: &PlaybackRuntime,
        xy_glide_deadline: Option<Instant>,
    ) -> Option<RuntimeAdvance> {
        self.observe_snapshot(now, playback);
        let realtime_active = runtime_tick_needed(playback, xy_glide_deadline);
        if realtime_active != self.realtime_active {
            self.realtime_active = realtime_active;
            self.last_tick_at = now;
            return None;
        }

        let interval = if realtime_active {
            PLAYBACK_TICK
        } else {
            MAINTENANCE_TICK
        };
        if now.duration_since(self.last_tick_at) < interval {
            return None;
        }

        let elapsed = now.duration_since(self.last_tick_at);
        self.last_tick_at = now;
        let lateness = elapsed.saturating_sub(interval);
        let request_snapshot = false;
        Some(RuntimeAdvance {
            elapsed,
            lateness,
            request_snapshot,
        })
    }

    pub(crate) fn display_snapshot_due(
        &self,
        now: Instant,
        runner: &NativeRunner,
        playback: &PlaybackRuntime,
    ) -> DisplaySnapshotDue {
        if self
            .snapshot_retry_at
            .is_some_and(|retry_at| retry_at > now)
            && !is_playing(playback)
        {
            return DisplaySnapshotDue::default();
        }
        DisplaySnapshotDue {
            one_shot: runner
                .next_timed_display_snapshot_deadline_after(Some(self.last_accepted_snapshot_at))
                .is_some_and(|deadline| deadline <= now),
            continuous: if is_playing(playback) {
                now.duration_since(self.last_snapshot_attempt_at) >= SNAPSHOT_TICK
            } else {
                runner
                    .next_continuous_display_snapshot_deadline(
                        self.last_snapshot_attempt_at,
                        SNAPSHOT_TICK,
                    )
                    .is_some_and(|deadline| deadline <= now)
            },
        }
    }

    pub(crate) fn record_snapshot_attempt(
        &mut self,
        now: Instant,
        due: DisplaySnapshotDue,
        revision_before: u64,
        revision_after: u64,
    ) {
        self.last_snapshot_attempt_at = now;
        self.observe_snapshot_revision(now, revision_before, revision_after);
        if due.one_shot && revision_after == revision_before {
            self.snapshot_retry_at = Some(now + SNAPSHOT_RETRY_DELAY);
        } else if due.one_shot {
            self.snapshot_retry_at = None;
        }
    }

    pub(crate) fn record_native_scene_capture(&mut self, captured_at: Instant) {
        self.last_accepted_snapshot_at = captured_at;
        self.last_snapshot_attempt_at = captured_at;
        self.snapshot_retry_at = None;
    }

    pub(crate) fn snapshot_publication_due(
        &mut self,
        now: Instant,
        playback: &PlaybackRuntime,
    ) -> bool {
        self.observe_snapshot(now, playback);
        let revision = playback.last_snapshot_revision();
        if is_playing(playback) {
            let error_visible = playback
                .last_snapshot()
                .and_then(|snapshot| snapshot.get("runtimeError"))
                .is_some_and(|error| !error.is_null());
            if error_visible
                && self
                    .playing_snapshot_revision
                    .is_some_and(|baseline| revision != 0 && revision != baseline)
                && revision != self.published_snapshot_revision
            {
                self.playing_error_revision = Some(revision);
                return now.duration_since(self.last_render_attempt_at) >= SNAPSHOT_TICK;
            }
            let error_cleared = self.playing_error_revision.is_some_and(|error_revision| {
                revision != error_revision
                    && playback
                        .last_snapshot()
                        .and_then(|snapshot| snapshot.get("runtimeError"))
                        .is_none_or(serde_json::Value::is_null)
            });
            return error_cleared
                && revision != self.published_snapshot_revision
                && now.duration_since(self.last_render_attempt_at) >= SNAPSHOT_TICK;
        }
        revision != 0
            && revision != self.published_snapshot_revision()
            && now.duration_since(self.last_render_attempt_at) >= SNAPSHOT_TICK
    }

    pub(crate) fn record_snapshot_publication_attempt(&mut self, now: Instant) {
        self.last_render_attempt_at = now;
    }

    pub(crate) fn record_snapshot_publication_accepted(&mut self, snapshot_revision: u64) {
        if snapshot_revision != 0 {
            self.published_snapshot_revision = snapshot_revision;
            if self
                .playing_error_revision
                .is_some_and(|error_revision| error_revision != snapshot_revision)
            {
                self.playing_error_revision = None;
            }
        }
    }

    pub(crate) fn observe_snapshot(&mut self, now: Instant, playback: &PlaybackRuntime) {
        if is_playing(playback) {
            self.playing_snapshot_revision
                .get_or_insert(self.published_snapshot_revision);
        } else {
            self.playing_snapshot_revision = None;
            self.playing_error_revision = None;
        }
        let revision = playback.last_snapshot_revision();
        self.observe_snapshot_revision(now, self.last_accepted_snapshot_revision, revision);
    }

    pub(crate) fn published_snapshot_revision(&self) -> u64 {
        self.published_snapshot_revision
    }

    pub(crate) fn record_runtime_advance_complete(
        &mut self,
        now: Instant,
        playback: &PlaybackRuntime,
        xy_glide_deadline: Option<Instant>,
    ) {
        let realtime_active = runtime_tick_needed(playback, xy_glide_deadline);
        if realtime_active != self.realtime_active {
            self.realtime_active = realtime_active;
            self.last_tick_at = now;
        }
    }

    pub(crate) fn display_snapshot_message(&self, playback: &PlaybackRuntime) -> HostMessage {
        HostMessage::TransportPulseStep {
            pulses: 0,
            source: playback.config().sync_source.clone(),
            at_ppqn_pulse: playback
                .last_status()
                .map(|status| status.current_ppqn_pulse),
            request_snapshot: Some(!is_playing(playback)),
        }
    }

    pub(crate) fn sleep_duration(
        &self,
        now: Instant,
        playback: &PlaybackRuntime,
        runner: &NativeRunner,
    ) -> Duration {
        let realtime_active = runtime_tick_needed(playback, runner.next_xy_glide_deadline());
        let next_runtime_due = self.last_tick_at
            + if realtime_active {
                PLAYBACK_TICK
            } else {
                MAINTENANCE_TICK
            };
        let mut next_due = next_runtime_due;
        if is_internal_playing(playback) {
            next_due =
                next_due.min((self.last_snapshot_attempt_at + SNAPSHOT_TICK).max(next_runtime_due));
        }
        if let Some(display_deadline) = self.display_snapshot_wake_deadline(now, runner) {
            let display_wake = if is_playing(playback) && display_deadline <= now {
                now + PLAYBACK_TICK
            } else {
                display_deadline
            };
            next_due = next_due.min(display_wake);
        }
        if !is_playing(playback)
            && playback.last_snapshot_revision() != self.published_snapshot_revision()
        {
            next_due = next_due.min(self.last_render_attempt_at + SNAPSHOT_TICK);
        }
        let max_sleep = SLEEP_MAX;
        next_due
            .checked_duration_since(now)
            .unwrap_or(Duration::ZERO)
            .min(max_sleep)
    }

    fn display_snapshot_deadline(&self, runner: &NativeRunner) -> Option<Instant> {
        let timed =
            runner.next_timed_display_snapshot_deadline_after(Some(self.last_accepted_snapshot_at));
        let continuous = runner.next_continuous_display_snapshot_deadline(
            self.last_snapshot_attempt_at,
            SNAPSHOT_TICK,
        );
        [timed, continuous].into_iter().flatten().min()
    }

    pub(crate) fn observe_snapshot_revision(
        &mut self,
        now: Instant,
        revision_before: u64,
        revision_after: u64,
    ) {
        if revision_after == revision_before
            || revision_after == 0
            || revision_after == self.last_accepted_snapshot_revision
        {
            return;
        }
        self.last_accepted_snapshot_revision = revision_after;
        self.last_accepted_snapshot_at = now;
        self.last_snapshot_attempt_at = now;
    }

    fn display_snapshot_wake_deadline(
        &self,
        now: Instant,
        runner: &NativeRunner,
    ) -> Option<Instant> {
        let deadline = self.display_snapshot_deadline(runner)?;
        if deadline <= now {
            self.snapshot_retry_at.or(Some(now))
        } else {
            Some(deadline)
        }
    }
}

pub(crate) fn runtime_tick_needed(
    playback: &PlaybackRuntime,
    xy_glide_deadline: Option<Instant>,
) -> bool {
    playback.has_scheduled_midi() || is_internal_playing(playback) || xy_glide_deadline.is_some()
}

pub(crate) fn is_internal_playing(playback: &PlaybackRuntime) -> bool {
    playback.config().sync_source == SyncSource::Internal
        && playback
            .last_status()
            .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
}

pub(crate) fn is_playing(playback: &PlaybackRuntime) -> bool {
    playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
}

pub(crate) fn prepare_dispatch_message(
    playback: &PlaybackRuntime,
    message: HostMessage,
) -> HostMessage {
    match message {
        HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        } if is_playing(playback) => HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod xy_glide_tests;
