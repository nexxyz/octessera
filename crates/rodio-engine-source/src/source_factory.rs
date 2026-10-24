use super::*;
#[cfg(any(test, feature = "routing-tree-executor"))]
use crossbeam_channel::bounded;

impl EngineSource {
    #[cfg(test)]
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
            Box::new(SynthEngine::new(sample_rate)),
        )
    }

    #[cfg(test)]
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
            Box::new(SynthEngine::new(sample_rate)),
            start_hook,
        )
    }

    #[cfg(test)]
    pub fn with_persistent_workers_exact_frames(
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
            Box::new(SynthEngine::new(sample_rate)),
        )
    }

    #[cfg(feature = "routing-tree-executor")]
    pub fn with_routing_tree_persistent_workers(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        Self::with_routing_tree_persistent_workers_impl(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            None,
        )
    }

    #[cfg(feature = "routing-tree-executor")]
    pub fn with_routing_tree_persistent_workers_with_hook(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        Self::with_routing_tree_persistent_workers_impl(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            Some(start_hook),
        )
    }

    #[cfg(feature = "routing-tree-executor")]
    fn with_routing_tree_persistent_workers_impl(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        start_hook: Option<SourceWorkerStartHook>,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        Self::validate_routing_tree_block_frames(block_frames)?;
        let mut engine = Box::new(SynthEngine::new(sample_rate));
        let (lifecycle, runtime) = match start_hook {
            Some(start_hook) => SourceWorkerLifecycle::start_routing_tree_prewarmed_with_hook(
                &mut engine,
                block_frames,
                start_hook,
            )?,
            None => SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, block_frames)?,
        };
        #[cfg(test)]
        let mut runtime = runtime;
        #[cfg(test)]
        runtime.set_deadline_for_test(Duration::from_secs(1));
        Self::finish_workers(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            lifecycle,
            EngineSourceWorkerState::routing_tree_persistent(runtime),
        )
    }

    #[cfg(feature = "routing-tree-executor")]
    fn validate_routing_tree_block_frames(
        block_frames: usize,
    ) -> Result<(), SourceWorkerSetupError> {
        if (MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(&block_frames) {
            Ok(())
        } else {
            Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested: block_frames,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            })
        }
    }

    #[cfg(test)]
    pub(crate) fn with_persistent_workers_with_engine(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        mut engine: Box<SynthEngine>,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        let (lifecycle, runtime) =
            SourceWorkerLifecycle::start_prewarmed_with_frames(&mut engine, block_frames)?;
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

    #[cfg(test)]
    fn finish_persistent_workers(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        engine: Box<SynthEngine>,
        lifecycle: SourceWorkerLifecycle,
        runtime: SourceWorkerRuntime,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        Self::finish_workers(
            control_rx,
            sample_rate,
            block_frames,
            load_tx,
            engine,
            lifecycle,
            EngineSourceWorkerState::persistent(runtime),
        )
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    fn finish_workers(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        engine: Box<SynthEngine>,
        lifecycle: SourceWorkerLifecycle,
        mut worker_state: EngineSourceWorkerState,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_owner) =
            match source_worker_reaper::spawn_persistent_reaper(lifecycle, retired_rx, false) {
                Ok(result) => result,
                Err(failure) => {
                    let source_worker_reaper::PersistentReaperSpawnFailure { lifecycle, error } =
                        *failure;
                    let _ = worker_state.retire();
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
            worker_state,
            SourceRetirementChannels {
                retired_tx,
                shutdown_tx,
            },
        );
        Ok((source, shutdown_owner))
    }

    #[cfg(test)]
    pub(crate) fn with_persistent_workers_with_engine_and_hook(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        mut engine: Box<SynthEngine>,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(Self, EngineSourceWorkerShutdownOwner), SourceWorkerSetupError> {
        let (lifecycle, runtime) = SourceWorkerLifecycle::start_prewarmed_with_frames_and_hook(
            &mut engine,
            block_frames,
            start_hook,
        )?;
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
