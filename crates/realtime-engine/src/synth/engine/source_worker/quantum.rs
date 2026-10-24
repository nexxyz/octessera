#[cfg(any(test, feature = "test-support"))]
use super::super::source_worker_bus;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_protocol::WorkerPhase;
#[cfg(any(test, feature = "test-support"))]
use super::super::source_worker_protocol::{SourceWorkerRenderDisposition, WorkStamp};
use super::super::SynthEngine;
use super::{SourceWorkerRuntime, SOURCE_WORKER_COUNT};
use crossbeam_channel::TryRecvError;
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(super) fn collect_wave_with_deadline(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
        phase: WorkerPhase,
        deadline: Instant,
    ) -> Option<()> {
        if self.expected_phase != Some(phase) {
            self.latch_completion_failure(0b11);
            return None;
        }
        while self.in_flight_mask != 0 {
            for parity in 0..SOURCE_WORKER_COUNT {
                if self.in_flight_mask & (1 << parity) == 0 || !self.home_is_empty(parity) {
                    continue;
                }
                let receive_result = self
                    .done_rxs
                    .as_ref()
                    .map(|done_rxs| done_rxs[parity].try_recv());
                match receive_result {
                    Some(Ok(completion)) => self.accept_completion(parity, completion),
                    Some(Err(TryRecvError::Empty)) | None => {}
                    Some(Err(TryRecvError::Disconnected)) => {
                        self.latch_completion_failure(1 << parity);
                        self.in_flight_mask &= !(1 << parity);
                    }
                }
            }
            if self.health.status() != SourceWorkerHealth::Healthy {
                if self.health.status().is_recovering() {
                    return None;
                }
                self.reclaim_available(engine);
            }
            if self.in_flight_mask == 0 {
                break;
            }
            if !wait && self.health.status() == SourceWorkerHealth::Healthy {
                break;
            }
            if Instant::now() >= deadline {
                if self.health.status() == SourceWorkerHealth::Healthy {
                    self.latch_deadline_or_exit();
                }
                if self.health.status().is_recovering() {
                    return None;
                }
                self.reclaim_available(engine);
                return None;
            }
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            if self.health.status().is_recovering() {
                return None;
            }
            self.reclaim_available(engine);
            return None;
        }
        if self.in_flight_mask != 0 {
            return None;
        }
        Some(())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(in crate::synth::engine) fn render_persistent_block(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) -> SourceWorkerRenderDisposition {
        #[cfg(any(test, feature = "test-support"))]
        self.render_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.mode != super::super::source_worker_protocol::SourceWorkerMode::Persistent {
            return SourceWorkerRenderDisposition::Fatal;
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            self.reclaim_available(engine);
            return if self.health.status().is_recovering() {
                SourceWorkerRenderDisposition::Recovering
            } else {
                SourceWorkerRenderDisposition::Fatal
            };
        }
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES
            || engine.bus_pan_pos.len() > super::super::super::types::BUS_COUNT
            || left.len() < frames
            || right.len() < frames
        {
            self.latch_invalid_block();
            return SourceWorkerRenderDisposition::Fatal;
        }
        if self.in_flight_mask != 0 || self.completed_mask != 0 || !self.home_is_ready() {
            self.latch_dispatch_failure(0b11);
            return SourceWorkerRenderDisposition::Fatal;
        }
        if !engine.block_slot_scratch.prepare_output(frames) {
            self.latch_invalid_block();
            return SourceWorkerRenderDisposition::Fatal;
        }
        let operation_started_at = Instant::now();
        let deadline = operation_started_at + self.rendezvous_deadline(frames);
        #[cfg(test)]
        let mut deadline = deadline;
        self.expected_stamp = Some(WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: engine.render_plan.generation,
            quantum_sequence: self.next_sequence,
            frames,
            base_sample_clock: engine.sample_clock,
        });
        if !self.dispatch_sources(engine, operation_started_at, deadline) {
            self.reclaim_available(engine);
            return self.render_failure_disposition();
        }
        if self
            .collect_wave_with_deadline(engine, true, WorkerPhase::Sources, deadline)
            .is_none()
        {
            return self.render_failure_disposition();
        }
        if self
            .finish_source_wave(engine, |engine, _, carriers| {
                source_worker_bus::stage_source_block(engine, carriers, frames, left, right)
                    .then_some(())
                    .ok_or(())
            })
            .is_none()
        {
            self.reclaim_available(engine);
            return self.render_failure_disposition();
        }
        let stamp = self.expected_stamp.expect("persistent source stamp");
        #[cfg(test)]
        if let Some(hook) = self.before_bus_dispatch.take() {
            hook(self, &mut deadline);
        }
        if !self.dispatch_buses(engine, stamp) {
            self.reclaim_available(engine);
            return self.render_failure_disposition();
        }
        #[cfg(test)]
        if let Some(hook) = self.after_bus_dispatch.take() {
            hook(self);
        }
        if self
            .collect_wave_with_deadline(engine, true, WorkerPhase::Buses, deadline)
            .is_none()
        {
            return self.render_failure_disposition();
        }
        if !self.finish_bus_wave(engine, frames, left, right) {
            self.reclaim_available(engine);
            return self.render_failure_disposition();
        }
        for (frame, preview) in engine.block_slot_scratch.preview_out[..frames]
            .iter_mut()
            .enumerate()
        {
            left[frame] += *preview;
            right[frame] += *preview;
            *preview = 0.0;
        }
        engine.finish_persistent_block(frames, left, right);
        SourceWorkerRenderDisposition::Fresh
    }

    #[cfg(any(test, feature = "test-support"))]
    fn render_failure_disposition(&self) -> SourceWorkerRenderDisposition {
        if self.health.status() == SourceWorkerHealth::DeadlineMiss {
            SourceWorkerRenderDisposition::NewlyMissed
        } else {
            SourceWorkerRenderDisposition::Fatal
        }
    }
}
