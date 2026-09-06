use super::source_lane_renderer::SynthSourceContext;
use super::source_worker_health::SourceWorkerHealth;
use super::source_worker_protocol::{SourceWorkerMode, SourceWorkerRenderDisposition};
use super::SynthEngine;
use super::{SourceWorkerRuntime, BLOCK_SLOT_SCRATCH_FRAMES};

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
            None::<fn(&mut SynthEngine) -> Result<(), ()>>,
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
            Some(apply_controls),
        )
    }

    fn render_interleaved_block_with_source_runtime_ready_inner<F>(
        &mut self,
        runtime: &mut SourceWorkerRuntime,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
        apply_controls: Option<F>,
    ) -> SourceWorkerRenderDisposition
    where
        F: FnOnce(&mut SynthEngine) -> Result<(), ()>,
    {
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
            #[cfg(all(
                feature = "routing-tree-benchmark",
                feature = "source-worker-benchmark-timing"
            ))]
            let routing_coordinator_remainder_started_at =
                runtime.take_routing_coordinator_remainder_started_at();
            if disposition != SourceWorkerRenderDisposition::Fresh {
                left.fill(0.0);
                right.fill(0.0);
            }
            crate::simd::interleave_stereo(left, right, out);
            #[cfg(all(
                feature = "routing-tree-benchmark",
                feature = "source-worker-benchmark-timing"
            ))]
            if disposition == SourceWorkerRenderDisposition::Fresh {
                runtime
                    .record_routing_coordinator_remainder(routing_coordinator_remainder_started_at);
            }
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
