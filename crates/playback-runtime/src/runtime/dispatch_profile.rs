use super::PlaybackRuntime;
use crate::protocol::RunnerMessage;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

impl PlaybackRuntime {
    pub(super) fn profile_ingest_messages(&mut self, messages: &[RunnerMessage]) {
        if let Some(profile) = self
            .dispatch_profile
            .as_mut()
            .filter(|profile| profile.active)
        {
            let count = messages
                .iter()
                .filter(|message| matches!(message, RunnerMessage::Snapshot { .. }))
                .count();
            profile.status_only = count == 0;
            profile.snapshots_received += count as u64;
        }
    }
}

#[cfg(test)]
impl PlaybackRuntime {
    pub(crate) fn test_enable_dispatch_profile(&mut self) {
        self.dispatch_profile = Some(RuntimeDispatchProfile::new());
    }

    pub(crate) fn test_refresh_counts(&self) -> (u64, u64) {
        let profile = self
            .dispatch_profile
            .as_ref()
            .expect("test profile enabled");
        (
            profile.spans[Stage::FullSnapshotRefresh as usize].n,
            profile.spans[Stage::StatusRefresh as usize].n,
        )
    }

    pub(crate) fn test_received_snapshots(&self) -> u64 {
        self.dispatch_profile
            .as_ref()
            .expect("test profile enabled")
            .snapshots_received
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Span {
    n: u64,
    total_us: u128,
    max_us: u128,
}

impl Span {
    fn add(&mut self, elapsed: Duration) {
        let us = elapsed.as_micros();
        self.n += 1;
        self.total_us += us;
        self.max_us = self.max_us.max(us);
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "n={} avg_us={} max_us={}",
            self.n,
            if self.n == 0 {
                0
            } else {
                self.total_us / u128::from(self.n)
            },
            self.max_us
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Dispatch,
    Runner,
    Ingest,
    StatusRefresh,
    SnapshotClone,
    SemanticCompare,
    FrameRender,
    Append,
    FullSnapshotRefresh,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RuntimeDispatchProfile {
    window: Instant,
    pub(super) active: bool,
    pub(super) status_only: bool,
    dispatches: u64,
    pulses: u64,
    max_pulses: u32,
    snapshot_requests: u64,
    pub(super) snapshots_received: u64,
    spans: [Span; 9],
}

impl RuntimeDispatchProfile {
    pub(super) fn from_environment() -> Option<Self> {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        ENABLED
            .get_or_init(|| {
                enabled(
                    std::env::var("OCTESSERA_RUNTIME_TIMING_TRACE")
                        .ok()
                        .as_deref(),
                )
            })
            .then(Self::new)
    }

    fn new() -> Self {
        Self {
            window: Instant::now(),
            active: false,
            status_only: false,
            dispatches: 0,
            pulses: 0,
            max_pulses: 0,
            snapshot_requests: 0,
            snapshots_received: 0,
            spans: [Span::default(); 9],
        }
    }

    pub(super) fn begin(&mut self, pulses: u32, request_snapshot: bool) {
        self.active = true;
        self.dispatches += 1;
        self.pulses += u64::from(pulses);
        self.max_pulses = self.max_pulses.max(pulses);
        self.snapshot_requests += u64::from(request_snapshot);
    }

    pub(super) fn start(&self) -> Option<Instant> {
        self.active.then(Instant::now)
    }

    pub(super) fn record(&mut self, stage: Stage, started: Option<Instant>) {
        if let Some(started) = started {
            self.spans[stage as usize].add(started.elapsed());
        }
    }

    pub(super) fn status_start(&self) -> Option<Instant> {
        (self.active && self.status_only).then(Instant::now)
    }

    pub(super) fn finish(&mut self, started: Option<Instant>) {
        self.record(Stage::Dispatch, started);
        self.active = false;
        if self.window.elapsed() < Duration::from_secs(5) {
            return;
        }
        let s = &self.spans;
        eprintln!("runtime-timing playing dispatches={} pulses={} max_pulses={} snapshot_requests={} snapshots_received={} dispatch=[{}] runner_send=[{}] ingest=[{}] status_only_refresh=[{}] status_clone=[{}] status_compare=[{}] status_frame=[{}] append_presentations=[{}] full_snapshot_refresh=[{}]",
            self.dispatches, self.pulses, self.max_pulses, self.snapshot_requests, self.snapshots_received,
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], s[8]);
        *self = Self::new();
    }
}

fn enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

#[cfg(test)]
mod tests;
