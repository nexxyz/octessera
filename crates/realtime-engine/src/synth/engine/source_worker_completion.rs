use super::super::source_worker_load::SourceWorkerLoadObservation;
use super::super::source_worker_transfer;
use super::super::SynthEngine;
use super::SourceWorkerRuntime;
#[cfg(feature = "source-worker-benchmark-timing")]
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(super) fn finish_completed(&mut self, engine: &mut SynthEngine) -> bool {
        if self.in_flight_mask != 0 || self.completed_mask != 0b11 || !self.home_is_ready() {
            self.load_observations = [None; 2];
            self.latch_completion_failure(0b11);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        let Some(mut first) = self.lease_home(0) else {
            self.latch_completion_failure(0b11);
            return false;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_completion_failure(0b11);
            return false;
        };
        match source_worker_transfer::with_both_source_partitions(
            engine,
            &mut first,
            &mut second,
            |engine, scratch| {
                source_worker_transfer::compact_source_pools(engine);
                #[cfg(feature = "source-worker-benchmark-timing")]
                let reduction_started_at = self.timing_probe.as_ref().map(|_| Instant::now());
                self.reduce_sources(engine, scratch, self.expected_frames);
                #[cfg(feature = "source-worker-benchmark-timing")]
                if let (Some(probe), Some(started_at), Some(sequence)) = (
                    self.timing_probe.as_ref(),
                    reduction_started_at,
                    self.expected_sequence,
                ) {
                    probe.record_reduction(sequence, started_at.elapsed());
                }
                for slot in 0..super::super::super::types::INSTRUMENT_SLOT_COUNT {
                    engine.active_synth_slots[slot] = engine
                        .synth_voice_pool
                        .active_count_for_slot(slot)
                        .unwrap_or(0)
                        > 0;
                    engine.active_sample_slots[slot] = engine
                        .sample_voice_pool
                        .active_count_for_slot(slot)
                        .unwrap_or(0)
                        > 0;
                }
            },
        ) {
            Ok(()) => {
                if let (Some(load), [Some(first), Some(second)]) =
                    (self.load.as_mut(), self.load_observations)
                {
                    let _ = load.observe_pair([
                        SourceWorkerLoadObservation {
                            dsp_duration_ns: first.dsp_duration_ns,
                            active_cost_units: first.active_cost_units,
                        },
                        SourceWorkerLoadObservation {
                            dsp_duration_ns: second.dsp_duration_ns,
                            active_cost_units: second.active_cost_units,
                        },
                    ]);
                }
                self.load_observations = [None; 2];
                first.return_home();
                second.return_home();
                self.completed_mask = 0;
                true
            }
            Err(()) => {
                self.load_observations = [None; 2];
                self.latch_completion_failure(0b11);
                #[cfg(feature = "source-worker-benchmark-timing")]
                self.freeze_timing(true, None);
                false
            }
        }
    }
}
