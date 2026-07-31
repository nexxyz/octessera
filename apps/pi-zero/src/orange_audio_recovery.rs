use super::{open_orange_audio_sink_with_health, AudioSink, OpenedAudioSink, OrangeDacStatus};
use crate::audio_hotplug::{
    has_sink, register_sink, remove_sink, replay_to_sink, ReplayCache, SinkSender,
};
use crate::audio_stream_health::AudioStreamHealth;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
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
    dyn Fn(Option<u32>, AudioSink, AudioStreamHealth) -> Result<OpenedAudioSink, String>
        + Send
        + Sync,
>;
pub(super) type OrangeRecoveryClock = Arc<dyn Fn() -> Instant + Send + Sync>;

fn production_opener() -> OrangeRecoveryOpener {
    Arc::new(|output_buffer_frames, sink, health| {
        open_orange_audio_sink_with_health(output_buffer_frames, sink, health)
    })
}

fn system_clock() -> OrangeRecoveryClock {
    Arc::new(Instant::now)
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OrangeRecoveryAttempt {
    Stable,
    RecoverableFailure,
    DeviceNotAvailable,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OrangeRecoveryDecision {
    Recovered,
    Retrying,
    Terminal,
}

#[cfg(test)]
pub(super) fn run_bounded_orange_recovery<F>(
    health: &AudioStreamHealth,
    mut attempt: F,
) -> OrangeRecoveryDecision
where
    F: FnMut(usize) -> OrangeRecoveryAttempt,
{
    if health.is_terminal() {
        return OrangeRecoveryDecision::Terminal;
    }
    for attempt_number in 1..=ORANGE_RECOVERY_MAX_ATTEMPTS {
        match attempt(attempt_number) {
            OrangeRecoveryAttempt::Stable => return OrangeRecoveryDecision::Recovered,
            OrangeRecoveryAttempt::DeviceNotAvailable => {
                health.mark_terminal();
                return OrangeRecoveryDecision::Terminal;
            }
            OrangeRecoveryAttempt::RecoverableFailure => {}
        }
    }
    health.mark_terminal();
    OrangeRecoveryDecision::Terminal
}

#[cfg(test)]
pub(super) fn run_bounded_optional_recovery<F>(
    health: &AudioStreamHealth,
    mut attempt: F,
) -> OrangeRecoveryDecision
where
    F: FnMut(usize) -> OrangeRecoveryAttempt,
{
    for attempt_number in 1..=ORANGE_RECOVERY_MAX_ATTEMPTS {
        match attempt(attempt_number) {
            OrangeRecoveryAttempt::Stable => return OrangeRecoveryDecision::Recovered,
            OrangeRecoveryAttempt::DeviceNotAvailable => return OrangeRecoveryDecision::Retrying,
            OrangeRecoveryAttempt::RecoverableFailure => {}
        }
    }
    assert!(!health.is_terminal());
    OrangeRecoveryDecision::Retrying
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
    opener: OrangeRecoveryOpener,
    clock: OrangeRecoveryClock,
}

struct OrangeRecoveryDependencies {
    output_buffer_frames: Option<u32>,
    realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    replay_events: Arc<Mutex<ReplayCache>>,
    opener: OrangeRecoveryOpener,
    clock: OrangeRecoveryClock,
}

impl OrangeRecoveryDependencies {
    fn production(
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
    ) -> Self {
        Self {
            output_buffer_frames,
            realtime_txs,
            replay_events,
            opener: production_opener(),
            clock: system_clock(),
        }
    }
}

pub(super) struct OrangeRecoveryWorker {
    stop_tx: Sender<()>,
    join: Option<JoinHandle<()>>,
}

impl OrangeRecoveryWorker {
    pub(super) fn spawn(
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
    ) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("orange-uac2-recovery".into())
            .spawn(move || {
                let mut controller = OrangeRecoveryController::new_optional_missing(
                    output_buffer_frames,
                    realtime_txs,
                    replay_events,
                );
                run_worker(&mut controller, stop_rx);
            })
            .expect("Orange UAC2 recovery worker should start");
        Self {
            stop_tx,
            join: Some(join),
        }
    }
}

impl Drop for OrangeRecoveryWorker {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_worker(controller: &mut OrangeRecoveryController, stop_rx: Receiver<()>) {
    loop {
        controller.recover_if_due();
        match stop_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

impl OrangeRecoveryController {
    pub(super) fn new_required(
        initial: OpenedAudioSink,
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
    ) -> Self {
        Self::new_with_dependencies(
            AudioSink::InternalDac,
            OrangeRecoveryMode::Required,
            initial.health.clone(),
            Some(initial),
            OrangeRecoveryPhase::Healthy,
            OrangeRecoveryDependencies::production(
                output_buffer_frames,
                realtime_txs,
                replay_events,
            ),
        )
    }

    pub(super) fn new_optional_missing(
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
    ) -> Self {
        let clock = system_clock();
        let now = clock();
        Self::new_with_dependencies(
            AudioSink::Usb,
            OrangeRecoveryMode::Optional,
            AudioStreamHealth::optional("UAC2Gadget".into()),
            None,
            OrangeRecoveryPhase::Retrying {
                attempts: 0,
                next_attempt_at: now,
            },
            OrangeRecoveryDependencies {
                output_buffer_frames,
                realtime_txs,
                replay_events,
                opener: production_opener(),
                clock,
            },
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
            opener,
            clock,
        }
    }

    #[cfg(test)]
    pub(super) fn new_optional_missing_with_dependencies(
        output_buffer_frames: Option<u32>,
        realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
        replay_events: Arc<Mutex<ReplayCache>>,
        opener: OrangeRecoveryOpener,
        clock: OrangeRecoveryClock,
    ) -> Self {
        let now = clock();
        Self::new_with_dependencies(
            AudioSink::Usb,
            OrangeRecoveryMode::Optional,
            AudioStreamHealth::optional("UAC2Gadget".into()),
            None,
            OrangeRecoveryPhase::Retrying {
                attempts: 0,
                next_attempt_at: now,
            },
            OrangeRecoveryDependencies {
                output_buffer_frames,
                realtime_txs,
                replay_events,
                opener,
                clock,
            },
        )
    }

    pub(super) fn recover_if_due(&mut self) {
        let now = (self.clock)();
        let phase = std::mem::replace(&mut self.phase, OrangeRecoveryPhase::Terminal);
        self.phase = match phase {
            OrangeRecoveryPhase::Healthy
                if self.mode == OrangeRecoveryMode::Required && self.health.is_terminal() =>
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
            OrangeRecoveryPhase::Healthy if self.health.is_faulted() => {
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
            } if now >= next_attempt_at => self.try_open(attempts),
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

    pub(super) fn status(&self) -> OrangeDacStatus {
        if self.mode == OrangeRecoveryMode::Required
            && (matches!(self.phase, OrangeRecoveryPhase::Terminal) || self.health.is_terminal())
        {
            OrangeDacStatus::Terminal
        } else if matches!(self.phase, OrangeRecoveryPhase::Healthy)
            && self.health.status() == OrangeDacStatus::Healthy
        {
            OrangeDacStatus::Healthy
        } else {
            OrangeDacStatus::Recovering
        }
    }

    fn try_open(&mut self, attempts: usize) -> OrangeRecoveryPhase {
        let attempt = attempts + 1;
        if self.mode == OrangeRecoveryMode::Required && self.health.is_terminal() {
            return OrangeRecoveryPhase::Terminal;
        }
        self.health.clear_recoverable_fault();
        let opened = match (self.opener)(self.output_buffer_frames, self.sink, self.health.clone())
        {
            Ok(opened) => opened,
            Err(error) => {
                eprintln!(
                    "Orange {:?} recovery attempt {attempt} failed: {error}",
                    self.sink
                );
                return self.failed_attempt(attempt);
            }
        };
        OrangeRecoveryPhase::Stabilizing {
            opened,
            attempts: attempt,
            stable_until: (self.clock)() + ORANGE_RECOVERY_STABLE_GRACE,
        }
    }

    fn finish_stabilizing(
        &mut self,
        opened: OpenedAudioSink,
        attempts: usize,
        stable_until: Instant,
        now: Instant,
    ) -> OrangeRecoveryPhase {
        if self.mode == OrangeRecoveryMode::Required && self.health.is_terminal() {
            return OrangeRecoveryPhase::Terminal;
        }
        if self.health.is_faulted() {
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
        if let Err(error) = replay_to_sink(&opened.engine_tx, &self.replay_events) {
            eprintln!("Orange {:?} recovery replay failed: {error}", self.sink);
            return self.failed_attempt(attempts);
        }
        register_sink(&self.realtime_txs, self.sink, opened.engine_tx.clone());
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
        remove_sink(&self.realtime_txs, self.sink);
        drop(self.current.take());
    }
}
