use super::super::source_worker_bus;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_protocol::{WorkStamp, WorkerPhase};
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
        record_dispatch_timing: bool,
    ) -> Option<()> {
        if self.expected_phase != Some(phase) {
            self.latch_completion_failure(0b11);
            return None;
        }
        if record_dispatch_timing {
            #[cfg(feature = "source-worker-benchmark-timing")]
            {
                let deadline_start = Instant::now();
                self.record_dispatch_to_deadline_start(deadline_start, deadline);
            }
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
                #[cfg(feature = "source-worker-benchmark-timing")]
                self.freeze_timing(
                    true,
                    (self.health.status() == SourceWorkerHealth::DeadlineMiss)
                        .then(|| self.dispatch_elapsed())
                        .flatten(),
                );
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
                #[cfg(feature = "source-worker-benchmark-timing")]
                self.freeze_timing(true, self.dispatch_elapsed());
                self.reclaim_available(engine);
                return None;
            }
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            self.reclaim_available(engine);
            return None;
        }
        if self.in_flight_mask != 0 {
            return None;
        }
        Some(())
    }

    pub(in crate::synth::engine) fn render_persistent_block(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        #[cfg(any(test, feature = "test-support"))]
        self.render_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.mode != super::super::source_worker_protocol::SourceWorkerMode::Persistent {
            return false;
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            self.reclaim_available(engine);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES
            || engine.bus_pan_pos.len() > super::super::super::types::BUS_COUNT
            || left.len() < frames
            || right.len() < frames
        {
            self.latch_invalid_block();
            return false;
        }
        if self.in_flight_mask != 0 || self.completed_mask != 0 || !self.home_is_ready() {
            self.latch_dispatch_failure(0b11);
            return false;
        }
        if !engine.block_slot_scratch.prepare_output(frames) {
            self.latch_invalid_block();
            return false;
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
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        if self
            .collect_wave_with_deadline(engine, true, WorkerPhase::Sources, deadline, true)
            .is_none()
        {
            return false;
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
            return false;
        }
        let stamp = self.expected_stamp.expect("persistent source stamp");
        #[cfg(test)]
        if let Some(hook) = self.before_bus_dispatch.take() {
            hook(self, &mut deadline);
        }
        if !self.dispatch_buses(engine, stamp) {
            self.reclaim_available(engine);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, self.dispatch_elapsed());
            return false;
        }
        #[cfg(test)]
        if let Some(hook) = self.after_bus_dispatch.take() {
            hook(self);
        }
        if self
            .collect_wave_with_deadline(engine, true, WorkerPhase::Buses, deadline, false)
            .is_none()
        {
            return false;
        }
        if !self.finish_bus_wave(engine, frames, left, right) {
            self.reclaim_available(engine);
            return false;
        }
        engine.finish_persistent_block(frames, left, right);
        true
    }
}
