use super::super::super::types::BUS_COUNT;
use super::super::bus_chain_owner::BusChainCarrier;
use super::super::source_worker_bus;
use super::super::source_worker_carrier_transfer;
use super::super::source_worker_lifecycle::SourceWorkerScratch;
use super::super::source_worker_load::SourceWorkerLoadObservation;
use super::super::source_worker_protocol::WorkerPhase;
use super::super::source_worker_transfer;
use super::super::SynthEngine;
use super::SourceWorkerRuntime;
#[cfg(feature = "source-worker-benchmark-timing")]
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(super) fn finish_source_wave<R>(
        &mut self,
        engine: &mut SynthEngine,
        process: impl FnOnce(
            &mut SynthEngine,
            [&SourceWorkerScratch; 2],
            &mut [Option<BusChainCarrier>; BUS_COUNT],
        ) -> Result<R, ()>,
    ) -> Option<R> {
        if self.expected_phase != Some(WorkerPhase::Sources)
            || self.in_flight_mask != 0
            || self.completed_mask != 0b11
            || !self.home_is_ready()
        {
            self.fail_completion();
            return None;
        }
        let Some(mut first) = self.lease_home(0) else {
            self.fail_completion();
            return None;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            self.fail_completion();
            return None;
        };
        let load = self.load_snapshot();
        let result = source_worker_carrier_transfer::with_both_source_owners(
            engine,
            &mut first,
            &mut second,
            |engine, scratch, carriers| {
                source_worker_transfer::compact_source_pools(engine);
                #[cfg(feature = "source-worker-benchmark-timing")]
                let reduction_started_at = self.timing_probe.as_ref().map(|_| Instant::now());
                self.reduce_sources(
                    engine,
                    scratch,
                    self.expected_stamp.expect("completed source stamp").frames,
                );
                #[cfg(feature = "source-worker-benchmark-timing")]
                if let (Some(probe), Some(started_at), Some(stamp)) = (
                    self.timing_probe.as_ref(),
                    reduction_started_at,
                    self.expected_stamp,
                ) {
                    probe.record_reduction(stamp.quantum_sequence, started_at.elapsed());
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
                #[cfg(feature = "source-worker-benchmark-timing")]
                {
                    self.coordinator_remainder_started_at =
                        self.timing_probe.as_ref().map(|_| Instant::now());
                }
                engine.with_source_worker_load(load, |engine| process(engine, scratch, carriers))
            },
        );
        match result {
            Ok(Ok(result)) => {
                first.return_home();
                second.return_home();
                self.completed_mask = 0;
                Some(result)
            }
            Ok(Err(())) | Err(()) => {
                self.fail_completion();
                None
            }
        }
    }

    pub(super) fn finish_completed<R>(
        &mut self,
        engine: &mut SynthEngine,
        render: impl FnOnce(&mut SynthEngine) -> R,
    ) -> Option<R> {
        let result = self.finish_source_wave(engine, |engine, _, _| Ok(render(engine)))?;
        let rendered_frames = self.expected_stamp.expect("completed source stamp").frames;
        if let (Some(load), [Some(first), Some(second)]) =
            (self.load.as_mut(), self.source_load_observations)
        {
            if load.observe_pair([first, second]) {
                if let Some(utilization_ppm) = load.snapshot().utilization_ppm {
                    engine.observe_worker_utilization(utilization_ppm, rendered_frames);
                }
            }
        }
        self.source_load_observations = [None; 2];
        Some(result)
    }

    pub(super) fn finish_bus_wave(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        if self.expected_phase != Some(WorkerPhase::Buses)
            || self.in_flight_mask != 0
            || self.completed_mask != 0b11
            || !self.home_is_ready()
            || !self.bus_dispatch_residency_valid
        {
            self.fail_completion();
            return false;
        }
        let Some(mut first) = self.lease_home(0) else {
            self.fail_completion();
            return false;
        };
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            self.fail_completion();
            return false;
        };
        let expected_residency = self.bus_dispatch_residency;
        let result = source_worker_carrier_transfer::with_both_source_owners_preserving_carriers(
            engine,
            &mut first,
            &mut second,
            &expected_residency,
            |engine, _, carriers| {
                Ok(source_worker_bus::apply_bus_block_from_carriers(
                    engine, carriers, frames, left, right,
                ))
            },
        );
        match result {
            Ok(Ok(true)) => {
                if !self.observe_combined_load(engine, frames) {
                    self.fail_completion();
                    return false;
                }
                first.return_home();
                second.return_home();
                self.completed_mask = 0;
                self.bus_dispatch_residency_valid = false;
                true
            }
            Ok(Ok(false)) | Ok(Err(())) | Err(()) => {
                self.fail_completion();
                false
            }
        }
    }

    fn observe_combined_load(&mut self, engine: &mut SynthEngine, rendered_frames: usize) -> bool {
        let mut valid = true;
        if let (
            Some(load),
            [Some(source_first), Some(source_second)],
            [Some(bus_first), Some(bus_second)],
        ) = (
            self.load.as_mut(),
            self.source_load_observations,
            self.bus_load_observations,
        ) {
            let observations = [
                SourceWorkerLoadObservation {
                    dsp_duration_ns: source_first
                        .dsp_duration_ns
                        .saturating_add(bus_first.dsp_duration_ns),
                    active_cost_units: source_first
                        .active_cost_units
                        .saturating_add(bus_first.active_cost_units),
                },
                SourceWorkerLoadObservation {
                    dsp_duration_ns: source_second
                        .dsp_duration_ns
                        .saturating_add(bus_second.dsp_duration_ns),
                    active_cost_units: source_second
                        .active_cost_units
                        .saturating_add(bus_second.active_cost_units),
                },
            ];
            if load.observe_pair(observations) {
                if let Some(utilization_ppm) = load.snapshot().utilization_ppm {
                    engine.observe_worker_utilization(utilization_ppm, rendered_frames);
                }
            } else {
                valid = false;
            }
        }
        self.source_load_observations = [None; 2];
        self.bus_load_observations = [None; 2];
        valid
    }

    fn fail_completion(&mut self) {
        self.source_load_observations = [None; 2];
        self.bus_load_observations = [None; 2];
        self.bus_dispatch_residency_valid = false;
        self.latch_completion_failure(0b11);
        #[cfg(feature = "source-worker-benchmark-timing")]
        self.freeze_timing(true, None);
    }
}
