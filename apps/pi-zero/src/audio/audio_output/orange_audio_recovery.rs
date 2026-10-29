use super::audio_output_open::open_orange_audio_sink_with_health;
use super::audio_output_open::OpenedAudioSink;
use super::audio_profile::OrangeAudioProfile;
use super::{AudioSink, MixTapState, OrangeDacStatus};
use crate::audio_engine_owner::AudioEngineOwner;
use crate::audio_route::RouteOpenError;
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use rodio_engine_source::{AudioLoadStatusSender, PcmMirrorConsumer, PcmMirrorProducers};
use std::sync::Arc;
use std::time::{Duration, Instant};

const ORANGE_RECOVERY_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const ORANGE_RECOVERY_STABLE_GRACE: Duration = Duration::from_millis(250);
pub(super) const ORANGE_RECOVERY_MAX_ATTEMPTS: usize = 3;

pub(super) type OrangeRecoveryOpener = Arc<
    dyn Fn(
            OrangeAudioProfile,
            AudioSink,
            AudioStreamHealth,
            Option<MixTapState>,
            Option<AudioLoadStatusSender>,
            PcmMirrorProducers,
            Option<PcmMirrorConsumer>,
        ) -> Result<OpenedAudioSink, RouteOpenError>
        + Send
        + Sync,
>;
pub(super) type OrangeRecoveryClock = Arc<dyn Fn() -> Instant + Send + Sync>;

fn production_opener() -> OrangeRecoveryOpener {
    Arc::new(
        |profile, sink, health, mix_taps, load_tx, mirror_producers, mirror_consumer| {
            open_orange_audio_sink_with_health(
                super::audio_output_open::AudioConstructionConfig::Orange(profile),
                sink,
                health,
                mix_taps,
                load_tx,
                mirror_producers,
                mirror_consumer,
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
    health: AudioStreamHealth,
    current: Option<OpenedAudioSink>,
    phase: OrangeRecoveryPhase,
    profile: OrangeAudioProfile,
    engine: AudioEngineOwner,
    mix_taps: Option<MixTapState>,
    mirror_producers: PcmMirrorProducers,
    opener: OrangeRecoveryOpener,
    clock: OrangeRecoveryClock,
}

pub(super) struct OrangeRecoveryDependencies {
    pub(super) profile: OrangeAudioProfile,
    pub(super) engine: AudioEngineOwner,
    pub(super) mix_taps: Option<MixTapState>,
    pub(super) mirror_producers: PcmMirrorProducers,
    pub(super) opener: OrangeRecoveryOpener,
    pub(super) clock: OrangeRecoveryClock,
}

impl OrangeRecoveryDependencies {
    fn production(
        profile: OrangeAudioProfile,
        engine: AudioEngineOwner,
        mix_taps: Option<MixTapState>,
        mirror_producers: PcmMirrorProducers,
    ) -> Self {
        Self {
            profile,
            engine,
            mix_taps,
            mirror_producers,
            opener: production_opener(),
            clock: system_clock(),
        }
    }
}

impl OrangeRecoveryController {
    pub(super) fn new_required(
        initial: OpenedAudioSink,
        profile: OrangeAudioProfile,
        engine: AudioEngineOwner,
        mix_taps: Option<MixTapState>,
        mirror_producers: PcmMirrorProducers,
    ) -> Result<Self, String> {
        let controller = Self::new_with_dependencies(
            AudioSink::Jack,
            initial.health.clone(),
            Some(initial),
            OrangeRecoveryPhase::Healthy,
            OrangeRecoveryDependencies::production(profile, engine, mix_taps, mirror_producers),
        );
        controller.engine.attach(
            controller
                .current
                .as_ref()
                .expect("initial Jack stream")
                .engine_tx
                .as_ref()
                .expect("Jack engine event sender")
                .clone(),
        )?;
        Ok(controller)
    }

    #[cfg(test)]
    pub(super) fn new_initial_with_dependencies(
        sink: AudioSink,
        initial: OpenedAudioSink,
        dependencies: OrangeRecoveryDependencies,
    ) -> Result<Self, String> {
        test_support::new_initial_with_dependencies(sink, initial, dependencies)
    }

    fn new_with_dependencies(
        sink: AudioSink,
        health: AudioStreamHealth,
        current: Option<OpenedAudioSink>,
        phase: OrangeRecoveryPhase,
        dependencies: OrangeRecoveryDependencies,
    ) -> Self {
        let OrangeRecoveryDependencies {
            profile,
            engine,
            mix_taps,
            mirror_producers,
            opener,
            clock,
        } = dependencies;
        Self {
            sink,
            health,
            current,
            phase,
            profile,
            engine,
            mix_taps,
            mirror_producers,
            opener,
            clock,
        }
    }

    #[cfg(test)]
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
        let Some(engine_tx) = opened.engine_tx.as_ref() else {
            self.health.mark_terminal();
            drop(opened);
            return OrangeRecoveryPhase::Terminal;
        };
        if let Err(error) = self.engine.attach(engine_tx.clone()) {
            eprintln!("Orange {:?} recovery replay failed: {error}", self.sink);
            self.health.mark_terminal();
            drop(opened);
            return OrangeRecoveryPhase::Terminal;
        }
        self.current = Some(opened);
        OrangeRecoveryPhase::Healthy
    }

    fn failed_attempt(&self, attempts: usize) -> OrangeRecoveryPhase {
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
        self.engine.detach();
        drop(self.current.take());
    }
}

mod open;
#[cfg(test)]
mod test_support;
