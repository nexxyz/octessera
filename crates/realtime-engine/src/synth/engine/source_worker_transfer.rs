use super::super::types::INSTRUMENT_SLOT_COUNT;
use super::source_lane_renderer::SynthSourceContext;
use super::source_worker::SourceWorkerRuntime;
use super::source_worker_health::SourceWorkerHealth;
use super::source_worker_lease::OwnerLease;
use super::source_worker_lifecycle::{
    OwnerEnvelope, SourceLanePartitionBundle, SourceWorkerLifecycle, SourceWorkerScratch,
};
use super::source_worker_protocol::{
    SourceWorkerMode, SourceWorkerSetupError, SourceWorkerStartHook,
};
use super::SynthEngine;
use super::BLOCK_SLOT_SCRATCH_FRAMES;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

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
    let Some(first_owner) = first.take_owner() else {
        return Err(());
    };
    let Some(second_owner) = second.take_owner() else {
        first.restore_owner(first_owner);
        return Err(());
    };
    if !valid_owner_pair(engine, &first_owner, &second_owner) {
        first.restore_owner(first_owner);
        second.restore_owner(second_owner);
        return Err(());
    }
    let OwnerEnvelope {
        parity: first_parity,
        partitions: first_partitions,
        scratch: first_scratch,
    } = first_owner;
    let OwnerEnvelope {
        parity: second_parity,
        partitions: second_partitions,
        scratch: second_scratch,
    } = second_owner;
    let operation_result = catch_unwind(AssertUnwindSafe(|| {
        install_source_partition_bundle_after_check(engine, first_parity, first_partitions);
        install_source_partition_bundle_after_check(engine, second_parity, second_partitions);
        let result = operation(engine, [&first_scratch, &second_scratch]);
        let partitions = take_source_partition_bundles(engine);
        (result, partitions)
    }));
    match operation_result {
        Ok((result, Some((first_partitions, second_partitions)))) => {
            first.restore_owner(OwnerEnvelope {
                parity: first_parity,
                partitions: first_partitions,
                scratch: first_scratch,
            });
            second.restore_owner(OwnerEnvelope {
                parity: second_parity,
                partitions: second_partitions,
                scratch: second_scratch,
            });
            Ok(result)
        }
        Ok((_, None)) => Err(()),
        Err(payload) => {
            if let Some((first_partitions, second_partitions)) =
                take_source_partition_bundles(engine)
            {
                first.restore_owner(OwnerEnvelope {
                    parity: first_parity,
                    partitions: first_partitions,
                    scratch: first_scratch,
                });
                second.restore_owner(OwnerEnvelope {
                    parity: second_parity,
                    partitions: second_partitions,
                    scratch: second_scratch,
                });
            }
            resume_unwind(payload);
        }
    }
}

pub(super) fn with_both_source_partitions_read_only<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    inspect: impl FnOnce(&SynthEngine) -> R,
) -> Result<R, ()> {
    with_both_source_partitions(engine, first, second, |engine, _| inspect(engine))
}

fn valid_owner_pair(engine: &SynthEngine, first: &OwnerEnvelope, second: &OwnerEnvelope) -> bool {
    first.parity == 0
        && second.parity == 1
        && first.partitions.synth.parity() == first.parity
        && first.partitions.sample.parity() == first.parity
        && second.partitions.synth.parity() == second.parity
        && second.partitions.sample.parity() == second.parity
        && can_install_source_partition_bundle(engine, first.parity)
        && can_install_source_partition_bundle(engine, second.parity)
        && source_partitions_vacant(engine)
}

fn install_source_partition_bundle_after_check(
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

fn take_source_partition_bundles(
    engine: &mut SynthEngine,
) -> Option<(SourceLanePartitionBundle, SourceLanePartitionBundle)> {
    let first = take_source_partition_bundle(engine, 0)?;
    let Some(second) = take_source_partition_bundle(engine, 1) else {
        restore_source_partition_bundle(engine, first);
        return None;
    };
    Some((first, second))
}

fn restore_source_partition_bundle(
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
        Self::start_prewarmed_with_options(engine, false, None, None)
    }

    pub fn start_prewarmed_with_hook(
        engine: &mut SynthEngine,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(engine, false, None, Some(start_hook))
    }

    #[cfg(any(test, feature = "test-support"))]
    fn start_prewarmed_with_hold(
        engine: &mut SynthEngine,
        hold_before_receive: bool,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(engine, hold_before_receive, None, None)
    }

    fn start_prewarmed_with_options(
        engine: &mut SynthEngine,
        hold_before_receive: bool,
        #[cfg(test)] disconnected_completion: Option<usize>,
        #[cfg(not(test))] _disconnected_completion: Option<usize>,
        start_hook: Option<SourceWorkerStartHook>,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
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
        let Some(home_partitions) = take_source_partition_bundles(engine) else {
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::PartitionsUnavailable);
        };
        let Some((synth_scratch, sample_scratch)) = engine.take_inline_source_scratch() else {
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
        if !lifecycle.seed_home([
            OwnerEnvelope {
                parity: 0,
                partitions: home_partitions.0,
                scratch: first_scratch,
            },
            OwnerEnvelope {
                parity: 1,
                partitions: home_partitions.1,
                scratch: second_scratch,
            },
        ]) {
            lifecycle.mark_runtime_closed();
            return Err(SourceWorkerSetupError::WorkerChannelsUnavailable);
        }
        let Some(runtime) = SourceWorkerRuntime::new(&lifecycle, engine.sample_rate) else {
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
        Self::start_prewarmed_with_options(engine, false, Some(parity), None)
    }

    #[cfg(test)]
    pub(crate) fn start_prewarmed_disconnected_held_for_test(
        engine: &mut SynthEngine,
        parity: usize,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        Self::start_prewarmed_with_options(engine, true, Some(parity), None)
    }
}

impl SynthEngine {
    fn take_inline_source_scratch(
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
    ) {
        if runtime.mode() == SourceWorkerMode::Inline {
            self.render_interleaved_block(frames, left, right, out);
            return;
        }
        if frames > BLOCK_SLOT_SCRATCH_FRAMES
            || runtime.health_snapshot().status != SourceWorkerHealth::Healthy
        {
            let _ = runtime.render_source_block(self, frames);
            left.fill(0.0);
            right.fill(0.0);
            out.fill(0.0);
            return;
        }
        left.resize(frames, 0.0);
        right.resize(frames, 0.0);
        out.resize(frames * 2, 0.0);
        assert!(self.block_slot_scratch.prepare_output(frames));
        if runtime.render_source_block(self, frames) {
            self.finish_block_slot_frame_graph(frames, left, right);
        } else {
            left.fill(0.0);
            right.fill(0.0);
        }
        crate::simd::interleave_stereo(left, right, out);
    }
}
