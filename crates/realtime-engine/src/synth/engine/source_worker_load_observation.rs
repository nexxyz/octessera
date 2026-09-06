pub(in crate::synth::engine) const EWMA_SCALE: u64 = 1_000_000;
pub(in crate::synth::engine) const EWMA_WINDOW_NS: u64 = 250_000_000;

#[derive(Clone, Copy)]
pub(in crate::synth::engine) struct SourceWorkerLoadObservation {
    pub(in crate::synth::engine) dsp_duration_ns: u64,
    pub(in crate::synth::engine) active_cost_units: u16,
}

pub(in crate::synth::engine) struct SourceWorkerLoad {
    quantum_ns: u64,
    pub(in crate::synth::engine) ewma_coefficient_ppm: u32,
    busy_ns_ewma: [u64; super::SOURCE_WORKER_COUNT],
    ns_per_unit_ewma: [u64; super::SOURCE_WORKER_COUNT],
    observed_active_cost_units: [u16; super::SOURCE_WORKER_COUNT],
    has_useful_measurement: [bool; super::SOURCE_WORKER_COUNT],
    observed: [bool; super::SOURCE_WORKER_COUNT],
}

impl SourceWorkerLoad {
    pub(in crate::synth::engine) fn new(frames: usize, sample_rate: u32) -> Self {
        let quantum_ns = render_quantum_ns(frames, sample_rate);
        let seed_ns_per_unit = quantum_ns / u64::from(super::SOURCE_WORKER_MAX_COST_UNITS);
        Self {
            quantum_ns,
            ewma_coefficient_ppm: ewma_coefficient_ppm(quantum_ns),
            busy_ns_ewma: [0; super::SOURCE_WORKER_COUNT],
            ns_per_unit_ewma: [seed_ns_per_unit; super::SOURCE_WORKER_COUNT],
            observed_active_cost_units: [0; super::SOURCE_WORKER_COUNT],
            has_useful_measurement: [false; super::SOURCE_WORKER_COUNT],
            observed: [false; super::SOURCE_WORKER_COUNT],
        }
    }

    pub(in crate::synth::engine) fn observe_pair(
        &mut self,
        observations: [SourceWorkerLoadObservation; super::SOURCE_WORKER_COUNT],
    ) -> bool {
        self.observe_pair_with_max_cost(observations, super::SOURCE_WORKER_MAX_COST_UNITS)
    }

    pub(in crate::synth::engine) fn observe_pair_with_max_cost(
        &mut self,
        observations: [SourceWorkerLoadObservation; super::SOURCE_WORKER_COUNT],
        max_cost_units: u16,
    ) -> bool {
        if observations
            .iter()
            .any(|observation| observation.active_cost_units > max_cost_units)
        {
            return false;
        }
        for (worker, observation) in observations.into_iter().enumerate() {
            self.busy_ns_ewma[worker] = ewma(
                self.busy_ns_ewma[worker],
                observation.dsp_duration_ns,
                self.ewma_coefficient_ppm,
            );
            if observation.active_cost_units != 0 {
                let sample_ns_per_unit = observation
                    .dsp_duration_ns
                    .saturating_div(u64::from(observation.active_cost_units));
                self.ns_per_unit_ewma[worker] = ewma(
                    self.ns_per_unit_ewma[worker],
                    sample_ns_per_unit,
                    self.ewma_coefficient_ppm,
                );
                self.has_useful_measurement[worker] = true;
            }
            self.observed_active_cost_units[worker] = observation.active_cost_units;
            self.observed[worker] = true;
        }
        true
    }

    pub(in crate::synth::engine) fn snapshot(&self) -> super::SourceWorkerLoadSnapshot {
        let utilization_ppm =
            if self.quantum_ns == 0 || !self.observed.iter().all(|observed| *observed) {
                None
            } else {
                let busy_ns = self.busy_ns_ewma.into_iter().max().unwrap_or(0);
                Some(
                    ((u128::from(busy_ns) * u128::from(EWMA_SCALE)) / u128::from(self.quantum_ns))
                        .min(u128::from(u32::MAX)) as u32,
                )
            };
        super::SourceWorkerLoadSnapshot {
            quantum_ns: self.quantum_ns,
            ewma_coefficient_ppm: self.ewma_coefficient_ppm,
            busy_ns_ewma: self.busy_ns_ewma,
            ns_per_unit_ewma: self.ns_per_unit_ewma,
            observed_active_cost_units: self.observed_active_cost_units,
            has_useful_measurement: self.has_useful_measurement,
            utilization_ppm,
            observed: self.observed,
        }
    }
}

pub(in crate::synth::engine) fn render_quantum_ns(frames: usize, sample_rate: u32) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    ((frames as u128 * 1_000_000_000_u128) / u128::from(sample_rate)).min(u128::from(u64::MAX))
        as u64
}

pub(in crate::synth::engine) fn ewma_coefficient_ppm(quantum_ns: u64) -> u32 {
    ((quantum_ns.min(EWMA_WINDOW_NS) as u128 * u128::from(EWMA_SCALE)) / u128::from(EWMA_WINDOW_NS))
        as u32
}

fn ewma(previous: u64, sample: u64, coefficient_ppm: u32) -> u64 {
    if sample >= previous {
        previous.saturating_add(
            (((sample - previous) as u128 * u128::from(coefficient_ppm)) / u128::from(EWMA_SCALE))
                as u64,
        )
    } else {
        previous.saturating_sub(
            (((previous - sample) as u128 * u128::from(coefficient_ppm)) / u128::from(EWMA_SCALE))
                as u64,
        )
    }
}
