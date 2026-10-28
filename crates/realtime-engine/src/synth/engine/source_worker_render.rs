use super::source_worker_protocol::{SourceWorkerMode, SourceWorkerRenderDisposition};
use super::SynthEngine;
use super::{SourceWorkerRuntime, BLOCK_SLOT_SCRATCH_FRAMES};

impl SynthEngine {
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
        if runtime.mode() == SourceWorkerMode::Inline {
            self.render_interleaved_block(frames, left, right, out);
            return SourceWorkerRenderDisposition::Fresh;
        }
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            left.fill(0.0);
            right.fill(0.0);
            out.fill(0.0);
            return SourceWorkerRenderDisposition::Fatal;
        }
        left.resize(frames, 0.0);
        right.resize(frames, 0.0);
        out.resize(frames * 2, 0.0);
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
        disposition
    }
}
