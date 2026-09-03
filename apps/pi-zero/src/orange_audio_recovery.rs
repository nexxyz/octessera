use super::audio_output_open::open_orange_audio_sink_with_health;
use super::audio_output_open::OpenedAudioSink;
use super::{AudioSink, OrangeDacStatus, RecordingTapState};
use crate::audio_replay::ReplayCache;
use crate::audio_route::RouteOpenError;
use crate::audio_sink_registry::{
    attach_sink_atomic, has_sink, remove_sink_atomic, AudioAttachGate, SinkSender,
};
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use rodio_engine_source::AudioLoadStatusSender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ORANGE_RECOVERY_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const ORANGE_RECOVERY_STABLE_GRACE: Duration = Duration::from_millis(250);
const ORANGE_OPTIONAL_RECOVERY_COOLDOWN: Duration = Duration::from_secs(2);
pub(super) const ORANGE_RECOVERY_MAX_ATTEMPTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrangeRecoveryMode {
    Required,
    Optional,
}

pub(super) type OrangeRecoveryOpener = Arc<
    dyn Fn(
            Option<u32>,
            AudioSink,
            AudioStreamHealth,
            Option<RecordingTapState>,
            Option<AudioLoadStatusSender>,
        ) -> Result<OpenedAudioSink, RouteOpenError>
        + Send
        + Sync,
>;
pub(super) type OrangeRecoveryClock = Arc<dyn Fn() -> Instant + Send + Sync>;

fn production_opener() -> OrangeRecoveryOpener {
    Arc::new(
        |output_buffer_frames, sink, health, recording_tap, load_tx| {
            open_orange_audio_sink_with_health(
                output_buffer_frames,
                sink,
                health,
                recording_tap,
                load_tx,
            )
        },
    )
}

fn system_clock() -> OrangeRecoveryClock {
    Arc::new(Instant::now)
}

enum OrangeRecoveryPhase {
    Healthy,
    Retrying {
        attempts: usize,
        next_attempt_at: Instant,
    },
    Stabilizing {
        opened: OpenedAudioSink,
        attempts: usize,
        stable_until: Instant,
    },
    Terminal,
}

pub(super) struct OrangeRecoveryController {
    sink: AudioSink,
    mode: OrangeRecoveryMode,
    health: AudioStreamHealth,
    current: Option<OpenedAudioSink>,
    phase: OrangeRecoveryPhase,
    output_buffer_frames: Option<u32>,
    realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    replay_events: Arc<Mutex<ReplayCache>>,
    attach_gate: AudioAttachGate,
    recording_tap: Option<RecordingTapState>,
    opener: OrangeRecoveryOpener,
    clock: OrangeRecoveryClock,
}

pub(super) struct OrangeRecoveryDependencies {
    pub(super) output_buffer_frames: Option<u32>,
    pub(super) realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    pub(super) replay_events: Arc<Mutex<ReplayCache>>,
    pub(super) attach_gate: AudioAttachGate,
    pub(super) recording_tap: Option<RecordingTapState>,
    pub(super) opener: OrangeRecoveryOpener,
    pub(super) clock: OrangeRecoveryClock,
}

impl OrangeRecoveryDependencies {
    fn production(
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
        recording_tap: Option<RecordingTapState>,
        attach_gate: AudioAttachGate,
    ) -> Self {
        Self {
            output_buffer_frames,
            realtime_txs,
            replay_events,
            attach_gate,
            recording_tap,
            opener: production_opener(),
            clock: system_clock(),
        }
    }
}

impl OrangeRecoveryController {
    pub(super) fn sink(&self) -> AudioSink {
        self.sink
    }
    pub(super) fn new_required(
        initial: OpenedAudioSink,
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
        recording_tap: Option<RecordingTapState>,
        attach_gate: AudioAttachGate,
    ) -> Result<Self, String> {
        let controller = Self::new_with_dependencies(
            AudioSink::Jack,
            OrangeRecoveryMode::Required,
            initial.health.clone(),
            Some(initial),
            OrangeRecoveryPhase::Healthy,
            OrangeRecoveryDependencies::production(
                output_buffer_frames,
                realtime_txs,
                replay_events,
                recording_tap,
                attach_gate,
            ),
        );
        attach_sink_atomic(
            &controller.attach_gate,
            &controller.realtime_txs,
            &controller.replay_events,
            AudioSink::Jack,
            controller
                .current
                .as_ref()
                .expect("initial Jack stream")
                .engine_tx
                .clone(),
        )?;
        Ok(controller)
    }

    pub(super) fn new_optional_missing(
        sink: AudioSink,
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
        recording_tap: Option<RecordingTapState>,
        attach_gate: AudioAttachGate,
    ) -> Self {
        let clock = system_clock();
        let now = clock();
        Self::new_with_dependencies(
            sink,
            OrangeRecoveryMode::Optional,
            AudioStreamHealth::optional(format!("{sink:?}")),
            None,
            OrangeRecoveryPhase::Retrying {
                attempts: 0,
                next_attempt_at: now,
            },
            OrangeRecoveryDependencies {
                output_buffer_frames,
                realtime_txs,
                replay_events,
                attach_gate,
                recording_tap,
                opener: production_opener(),
                clock,
            },
        )
    }

    #[allow(dead_code)]
    pub(super) fn new_optional_initial(
        sink: AudioSink,
        initial: OpenedAudioSink,
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
        recording_tap: Option<RecordingTapState>,
        attach_gate: AudioAttachGate,
    ) -> Result<Self, String> {
        let controller = Self::new_with_dependencies(
            sink,
            OrangeRecoveryMode::Optional,
            initial.health.clone(),
            Some(initial),
            OrangeRecoveryPhase::Healthy,
            OrangeRecoveryDependencies::production(
                output_buffer_frames,
                realtime_txs,
                replay_events,
                recording_tap,
                attach_gate,
            ),
        );
        attach_sink_atomic(
            &controller.attach_gate,
            &controller.realtime_txs,
            &controller.replay_events,
            sink,
            controller
                .current
                .as_ref()
                .expect("initial optional stream")
                .engine_tx
                .clone(),
        )?;
        Ok(controller)
    }

    #[cfg(test)]
    pub(super) fn new_initial_with_dependencies(
        sink: AudioSink,
        required: bool,
        initial: OpenedAudioSink,
        dependencies: OrangeRecoveryDependencies,
    ) -> Result<Self, String> {
        test_support::new_initial_with_dependencies(
            sink,
            if required {
                OrangeRecoveryMode::Required
            } else {
                OrangeRecoveryMode::Optional
            },
            initial,
            dependencies,
        )
    }

    fn new_with_dependencies(
        sink: AudioSink,
        mode: OrangeRecoveryMode,
        health: AudioStreamHealth,
        current: Option<OpenedAudioSink>,
        phase: OrangeRecoveryPhase,
        dependencies: OrangeRecoveryDependencies,
    ) -> Self {
        let OrangeRecoveryDependencies {
            output_buffer_frames,
            realtime_txs,
            replay_events,
            attach_gate,
            recording_tap,
            opener,
            clock,
        } = dependencies;
        Self {
            sink,
            mode,
            health,
            current,
            phase,
            output_buffer_frames,
            realtime_txs,
            replay_events,
            attach_gate,
            recording_tap,
            opener,
            clock,
        }
    }

    #[cfg(test)]
    pub(super) fn new_optional_missing_with_dependencies(
        sink: AudioSink,
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
        recording_tap: Option<RecordingTapState>,
        opener: OrangeRecoveryOpener,
        clock: OrangeRecoveryClock,
    ) -> Self {
        let now = clock();
        Self::new_with_dependencies(
            sink,
            OrangeRecoveryMode::Optional,
            AudioStreamHealth::optional(format!("{sink:?}")),
            None,
            OrangeRecoveryPhase::Retrying {
                attempts: 0,
                next_attempt_at: now,
            },
            OrangeRecoveryDependencies {
                output_buffer_frames,
                realtime_txs,
                replay_events,
                attach_gate: crate::audio_sink_registry::new_attach_gate(),
                recording_tap,
                opener,
                clock,
            },
        )
    }

    pub(super) fn recover_if_due(&mut self) {
        self.recover_if_due_with(|| {}, None);
    }

    pub(super) fn recover_if_due_with(
        &mut self,
        mut before_open: impl FnMut(),
        load_tx: Option<AudioLoadStatusSender>,
    ) {
        let now = (self.clock)();
        let phase = std::mem::replace(&mut self.phase, OrangeRecoveryPhase::Terminal);
        self.phase = match phase {
            OrangeRecoveryPhase::Healthy
                if self.health.external_status() == AudioStreamStatus::Terminal =>
            {
                self.detach_current();
                OrangeRecoveryPhase::Terminal
            }
            OrangeRecoveryPhase::Healthy
                if self.mode == OrangeRecoveryMode::Optional
                    && !has_sink(&self.realtime_txs, self.sink) =>
            {
                self.detach_current();
                OrangeRecoveryPhase::Retrying {
                    attempts: 0,
                    next_attempt_at: now,
                }
            }
            OrangeRecoveryPhase::Healthy
                if self.health.external_status() == AudioStreamStatus::Recovering =>
            {
                self.detach_current();
                OrangeRecoveryPhase::Retrying {
                    attempts: 0,
                    next_attempt_at: now,
                }
            }
            OrangeRecoveryPhase::Healthy => OrangeRecoveryPhase::Healthy,
            OrangeRecoveryPhase::Retrying {
                attempts,
                next_attempt_at,
            } if now >= next_attempt_at => self.try_open(attempts, &mut before_open, load_tx),
            OrangeRecoveryPhase::Retrying {
                attempts,
                next_attempt_at,
            } => OrangeRecoveryPhase::Retrying {
                attempts,
                next_attempt_at,
            },
            OrangeRecoveryPhase::Stabilizing {
                opened,
                attempts,
                stable_until,
            } => self.finish_stabilizing(opened, attempts, stable_until, now),
            OrangeRecoveryPhase::Terminal => OrangeRecoveryPhase::Terminal,
        };
    }

    pub(super) fn device_status(&self) -> OrangeDacStatus {
        if matches!(self.phase, OrangeRecoveryPhase::Terminal) || self.health.external_is_terminal()
        {
            OrangeDacStatus::Terminal
        } else if matches!(self.phase, OrangeRecoveryPhase::Healthy)
            && self.health.external_status() == OrangeDacStatus::Healthy
        {
            OrangeDacStatus::Healthy
        } else {
            OrangeDacStatus::Recovering
        }
    }

    pub(super) fn runtime_status(&self) -> OrangeDacStatus {
        if self.health.runtime_status() == AudioStreamStatus::Terminal {
            OrangeDacStatus::Terminal
        } else {
            self.device_status()
        }
    }

    pub(super) fn report_runtime_terminal(&self) {
        self.health.log_worker_terminal_once();
    }

    fn finish_stabilizing(
        &mut self,
        opened: OpenedAudioSink,
        attempts: usize,
        stable_until: Instant,
        now: Instant,
    ) -> OrangeRecoveryPhase {
        if self.health.external_status() == AudioStreamStatus::Terminal {
            return OrangeRecoveryPhase::Terminal;
        }
        if self.health.external_status() == AudioStreamStatus::Recovering {
            eprintln!(
                "Orange {:?} recovery attempt {attempts} remained unstable",
                self.sink
            );
            drop(opened);
            return self.failed_attempt(attempts);
        }
        if now < stable_until {
            return OrangeRecoveryPhase::Stabilizing {
                opened,
                attempts,
                stable_until,
            };
        }
        if let Err(error) = attach_sink_atomic(
            &self.attach_gate,
            &self.realtime_txs,
            &self.replay_events,
            self.sink,
            opened.engine_tx.clone(),
        ) {
            eprintln!("Orange {:?} recovery replay failed: {error}", self.sink);
            self.health.mark_terminal();
            drop(opened);
            return OrangeRecoveryPhase::Terminal;
        }
        self.current = Some(opened);
        OrangeRecoveryPhase::Healthy
    }

    fn failed_attempt(&self, attempts: usize) -> OrangeRecoveryPhase {
        if self.mode == OrangeRecoveryMode::Optional {
            return if attempts >= ORANGE_RECOVERY_MAX_ATTEMPTS {
                OrangeRecoveryPhase::Retrying {
                    attempts: 0,
                    next_attempt_at: (self.clock)() + ORANGE_OPTIONAL_RECOVERY_COOLDOWN,
                }
            } else {
                OrangeRecoveryPhase::Retrying {
                    attempts,
                    next_attempt_at: (self.clock)() + ORANGE_RECOVERY_RETRY_INTERVAL,
                }
            };
        }
        if attempts >= ORANGE_RECOVERY_MAX_ATTEMPTS {
            self.health.mark_terminal();
            eprintln!("Orange {:?} recovery reached its terminal state", self.sink);
            OrangeRecoveryPhase::Terminal
        } else {
            OrangeRecoveryPhase::Retrying {
                attempts,
                next_attempt_at: (self.clock)() + ORANGE_RECOVERY_RETRY_INTERVAL,
            }
        }
    }

    fn detach_current(&mut self) {
        let _ = remove_sink_atomic(&self.attach_gate, &self.realtime_txs, self.sink);
        drop(self.current.take());
    }
}

#[path = "orange_audio_recovery_open.rs"]
mod open;
#[cfg(test)]
#[path = "orange_audio_recovery_test_support.rs"]
mod test_support;
