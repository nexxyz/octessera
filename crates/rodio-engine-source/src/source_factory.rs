use super::*;

impl EngineSource {
    pub fn with_persistent_workers(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        Self::with_persistent_workers_with_engine(
            control_rx,
            sample_rate,
            block_frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES),
            load_tx,
            SynthEngine::new(sample_rate),
        )
    }

    pub fn with_persistent_workers_with_hook(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        Self::with_persistent_workers_with_engine_and_hook(
            control_rx,
            sample_rate,
            block_frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES),
            load_tx,
            SynthEngine::new(sample_rate),
            start_hook,
        )
    }

    pub fn with_persistent_workers_for_benchmark(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        if !(MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(&block_frames) {
            return Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested: block_frames,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            });
        }
        Self::with_persistent_workers_with_engine(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            SynthEngine::new(sample_rate),
        )
    }

    pub fn with_persistent_workers_for_benchmark_with_hook(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        if !(MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(&block_frames) {
            return Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested: block_frames,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            });
        }
        Self::with_persistent_workers_with_engine_and_hook(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            SynthEngine::new(sample_rate),
            start_hook,
        )
    }

    pub(crate) fn with_persistent_workers_with_engine(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        mut engine: SynthEngine,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        let (lifecycle, runtime) = SourceWorkerLifecycle::start_prewarmed(&mut engine)?;
        Self::finish_persistent_workers(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            lifecycle,
            runtime,
        )
    }

    #[cfg(feature = "source-worker-benchmark-timing")]
    pub fn with_persistent_workers_for_benchmark_with_timing_probe(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        timing_probe: Arc<SourceWorkerTimingProbe>,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        if !(MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(&block_frames) {
            return Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested: block_frames,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            });
        }
        let mut engine = SynthEngine::new(sample_rate);
        let (lifecycle, mut runtime) = SourceWorkerLifecycle::start_prewarmed(&mut engine)?;
        runtime.attach_timing_probe(timing_probe);
        Self::finish_persistent_workers(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            lifecycle,
            runtime,
        )
    }

    #[cfg(feature = "source-worker-benchmark-timing")]
    pub fn with_persistent_workers_for_benchmark_with_timing_probe_and_hook(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        timing_probe: Arc<SourceWorkerTimingProbe>,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        if !(MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(&block_frames) {
            return Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested: block_frames,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            });
        }
        let mut engine = SynthEngine::new(sample_rate);
        let (lifecycle, mut runtime) =
            SourceWorkerLifecycle::start_prewarmed_with_hook(&mut engine, start_hook)?;
        runtime.attach_timing_probe(timing_probe);
        Self::finish_persistent_workers(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            lifecycle,
            runtime,
        )
    }

    fn finish_persistent_workers(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        engine: SynthEngine,
        lifecycle: SourceWorkerLifecycle,
        runtime: SourceWorkerRuntime,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_owner) =
            match source_worker_reaper::spawn_persistent_reaper(lifecycle, retired_rx, false) {
                Ok(result) => result,
                Err(failure) => {
                    let source_worker_reaper::PersistentReaperSpawnFailure { lifecycle, error } =
                        *failure;
                    let _ = runtime.retire();
                    let _ = lifecycle.shutdown_after_runtime_drop();
                    return Err(error);
                }
            };
        let source = Self::with_engine(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            EngineSourceWorkerState::persistent(runtime),
            SourceRetirementChannels {
                retired_tx,
                shutdown_tx,
            },
        );
        Ok((source, shutdown_owner))
    }

    pub(crate) fn with_persistent_workers_with_engine_and_hook(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        mut engine: SynthEngine,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        let (lifecycle, runtime) =
            SourceWorkerLifecycle::start_prewarmed_with_hook(&mut engine, start_hook)?;
        let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_owner) =
            match source_worker_reaper::spawn_persistent_reaper(lifecycle, retired_rx, false) {
                Ok(result) => result,
                Err(failure) => {
                    let source_worker_reaper::PersistentReaperSpawnFailure { lifecycle, error } =
                        *failure;
                    let _ = runtime.retire();
                    let _ = lifecycle.shutdown_after_runtime_drop();
                    return Err(error);
                }
            };
        let source = Self::with_engine(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            EngineSourceWorkerState::persistent(runtime),
            SourceRetirementChannels {
                retired_tx,
                shutdown_tx,
            },
        );
        Ok((source, shutdown_owner))
    }
}
