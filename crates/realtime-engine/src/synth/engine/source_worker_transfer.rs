use super::super::types::INSTRUMENT_SLOT_COUNT;
use super::source_lane_renderer::SynthSourceContext;
use super::source_worker::SourceWorkerRuntime;
use super::source_worker_health::SourceWorkerHealth;
use super::source_worker_lease::OwnerLease;
use super::source_worker_lifecycle::{
    OwnerEnvelope, SourceLanePartitionBundle, SourceWorkerLifecycle, SourceWorkerScratch,
};
use super::source_worker_protocol::{
    SourceWorkerMode, SourceWorkerRenderDisposition, SourceWorkerSetupError, SourceWorkerStartHook,
};
use super::SynthEngine;
use super::BLOCK_SLOT_SCRATCH_FRAMES;

pub(super) fn take_source_partition_bundle(
    engine: &mut SynthEngine,
    parity: usize,
) -> Option<SourceLanePartitionBundle> {
    if parity >= 2
        || !engine.synth_voice_pool.partition_is_present(parity)
        || !engine.sample_voice_pool.partition_is_present(parity)
    {
        return None;
    }
    let synth = engine.synth_voice_pool.take_partition(parity)?;
    let Some(sample) = engine.sample_voice_pool.take_partition(parity) else {
        engine
            .synth_voice_pool
            .install_partition_after_vacancy_check(parity, synth);
        return None;
    };
    Some(SourceLanePartitionBundle { synth, sample })
}

pub(super) fn can_install_source_partition_bundle(engine: &SynthEngine, parity: usize) -> bool {
    parity < 2
        && engine.synth_voice_pool.partition_is_vacant(parity)
        && engine.sample_voice_pool.partition_is_vacant(parity)
}

pub(super) fn source_partitions_vacant(engine: &SynthEngine) -> bool {
    (0..2).all(|parity| {
        engine.synth_voice_pool.partition_is_vacant(parity)
            && engine.sample_voice_pool.partition_is_vacant(parity)
    })
}

pub(super) fn with_both_source_partitions<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    operation: impl FnOnce(&mut SynthEngine, [&SourceWorkerScratch; 2]) -> R,
) -> Result<R, ()> {
    super::source_worker_carrier_transfer::with_both_source_owners(
        engine,
        first,
        second,
        |engine, scratch, _| operation(engine, scratch),
    )
}

pub(super) fn with_both_source_partitions_read_only<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    inspect: impl FnOnce(&SynthEngine) -> R,
) -> Result<R, ()> {
    super::source_worker_carrier_transfer::with_both_source_owners(
        engine,
        first,
        second,
        |engine, _, _| inspect(engine),
    )
}

pub(super) fn install_source_partition_bundle_after_check(
    engine: &mut SynthEngine,
    parity: usize,
    partitions: SourceLanePartitionBundle,
) {
    let SourceLanePartitionBundle { synth, sample } = partitions;
    engine
        .sample_voice_pool
        .install_partition_after_vacancy_check(parity, sample);
    engine
        .synth_voice_pool
        .install_partition_after_vacancy_check(parity, synth);
}

pub(super) fn take_source_partition_bundles(
    engine: &mut SynthEngine,
) -> Option<(SourceLanePartitionBundle, SourceLanePartitionBundle)> {
    let first = take_source_partition_bundle(engine, 0)?;
    let Some(second) = take_source_partition_bundle(engine, 1) else {
        restore_source_partition_bundle(engine, first);
        return None;
    };
    Some((first, second))
}

pub(super) fn restore_source_partition_bundle(
    engine: &mut SynthEngine,
    partitions: SourceLanePartitionBundle,
) {
    let SourceLanePartitionBundle { synth, sample } = partitions;
    let parity = synth.parity();
    engine
        .sample_voice_pool
        .install_partition_after_vacancy_check(parity, sample);
    engine
        .synth_voice_pool
        .install_partition_after_vacancy_check(parity, synth);
}

pub(super) fn compact_source_pools(engine: &mut SynthEngine) {
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        engine.synth_voice_pool.compact_slot_lanes(slot);
        engine.sample_voice_pool.compact_slot_lanes(slot);
        engine.active_synth_slots[slot] = false;
        engine.active_sample_slots[slot] = false;
    }
}

impl SourceWorkerLifecycle {
    pub fn start_prewarmed(
        engine: &mut SynthEngine,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_frames(
            engine,
            super::super::types::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES,
        )
    }

    pub fn start_prewarmed_with_frames(
        engine: &mut SynthEngine,
        active_frames: usize,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(engine, active_frames, false, None, None)
    }

    pub fn start_prewarmed_with_hook(
        engine: &mut SynthEngine,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_frames_and_hook(
            engine,
            super::super::types::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES,
            start_hook,
        )
    }

    pub fn start_prewarmed_with_frames_and_hook(
        engine: &mut SynthEngine,
        active_frames: usize,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(engine, active_frames, false, None, Some(start_hook))
    }

    #[cfg(any(test, feature = "test-support"))]
    fn start_prewarmed_with_hold(
        engine: &mut SynthEngine,
        hold_before_receive: bool,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(
            engine,
            super::super::types::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES,
            hold_before_receive,
            None,
            None,
        )
    }

    fn start_prewarmed_with_options(
        engine: &mut SynthEngine,
        active_frames: usize,
        hold_before_receive: bool,
        #[cfg(test)] disconnected_completion: Option<usize>,
        #[cfg(not(test))] _disconnected_completion: Option<usize>,
        start_hook: Option<SourceWorkerStartHook>,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        if engine.bus_chains.len() > super::super::types::BUS_COUNT {
            return Err(SourceWorkerSetupError::UnsupportedPersistentBusCount {
                requested: engine.bus_chains.len(),
                max: super::super::types::BUS_COUNT,
            });
        }
        let mut lifecycle =
            SourceWorkerLifecycle::start_with_hold_and_hook(hold_before_receive, start_hook)?;
        if let Err(error) = lifecycle.prewarm() {
            lifecycle.mark_runtime_closed();
            let _ = lifecycle.shutdown_after_runtime_drop();
            return Err(error);
        }
        #[cfg(test)]
        if let Some(parity) = disconnected_completion {
            lifecycle.disconnect_completion_for_test(parity);
        }
        let bus_carriers = super::source_worker_carrier_transfer::take_bus_carriers(engine);
        engine
            .bus_chains
            .reserve_exact(super::super::types::BUS_COUNT);
        engine.set_persistent_bus_limit(Some(super::super::types::BUS_COUNT));
        let Some(home_partitions) = take_source_partition_bundles(engine) else {
            super::source_worker_carrier_transfer::restore_bus_carriers_to_engine(
                engine,
                bus_carriers,
            );
            engine.set_persistent_bus_limit(None);
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::PartitionsUnavailable);
        };
        let Some((synth_scratch, sample_scratch)) = engine.take_inline_source_scratch() else {
            super::source_worker_carrier_transfer::restore_bus_carriers_to_engine(
                engine,
                bus_carriers,
            );
            engine.set_persistent_bus_limit(None);
            for SourceLanePartitionBundle { synth, sample } in
                [home_partitions.0, home_partitions.1]
            {
                let parity = synth.parity();
                engine
                    .sample_voice_pool
                    .install_partition_after_vacancy_check(parity, sample);
                engine
                    .synth_voice_pool
                    .install_partition_after_vacancy_check(parity, synth);
            }
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::InlineSourceExecutorUnavailable);
        };
        let Some([first_scratch, second_scratch]) =
            SourceWorkerScratch::from_inline_scratch(synth_scratch, sample_scratch)
        else {
            super::source_worker_carrier_transfer::restore_bus_carriers_to_engine(
                engine,
                bus_carriers,
            );
            engine.set_persistent_bus_limit(None);
            for SourceLanePartitionBundle { synth, sample } in
                [home_partitions.0, home_partitions.1]
            {
                let parity = synth.parity();
                engine
                    .sample_voice_pool
                    .install_partition_after_vacancy_check(parity, sample);
                engine
                    .synth_voice_pool
                    .install_partition_after_vacancy_check(parity, synth);
            }
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::InlineSourceExecutorUnavailable);
        };
        let [first_carriers, second_carriers] =
            super::source_worker_carrier_transfer::split_bus_carriers(bus_carriers);
        if !lifecycle.seed_home([
            OwnerEnvelope {
                runtime_generation: lifecycle.runtime_generation(),
                parity: 0,
                partitions: home_partitions.0,
                scratch: first_scratch,
                bus_carriers: first_carriers,
                #[cfg(feature = "routing-tree-benchmark")]
                routing_tree: None,
            },
            OwnerEnvelope {
                runtime_generation: lifecycle.runtime_generation(),
                parity: 1,
                partitions: home_partitions.1,
                scratch: second_scratch,
                bus_carriers: second_carriers,
                #[cfg(feature = "routing-tree-benchmark")]
                routing_tree: None,
            },
        ]) {
            engine.set_persistent_bus_limit(None);
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::WorkerChannelsUnavailable);
        }
        let Some(runtime) = SourceWorkerRuntime::new(&lifecycle, engine.sample_rate, active_frames)
        else {
            engine.set_persistent_bus_limit(None);
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::WorkerChannelsUnavailable);
        };
        Ok((lifecycle, runtime))
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn start_prewarmed_held_for_test(
        engine: &mut SynthEngine,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_hold(engine, true)
    }

    #[cfg(test)]
    pub(crate) fn start_prewarmed_disconnected_for_test(
        engine: &mut SynthEngine,
        parity: usize,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(
            engine,
            super::super::types::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES,
            false,
            Some(parity),
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn start_prewarmed_disconnected_held_for_test(
        engine: &mut SynthEngine,
        parity: usize,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(
            engine,
            super::super::types::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES,
            true,
            Some(parity),
            None,
        )
    }
}

impl SynthEngine {
    pub(super) fn take_inline_source_scratch(
        &mut self,
    ) -> Option<(
        [super::source_lane_renderer::SourceLaneBlockScratch; 2],
        [super::source_lane_renderer::SourceLaneBlockScratch; 2],
    )> {
        self.block_slot_scratch
            .inline_source_executor
            .take()
            .map(super::inline_source_executor::InlineSourceExecutor::into_partition_scratch)
    }

    pub(super) fn synth_source_context(&self) -> SynthSourceContext {
        SynthSourceContext {
            sample_rate: self.sample_rate,
            configs: self.instruments,
            render_configs: self.synth_render_configs,
            revisions: self.synth_render_revisions,
            mods: self.mods,
        }
    }

    pub fn render_interleaved_block_with_source_runtime(
        &mut self,
        runtime: &mut SourceWorkerRuntime,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
    ) -> SourceWorkerRenderDisposition {
        if runtime.mode() == SourceWorkerMode::Inline {
            self.render_interleaved_block(frames, left, right, out);
            return SourceWorkerRenderDisposition::Fresh;
        }
        #[cfg(feature = "routing-tree-benchmark")]
        if runtime.mode() == SourceWorkerMode::RoutingTreePersistent {
            let _ = runtime.refresh_recovery_disposition(self);
            return self.render_interleaved_block_with_source_runtime_ready(
                runtime, frames, left, right, out,
            );
        }
        let _ = runtime.refresh_recovery_disposition(self);
        self.render_interleaved_block_with_source_runtime_ready(runtime, frames, left, right, out)
    }

    pub fn render_interleaved_block_with_source_runtime_ready(
        &mut self,
        runtime: &mut SourceWorkerRuntime,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
    ) -> SourceWorkerRenderDisposition {
        self.render_interleaved_block_with_source_runtime_ready_inner(
            runtime,
            frames,
            left,
            right,
            out,
            |_| Ok(()),
        )
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub fn render_interleaved_block_with_source_runtime_ready_with_controls(
        &mut self,
        runtime: &mut SourceWorkerRuntime,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
        apply_controls: impl FnOnce(&mut SynthEngine) -> Result<(), ()>,
    ) -> SourceWorkerRenderDisposition {
        self.render_interleaved_block_with_source_runtime_ready_inner(
            runtime,
            frames,
            left,
            right,
            out,
            apply_controls,
        )
    }

    fn render_interleaved_block_with_source_runtime_ready_inner(
        &mut self,
        runtime: &mut SourceWorkerRuntime,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
        apply_controls: impl FnOnce(&mut SynthEngine) -> Result<(), ()>,
    ) -> SourceWorkerRenderDisposition {
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let _ = &apply_controls;
        if runtime.mode() == SourceWorkerMode::Inline {
            self.render_interleaved_block(frames, left, right, out);
            return SourceWorkerRenderDisposition::Fresh;
        }
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            if runtime.health_snapshot().status == SourceWorkerHealth::Healthy {
                let _ = runtime.render_source_block(self, frames);
            }
            left.fill(0.0);
            right.fill(0.0);
            out.fill(0.0);
            return SourceWorkerRenderDisposition::Fatal;
        }
        left.resize(frames, 0.0);
        right.resize(frames, 0.0);
        out.resize(frames * 2, 0.0);
        #[cfg(feature = "routing-tree-benchmark")]
        if runtime.mode() == SourceWorkerMode::RoutingTreePersistent {
            let disposition = runtime.render_routing_tree_persistent_block(
                self,
                frames,
                &mut left[..frames],
                &mut right[..frames],
                apply_controls,
            );
            if disposition != SourceWorkerRenderDisposition::Fresh {
                left.fill(0.0);
                right.fill(0.0);
            }
            crate::simd::interleave_stereo(left, right, out);
            return disposition;
        }
        let health = runtime.health_snapshot().status;
        if health != SourceWorkerHealth::Healthy {
            if !health.is_recovering() {
                let _ = runtime.render_source_block(self, frames);
            }
            left.fill(0.0);
            right.fill(0.0);
            out.fill(0.0);
            return if health.is_recovering() {
                SourceWorkerRenderDisposition::Recovering
            } else {
                SourceWorkerRenderDisposition::Fatal
            };
        }
        let disposition = runtime.render_persistent_block(
            self,
            frames,
            &mut left[..frames],
            &mut right[..frames],
        );
        #[cfg(feature = "source-worker-benchmark-timing")]
        let coordinator_remainder_started_at = runtime.take_coordinator_remainder_started_at();
        if disposition != SourceWorkerRenderDisposition::Fresh {
            left.fill(0.0);
            right.fill(0.0);
        }
        crate::simd::interleave_stereo(left, right, out);
        #[cfg(feature = "source-worker-benchmark-timing")]
        if disposition == SourceWorkerRenderDisposition::Fresh {
            runtime.record_coordinator_remainder(coordinator_remainder_started_at);
        }
        disposition
    }
}
