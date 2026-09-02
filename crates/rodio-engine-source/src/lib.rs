mod audio_quantum;
mod control_drain;
mod event;
mod profile_cache;
mod queue;
mod retired_audio_backlog;
mod sample_decode;
mod source_factory;
mod source_shutdown;
mod source_worker;
mod source_worker_reaper;
mod telemetry;

use audio_quantum::audio_render_quantum_frames;
#[cfg(test)]
use audio_quantum::resolve_audio_render_quantum_frames;
use crossbeam_channel::{bounded, Sender, TrySendError};
pub use event::EngineEvent;
pub use queue::{event_queue, EngineEventReceiver, EngineEventSender, QueueKind, QueueSendError};
#[cfg(feature = "source-worker-benchmark-timing")]
use realtime_engine::synth::SourceWorkerTimingProbe;
use realtime_engine::synth::{
    RetiredAudioState, SourceWorkerHealth, SourceWorkerLifecycle, SourceWorkerRuntime,
    SourceWorkerSetupError, SourceWorkerStartHook, SynthEngine, SynthProfileSnapshot,
    DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES,
};
use retired_audio_backlog::RetiredAudioBacklog;
pub use sample_decode::decode_sample_file;
use source_worker::{EngineSourceMode, EngineSourceWorkerState};
pub use source_worker::{EngineSourceWorkerShutdownError, EngineSourceWorkerShutdownOwner};
use source_worker_reaper::SourceShutdownEnvelope;
pub use source_worker_reaper::SOURCE_REAPER_THREAD_NAME;
#[cfg(feature = "source-worker-benchmark-timing")]
use std::sync::Arc;
use std::time::{Duration, Instant};
pub use telemetry::{audio_load_status_channel, AudioLoadStatusReceiver, AudioLoadStatusSender};
use telemetry::{DrainedControlEvents, EngineTelemetry};

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2048;
const LOAD_REPORT_INTERVAL: Duration = Duration::from_millis(100);
const RETIREMENT_QUEUE_CAPACITY: usize = 64;
const RETIREMENT_BACKLOG_CAPACITY: usize = 256;
const RETIREMENT_CONTROL_BACKLOG_CAPACITY: usize = RETIREMENT_BACKLOG_CAPACITY - 1;

pub(crate) struct RetiredAudioItem {
    state: Option<RetiredAudioState>,
    event: Option<EngineEvent>,
    #[cfg(test)]
    pub(crate) drop_probe: Option<RetiredAudioDropProbe>,
}

struct SourceRetirementChannels {
    retired_tx: Sender<RetiredAudioItem>,
    shutdown_tx: Sender<SourceShutdownEnvelope>,
}

#[cfg(test)]
pub(crate) struct RetiredAudioDropProbe {
    drop_tx: std::sync::mpsc::Sender<std::thread::ThreadId>,
}

#[cfg(test)]
impl Drop for RetiredAudioDropProbe {
    fn drop(&mut self) {
        let _ = self.drop_tx.send(std::thread::current().id());
    }
}

pub struct EngineSource {
    engine: SynthEngine,
    worker_state: EngineSourceWorkerState,
    control_rx: EngineEventReceiver,
    sample_rate: u32,
    block_frames: usize,
    cached_profile_snapshot: SynthProfileSnapshot,
    buf: Vec<f32>,
    left_buf: Vec<f32>,
    right_buf: Vec<f32>,
    idx: usize,
    load_tx: Option<AudioLoadStatusSender>,
    last_load_report: Instant,
    telemetry: EngineTelemetry,
    retired_tx: Sender<RetiredAudioItem>,
    retired_backlog: Option<RetiredAudioBacklog>,
    shutdown_tx: Option<Sender<SourceShutdownEnvelope>>,
    retirement_disconnected: bool,
    #[cfg(test)]
    retired_drop_probe: Option<std::sync::mpsc::Sender<std::thread::ThreadId>>,
}

impl EngineSource {
    pub fn new(control_rx: EngineEventReceiver, sample_rate: u32) -> Self {
        Self::with_load_status_tx(control_rx, sample_rate, None)
    }

    pub fn with_block_frames(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
    ) -> Self {
        Self::with_config(
            control_rx,
            sample_rate,
            block_frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES),
            None,
        )
    }

    pub fn resolve_block_frames(default_frames: usize) -> usize {
        audio_render_quantum_frames(default_frames)
    }

    pub fn block_frames(&self) -> usize {
        self.block_frames
    }

    pub fn source_worker_health(&self) -> SourceWorkerHealth {
        self.worker_state.health()
    }

    pub fn profile_snapshot(&self) -> realtime_engine::synth::SynthProfileSnapshot {
        match self.worker_state.mode {
            EngineSourceMode::Inline => self.engine.profile_snapshot(),
            EngineSourceMode::Persistent => self.cached_profile_snapshot,
        }
    }

    pub fn with_load_status_tx(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Self {
        Self::with_config(
            control_rx,
            sample_rate,
            audio_render_quantum_frames(DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES),
            load_tx,
        )
    }

    fn with_config(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Self {
        let (retired_tx, shutdown_tx) = source_worker_reaper::spawn_inline_reaper();
        Self::with_retirement_sender(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            retired_tx,
            shutdown_tx,
        )
    }

    fn with_retirement_sender(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        retired_tx: Sender<RetiredAudioItem>,
        shutdown_tx: Sender<SourceShutdownEnvelope>,
    ) -> Self {
        Self::with_engine(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            SynthEngine::new(sample_rate),
            EngineSourceWorkerState::inline(),
            SourceRetirementChannels {
                retired_tx,
                shutdown_tx,
            },
        )
    }

    fn with_engine(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        engine: SynthEngine,
        worker_state: EngineSourceWorkerState,
        retirement: SourceRetirementChannels,
    ) -> Self {
        let SourceRetirementChannels {
            retired_tx,
            shutdown_tx,
        } = retirement;
        Self {
            engine,
            worker_state,
            control_rx,
            sample_rate,
            block_frames,
            cached_profile_snapshot: SynthProfileSnapshot::default(),
            buf: Vec::with_capacity(block_frames * 2),
            left_buf: Vec::with_capacity(block_frames),
            right_buf: Vec::with_capacity(block_frames),
            idx: 0,
            load_tx,
            last_load_report: Instant::now(),
            telemetry: EngineTelemetry::default(),
            retired_tx,
            retired_backlog: Some(RetiredAudioBacklog::new()),
            shutdown_tx: Some(shutdown_tx),
            retirement_disconnected: false,
            #[cfg(test)]
            retired_drop_probe: None,
        }
    }

    fn refill(&mut self) {
        let t0 = Instant::now();
        let drained = match self.worker_state.mode {
            EngineSourceMode::Inline => {
                let drained = self.drain_control_events();
                self.refill_inline();
                drained
            }
            EngineSourceMode::Persistent => self.refill_persistent(),
        };
        if !self.engine.pending_render_retired_is_empty()
            && self.retirement_storage_can_accept_item()
        {
            let retired = self.engine.take_pending_render_retired();
            self.retire_state(retired);
        }
        if matches!(self.worker_state.mode, EngineSourceMode::Persistent) {
            self.refresh_persistent_profile_cache();
        }
        self.idx = 0;
        let elapsed = t0.elapsed().as_secs_f32();
        let block_seconds = (self.block_frames as f32) / (self.sample_rate as f32);
        let ratio = if block_seconds > 0.0 {
            elapsed / block_seconds
        } else {
            0.0
        };
        self.telemetry
            .observe_block(ratio, drained.control_events, drained.config_events);
        self.engine.set_runtime_load_ratio(ratio);
        self.report_load_status();
    }

    fn refill_inline(&mut self) {
        if self.engine.is_idle() {
            self.buf.resize(self.block_frames * 2, 0.0);
            self.buf.fill(0.0);
            self.left_buf.clear();
            self.right_buf.clear();
        } else {
            self.engine.render_interleaved_block(
                self.block_frames,
                &mut self.left_buf,
                &mut self.right_buf,
                &mut self.buf,
            );
        }
    }

    fn refill_persistent(&mut self) -> DrainedControlEvents {
        let Self {
            engine,
            worker_state,
            control_rx,
            retired_tx,
            retired_backlog,
            retirement_disconnected,
            #[cfg(test)]
            retired_drop_probe,
            block_frames,
            cached_profile_snapshot,
            buf,
            left_buf,
            right_buf,
            ..
        } = self;
        let Some(worker) = worker_state.worker.as_mut() else {
            return DrainedControlEvents::default();
        };
        let runtime = &mut worker.runtime;
        let mut controls = control_drain::ControlDrain::new(
            control_rx,
            retired_tx,
            retired_backlog.as_mut().expect("retired backlog"),
            retirement_disconnected,
            #[cfg(test)]
            retired_drop_probe.clone(),
        );
        let cached = *cached_profile_snapshot;
        let (drained, profile_snapshot) = match runtime.with_controls_ready(engine, |engine| {
            let drained = controls.drain(engine);
            (drained, engine.profile_snapshot())
        }) {
            Some(result) => result,
            None => (DrainedControlEvents::default(), cached),
        };
        *cached_profile_snapshot = profile_snapshot;
        debug_assert!((MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(block_frames));
        buf.resize(*block_frames * 2, 0.0);
        left_buf.resize(*block_frames, 0.0);
        right_buf.resize(*block_frames, 0.0);
        #[cfg(feature = "source-worker-benchmark-timing")]
        let engine_block_started_at = runtime.timing_block_start();
        engine.render_interleaved_block_with_source_runtime(
            runtime,
            *block_frames,
            left_buf,
            right_buf,
            buf,
        );
        #[cfg(feature = "source-worker-benchmark-timing")]
        runtime.record_engine_block_total(engine_block_started_at);
        drained
    }

    fn report_load_status(&mut self) {
        if self.load_tx.is_none() {
            return;
        }
        if self.last_load_report.elapsed() < LOAD_REPORT_INTERVAL {
            return;
        }
        self.last_load_report = Instant::now();
        let mut status = self.engine.audio_load_status();
        self.telemetry.apply_to_status(&mut status);
        if let Some(load_tx) = &self.load_tx {
            load_tx.try_send(status);
        }
    }

    fn retire_state(&mut self, state: RetiredAudioState) {
        if state.is_empty() {
            return;
        }
        self.retire_item(RetiredAudioItem {
            state: Some(state),
            event: None,
            #[cfg(test)]
            drop_probe: None,
        });
    }

    #[cfg(test)]
    fn retire_event(&mut self, event: EngineEvent) {
        self.retire_item(RetiredAudioItem {
            state: None,
            event: Some(event),
            #[cfg(test)]
            drop_probe: None,
        });
    }

    fn retire_item(&mut self, item: RetiredAudioItem) {
        #[cfg(test)]
        let mut item = item;
        #[cfg(test)]
        {
            item.drop_probe =
                self.retired_drop_probe
                    .as_ref()
                    .map(|drop_tx| RetiredAudioDropProbe {
                        drop_tx: drop_tx.clone(),
                    });
        }
        let Some(backlog) = self.retired_backlog.as_mut() else {
            return;
        };
        if self.retirement_disconnected {
            let _ = backlog.enqueue(item);
            return;
        }
        backlog.flush(&self.retired_tx, &mut self.retirement_disconnected);
        match self.retired_tx.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(item)) => {
                let _ = backlog.enqueue(item);
            }
            Err(TrySendError::Disconnected(item)) => {
                self.retirement_disconnected = true;
                let _ = backlog.enqueue(item);
            }
        }
    }

    fn retirement_storage_can_accept_item(&mut self) -> bool {
        let Some(backlog) = self.retired_backlog.as_mut() else {
            return false;
        };
        backlog.flush(&self.retired_tx, &mut self.retirement_disconnected);
        backlog.len < RETIREMENT_BACKLOG_CAPACITY
    }

    fn drain_control_events(&mut self) -> DrainedControlEvents {
        let mut controls = control_drain::ControlDrain::new(
            &mut self.control_rx,
            &self.retired_tx,
            self.retired_backlog.as_mut().expect("retired backlog"),
            &mut self.retirement_disconnected,
            #[cfg(test)]
            self.retired_drop_probe.clone(),
        );
        controls.drain(&mut self.engine)
    }
}

pub(crate) fn drop_retired_item(item: RetiredAudioItem) {
    drop(item.state);
    drop(item.event);
    #[cfg(test)]
    drop(item.drop_probe);
}

impl Iterator for EngineSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.buf.len() {
            self.refill();
        }
        let v = self.buf.get(self.idx).copied().unwrap_or(0.0);
        self.idx += 1;
        Some(v)
    }
}

impl rodio::Source for EngineSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "e2e_tests.rs"]
mod e2e_tests;
