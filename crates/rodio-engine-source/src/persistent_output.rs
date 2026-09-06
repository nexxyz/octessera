#[cfg(any(test, feature = "routing-tree-executor"))]
use super::control_drain;
use super::telemetry::DrainedControlEvents;
#[cfg(any(test, feature = "routing-tree-executor"))]
use super::{EngineSource, MAX_BLOCK_FRAMES, MIN_BLOCK_FRAMES, OUTPUT_CHANNELS};
use realtime_engine::synth::AudioLoadStatus;
#[cfg(any(test, feature = "routing-tree-executor"))]
use realtime_engine::synth::{SourceWorkerRenderDisposition, BLOCK_SLOT_SCRATCH_FRAMES};
use serde::{Deserialize, Serialize};

#[cfg(any(test, feature = "routing-tree-executor"))]
const _: () = assert!(BLOCK_SLOT_SCRATCH_FRAMES == super::MAX_BLOCK_FRAMES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(test, feature = "routing-tree-executor"))]
pub(super) enum PersistentOutputKind {
    Fresh,
    Repeated,
    Dropped,
    Fatal,
}

/// Cumulative output quantum counters from the persistent source cache.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistentOutputCounters {
    pub rendered_quantums: u64,
    pub repeated_quantums: u64,
    pub dropped_quantums: u64,
    pub deadline_misses: u64,
    pub deadline_recoveries: u64,
}

pub(super) struct PreviousMasterQuantum {
    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) samples: [f32; BLOCK_SLOT_SCRATCH_FRAMES * OUTPUT_CHANNELS],
    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) cached_frames: usize,
    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) valid: bool,
    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) repeat_used_for_current_recovery: bool,
    pub(super) rendered_quantums: u64,
    pub(super) repeated_quantums: u64,
    pub(super) dropped_quantums: u64,
    pub(super) deadline_misses: u64,
    pub(super) deadline_recoveries: u64,
    pub(super) flash_frames_remaining: u64,
}

impl PreviousMasterQuantum {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(any(test, feature = "routing-tree-executor"))]
            samples: [0.0; BLOCK_SLOT_SCRATCH_FRAMES * OUTPUT_CHANNELS],
            #[cfg(any(test, feature = "routing-tree-executor"))]
            cached_frames: 0,
            #[cfg(any(test, feature = "routing-tree-executor"))]
            valid: false,
            #[cfg(any(test, feature = "routing-tree-executor"))]
            repeat_used_for_current_recovery: false,
            rendered_quantums: 0,
            repeated_quantums: 0,
            dropped_quantums: 0,
            deadline_misses: 0,
            deadline_recoveries: 0,
            flash_frames_remaining: 0,
        }
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) fn fresh(&mut self, frames: usize, output: &[f32]) -> PersistentOutputKind {
        let samples = frames.saturating_mul(OUTPUT_CHANNELS);
        if frames > BLOCK_SLOT_SCRATCH_FRAMES || output.len() < samples {
            self.valid = false;
            return PersistentOutputKind::Fatal;
        }
        self.samples[..samples].copy_from_slice(&output[..samples]);
        self.cached_frames = frames;
        self.valid = true;
        self.repeat_used_for_current_recovery = false;
        self.rendered_quantums = self.rendered_quantums.saturating_add(1);
        PersistentOutputKind::Fresh
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) fn deadline_miss(
        &mut self,
        sample_rate: u32,
        frames: usize,
        output: &mut [f32],
    ) -> PersistentOutputKind {
        self.deadline_misses = self.deadline_misses.saturating_add(1);
        self.flash_frames_remaining = u64::from(sample_rate).saturating_mul(5);
        if self.valid
            && self.cached_frames == frames
            && !self.repeat_used_for_current_recovery
            && output.len() >= frames.saturating_mul(OUTPUT_CHANNELS)
        {
            let samples = frames * OUTPUT_CHANNELS;
            output[..samples].copy_from_slice(&self.samples[..samples]);
            self.repeat_used_for_current_recovery = true;
            self.repeated_quantums = self.repeated_quantums.saturating_add(1);
            PersistentOutputKind::Repeated
        } else {
            if self.valid && self.cached_frames != frames {
                self.valid = false;
            }
            output.fill(0.0);
            self.dropped_quantums = self.dropped_quantums.saturating_add(1);
            PersistentOutputKind::Dropped
        }
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) fn recovery_silence(&mut self, output: &mut [f32]) -> PersistentOutputKind {
        output.fill(0.0);
        self.dropped_quantums = self.dropped_quantums.saturating_add(1);
        PersistentOutputKind::Dropped
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) fn fatal_silence(&mut self, output: &mut [f32]) -> PersistentOutputKind {
        output.fill(0.0);
        PersistentOutputKind::Fatal
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) fn deadline_recovery(&mut self) {
        self.deadline_recoveries = self.deadline_recoveries.saturating_add(1);
        self.repeat_used_for_current_recovery = false;
    }

    pub(super) fn apply_to_status(&self, status: &mut AudioLoadStatus) {
        status.rendered_quantums = self.rendered_quantums;
        status.repeated_quantums = self.repeated_quantums;
        status.dropped_quantums = self.dropped_quantums;
        status.deadline_misses = self.deadline_misses;
        status.deadline_recoveries = self.deadline_recoveries;
        status.missed_quantum_flash = self.flash_frames_remaining > 0;
    }

    pub(super) fn counters(&self) -> PersistentOutputCounters {
        PersistentOutputCounters {
            rendered_quantums: self.rendered_quantums,
            repeated_quantums: self.repeated_quantums,
            dropped_quantums: self.dropped_quantums,
            deadline_misses: self.deadline_misses,
            deadline_recoveries: self.deadline_recoveries,
        }
    }

    pub(super) fn consume_frame(&mut self) -> bool {
        if self.flash_frames_remaining == 0 {
            return false;
        }
        self.flash_frames_remaining -= 1;
        self.flash_frames_remaining == 0
    }
}

impl Default for PreviousMasterQuantum {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) struct RefillResult {
    pub(super) drained: DrainedControlEvents,
    pub(super) force_status: bool,
    pub(super) fresh: bool,
    pub(super) defer_status: bool,
}

#[cfg(any(test, feature = "routing-tree-executor"))]
impl EngineSource {
    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn refill_routing_tree_persistent(&mut self) -> RefillResult {
        let Self {
            engine,
            worker_state,
            control_rx,
            retired_tx,
            retired_backlog,
            retirement_disconnected,
            #[cfg(test)]
            retired_drop_probe,
            sample_rate,
            block_frames,
            cached_profile_snapshot,
            buf,
            left_buf,
            right_buf,
            persistent_output,
            ..
        } = self;
        let Some(worker) = worker_state.worker.as_mut() else {
            buf.resize(*block_frames * OUTPUT_CHANNELS, 0.0);
            persistent_output.fatal_silence(buf);
            return RefillResult {
                drained: DrainedControlEvents::default(),
                force_status: false,
                fresh: false,
                defer_status: false,
            };
        };
        let runtime = &mut worker.runtime;
        let recovery = runtime.refresh_recovery_disposition(engine);
        debug_assert!((MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(block_frames));
        buf.resize(*block_frames * OUTPUT_CHANNELS, 0.0);
        left_buf.resize(*block_frames, 0.0);
        right_buf.resize(*block_frames, 0.0);
        match recovery {
            SourceWorkerRenderDisposition::Recovering => {
                persistent_output.recovery_silence(buf);
                return RefillResult {
                    drained: DrainedControlEvents::default(),
                    force_status: false,
                    fresh: false,
                    defer_status: false,
                };
            }
            SourceWorkerRenderDisposition::Fatal => {
                let _ = engine.render_interleaved_block_with_source_runtime_ready(
                    runtime,
                    *block_frames,
                    left_buf,
                    right_buf,
                    buf,
                );
                persistent_output.fatal_silence(buf);
                return RefillResult {
                    drained: DrainedControlEvents::default(),
                    force_status: false,
                    fresh: false,
                    defer_status: false,
                };
            }
            SourceWorkerRenderDisposition::Fresh
            | SourceWorkerRenderDisposition::RecoveredReady => {}
            SourceWorkerRenderDisposition::NewlyMissed => unreachable!("recovery refresh miss"),
        }
        let recovered = recovery == SourceWorkerRenderDisposition::RecoveredReady;
        if recovered {
            persistent_output.deadline_recovery();
        }
        let mut controls = control_drain::ControlDrain::new(
            control_rx,
            retired_tx,
            retired_backlog.as_mut().expect("retired backlog"),
            retirement_disconnected,
            #[cfg(test)]
            retired_drop_probe.clone(),
        );
        let effective_sample_clock = if recovered {
            engine.sample_clock()
        } else {
            engine.sample_clock().saturating_add(*block_frames as u64)
        };
        let mut drained = DrainedControlEvents::default();
        #[cfg(feature = "source-worker-benchmark-timing")]
        let engine_block_started_at = runtime.timing_block_start();
        let disposition = engine.render_interleaved_block_with_source_runtime_ready_with_controls(
            runtime,
            *block_frames,
            left_buf,
            right_buf,
            buf,
            |engine| {
                drained = controls.drain_routing_tree(engine, effective_sample_clock);
                if engine.take_routing_tree_rejection() {
                    Err(())
                } else {
                    Ok(())
                }
            },
        );
        #[cfg(feature = "source-worker-benchmark-timing")]
        runtime.record_engine_block_total(engine_block_started_at);
        *cached_profile_snapshot = engine.profile_snapshot();
        let defer_status = drained.control_events > 0 || drained.config_events > 0;
        match disposition {
            SourceWorkerRenderDisposition::Fresh => persistent_output.fresh(*block_frames, buf),
            SourceWorkerRenderDisposition::NewlyMissed => {
                persistent_output.deadline_miss(*sample_rate, *block_frames, buf)
            }
            SourceWorkerRenderDisposition::Recovering => persistent_output.recovery_silence(buf),
            SourceWorkerRenderDisposition::RecoveredReady => {
                unreachable!("recovery must precede rendering")
            }
            SourceWorkerRenderDisposition::Fatal => persistent_output.fatal_silence(buf),
        };
        RefillResult {
            drained,
            force_status: disposition == SourceWorkerRenderDisposition::NewlyMissed
                || (recovered && disposition == SourceWorkerRenderDisposition::Fresh),
            fresh: disposition == SourceWorkerRenderDisposition::Fresh,
            defer_status,
        }
    }

    #[cfg(test)]
    pub(super) fn refill_persistent(&mut self) -> RefillResult {
        let Self {
            engine,
            worker_state,
            control_rx,
            retired_tx,
            retired_backlog,
            retirement_disconnected,
            #[cfg(test)]
            retired_drop_probe,
            sample_rate,
            block_frames,
            cached_profile_snapshot,
            buf,
            left_buf,
            right_buf,
            persistent_output,
            ..
        } = self;
        let Some(worker) = worker_state.worker.as_mut() else {
            buf.resize(*block_frames * OUTPUT_CHANNELS, 0.0);
            persistent_output.fatal_silence(buf);
            return RefillResult {
                drained: DrainedControlEvents::default(),
                force_status: false,
                fresh: false,
                defer_status: false,
            };
        };
        let runtime = &mut worker.runtime;
        let recovery = runtime.refresh_recovery_disposition(engine);
        debug_assert!((MIN_BLOCK_FRAMES..=MAX_BLOCK_FRAMES).contains(block_frames));
        buf.resize(*block_frames * OUTPUT_CHANNELS, 0.0);
        left_buf.resize(*block_frames, 0.0);
        right_buf.resize(*block_frames, 0.0);
        match recovery {
            SourceWorkerRenderDisposition::Recovering => {
                persistent_output.recovery_silence(buf);
                return RefillResult {
                    drained: DrainedControlEvents::default(),
                    force_status: false,
                    fresh: false,
                    defer_status: false,
                };
            }
            SourceWorkerRenderDisposition::Fatal => {
                let _ = engine.render_interleaved_block_with_source_runtime_ready(
                    runtime,
                    *block_frames,
                    left_buf,
                    right_buf,
                    buf,
                );
                persistent_output.fatal_silence(buf);
                return RefillResult {
                    drained: DrainedControlEvents::default(),
                    force_status: false,
                    fresh: false,
                    defer_status: false,
                };
            }
            SourceWorkerRenderDisposition::Fresh
            | SourceWorkerRenderDisposition::RecoveredReady => {}
            SourceWorkerRenderDisposition::NewlyMissed => unreachable!("recovery refresh miss"),
        }
        let recovered = recovery == SourceWorkerRenderDisposition::RecoveredReady;
        if recovered {
            persistent_output.deadline_recovery();
        }
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
        #[cfg(feature = "source-worker-benchmark-timing")]
        let engine_block_started_at = runtime.timing_block_start();
        let disposition = engine.render_interleaved_block_with_source_runtime_ready(
            runtime,
            *block_frames,
            left_buf,
            right_buf,
            buf,
        );
        #[cfg(feature = "source-worker-benchmark-timing")]
        runtime.record_engine_block_total(engine_block_started_at);
        match disposition {
            SourceWorkerRenderDisposition::Fresh => persistent_output.fresh(*block_frames, buf),
            SourceWorkerRenderDisposition::NewlyMissed => {
                persistent_output.deadline_miss(*sample_rate, *block_frames, buf)
            }
            SourceWorkerRenderDisposition::Recovering => persistent_output.recovery_silence(buf),
            SourceWorkerRenderDisposition::RecoveredReady => {
                unreachable!("recovery must precede rendering")
            }
            SourceWorkerRenderDisposition::Fatal => persistent_output.fatal_silence(buf),
        };
        RefillResult {
            drained,
            force_status: disposition == SourceWorkerRenderDisposition::NewlyMissed
                || (recovered && disposition == SourceWorkerRenderDisposition::Fresh),
            fresh: disposition == SourceWorkerRenderDisposition::Fresh,
            defer_status: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_output_repeats_once_and_geometry_mismatch_stays_invalid() {
        let mut cache = PreviousMasterQuantum::new();
        let fresh = [
            1.0_f32.to_bits(),
            (-0.0_f32).to_bits(),
            0x7fc0_1234,
            4.0_f32.to_bits(),
        ];
        let fresh: Vec<f32> = fresh.into_iter().map(f32::from_bits).collect();
        assert_eq!(cache.fresh(2, &fresh), PersistentOutputKind::Fresh);

        let mut repeated = vec![9.0; 4];
        assert_eq!(
            cache.deadline_miss(44_100, 2, &mut repeated),
            PersistentOutputKind::Repeated
        );
        assert_eq!(
            repeated
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            fresh
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>()
        );

        let mut pending = vec![9.0; 4];
        assert_eq!(
            cache.recovery_silence(&mut pending),
            PersistentOutputKind::Dropped
        );
        assert!(pending.iter().all(|sample| sample.to_bits() == 0));

        let mut mismatch = vec![9.0; 6];
        assert_eq!(
            cache.deadline_miss(44_100, 3, &mut mismatch),
            PersistentOutputKind::Dropped
        );
        cache.deadline_recovery();
        let mut no_stale_revival = vec![9.0; 4];
        assert_eq!(
            cache.deadline_miss(44_100, 2, &mut no_stale_revival),
            PersistentOutputKind::Dropped
        );
    }

    #[test]
    fn counters_snapshot_contains_only_previous_master_quantum_totals() {
        let mut cache = PreviousMasterQuantum::new();
        let fresh = [1.0_f32, 0.0, 0.0, 1.0];
        assert_eq!(cache.fresh(2, &fresh), PersistentOutputKind::Fresh);
        let mut repeated = vec![0.0; 4];
        assert_eq!(
            cache.deadline_miss(44_100, 2, &mut repeated),
            PersistentOutputKind::Repeated
        );
        let counters = cache.counters();
        assert_eq!(
            counters,
            PersistentOutputCounters {
                rendered_quantums: 1,
                repeated_quantums: 1,
                dropped_quantums: 0,
                deadline_misses: 1,
                deadline_recoveries: 0,
            }
        );
    }
}
