pub const SOURCE_WORKER_SYNTH_COST_UNITS: u16 = 3;
pub const SOURCE_WORKER_SAMPLE_COST_UNITS: u16 = 2;
pub const SOURCE_WORKER_MAX_COST_UNITS: u16 = 160;

const SOURCE_WORKER_COUNT: usize = 2;
const EWMA_SCALE: u64 = 1_000_000;
const EWMA_WINDOW_NS: u64 = 250_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWorkerLoadSnapshot {
    pub quantum_ns: u64,
    pub ewma_coefficient_ppm: u32,
    pub busy_ns_ewma: [u64; SOURCE_WORKER_COUNT],
    pub ns_per_unit_ewma: [u64; SOURCE_WORKER_COUNT],
    pub observed_active_cost_units: [u16; SOURCE_WORKER_COUNT],
    pub has_useful_measurement: [bool; SOURCE_WORKER_COUNT],
    pub utilization_ppm: Option<u32>,
    pub observed: [bool; SOURCE_WORKER_COUNT],
}

impl SourceWorkerLoadSnapshot {
    pub(super) fn choose_worker(
        &self,
        current_active_cost_units: [u16; SOURCE_WORKER_COUNT],
        new_cost_units: u16,
        prospective_victim: Option<(usize, u16)>,
        inactive_lanes: [bool; SOURCE_WORKER_COUNT],
    ) -> Option<usize> {
        let mut active_cost_units = current_active_cost_units;
        if let Some((victim_worker, victim_cost_units)) = prospective_victim {
            if victim_worker >= SOURCE_WORKER_COUNT {
                return None;
            }
            active_cost_units[victim_worker] =
                active_cost_units[victim_worker].saturating_sub(victim_cost_units);
        }
        let victim_worker = prospective_victim.map(|(worker, _)| worker);
        let mut selected_worker = None;
        let mut selected_projection = u64::MAX;
        for worker in 0..SOURCE_WORKER_COUNT {
            if !inactive_lanes[worker] && victim_worker != Some(worker) {
                continue;
            }
            let mut projected_active_cost_units = active_cost_units;
            projected_active_cost_units[worker] =
                projected_active_cost_units[worker].saturating_add(new_cost_units);
            let projected_ns = self.projected_ns(projected_active_cost_units)[worker];
            if selected_worker.is_none() || projected_ns < selected_projection {
                selected_worker = Some(worker);
                selected_projection = projected_ns;
            }
        }
        selected_worker
    }

    pub fn projected_ns(
        &self,
        active_cost_units: [u16; SOURCE_WORKER_COUNT],
    ) -> [u64; SOURCE_WORKER_COUNT] {
        std::array::from_fn(|worker| self.projected_worker_ns(worker, active_cost_units[worker]))
    }

    fn projected_worker_ns(&self, worker: usize, active_cost_units: u16) -> u64 {
        let baseline_units = self.observed_active_cost_units[worker];
        let ns_per_unit = if self.has_useful_measurement[worker] {
            self.ns_per_unit_ewma[worker]
        } else {
            self.quantum_ns / u64::from(SOURCE_WORKER_MAX_COST_UNITS)
        };
        let busy_ns = self.busy_ns_ewma[worker];
        if active_cost_units >= baseline_units {
            busy_ns.saturating_add(
                ns_per_unit.saturating_mul(u64::from(active_cost_units - baseline_units)),
            )
        } else {
            busy_ns.saturating_sub(
                ns_per_unit.saturating_mul(u64::from(baseline_units - active_cost_units)),
            )
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SourceWorkerLoadObservation {
    pub(super) dsp_duration_ns: u64,
    pub(super) active_cost_units: u16,
}

pub(super) struct SourceWorkerLoad {
    quantum_ns: u64,
    ewma_coefficient_ppm: u32,
    busy_ns_ewma: [u64; SOURCE_WORKER_COUNT],
    ns_per_unit_ewma: [u64; SOURCE_WORKER_COUNT],
    observed_active_cost_units: [u16; SOURCE_WORKER_COUNT],
    has_useful_measurement: [bool; SOURCE_WORKER_COUNT],
    observed: [bool; SOURCE_WORKER_COUNT],
}

impl SourceWorkerLoad {
    pub(super) fn new(frames: usize, sample_rate: u32) -> Self {
        let quantum_ns = render_quantum_ns(frames, sample_rate);
        let seed_ns_per_unit = quantum_ns / u64::from(SOURCE_WORKER_MAX_COST_UNITS);
        Self {
            quantum_ns,
            ewma_coefficient_ppm: ewma_coefficient_ppm(quantum_ns),
            busy_ns_ewma: [0; SOURCE_WORKER_COUNT],
            ns_per_unit_ewma: [seed_ns_per_unit; SOURCE_WORKER_COUNT],
            observed_active_cost_units: [0; SOURCE_WORKER_COUNT],
            has_useful_measurement: [false; SOURCE_WORKER_COUNT],
            observed: [false; SOURCE_WORKER_COUNT],
        }
    }

    pub(super) fn observe_pair(
        &mut self,
        observations: [SourceWorkerLoadObservation; SOURCE_WORKER_COUNT],
    ) -> bool {
        if observations
            .iter()
            .any(|observation| observation.active_cost_units > SOURCE_WORKER_MAX_COST_UNITS)
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

    pub(super) fn snapshot(&self) -> SourceWorkerLoadSnapshot {
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
        SourceWorkerLoadSnapshot {
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

pub(super) fn render_quantum_ns(frames: usize, sample_rate: u32) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    ((frames as u128 * 1_000_000_000_u128) / u128::from(sample_rate)).min(u128::from(u64::MAX))
        as u64
}

fn ewma_coefficient_ppm(quantum_ns: u64) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_costs_fill_one_worker_partition() {
        assert_eq!(SOURCE_WORKER_SYNTH_COST_UNITS, 3);
        assert_eq!(SOURCE_WORKER_SAMPLE_COST_UNITS, 2);
        assert_eq!(SOURCE_WORKER_MAX_COST_UNITS, 160);
    }

    #[test]
    fn ewma_coefficient_uses_render_quantum_over_250ms() {
        for frames in [64, 128, 256, 2048] {
            for sample_rate in [44_100, 48_000] {
                let quantum_ns = render_quantum_ns(frames, sample_rate);
                let expected = ((quantum_ns as u128 * u128::from(EWMA_SCALE))
                    / u128::from(EWMA_WINDOW_NS)) as u32;
                assert_eq!(ewma_coefficient_ppm(quantum_ns), expected);
            }
        }
        assert_eq!(ewma_coefficient_ppm(EWMA_WINDOW_NS), EWMA_SCALE as u32);
        assert_eq!(ewma_coefficient_ppm(EWMA_WINDOW_NS + 1), EWMA_SCALE as u32);
    }

    #[test]
    fn unseen_calibration_is_seeded_and_empty_observations_retain_it() {
        let mut load = SourceWorkerLoad::new(128, 48_000);
        let seed = render_quantum_ns(128, 48_000) / u64::from(SOURCE_WORKER_MAX_COST_UNITS);
        assert_eq!(load.snapshot().ns_per_unit_ewma, [seed; 2]);
        assert!(load.observe_pair([
            SourceWorkerLoadObservation {
                dsp_duration_ns: 20_000,
                active_cost_units: 0,
            },
            SourceWorkerLoadObservation {
                dsp_duration_ns: 30_000,
                active_cost_units: 0,
            },
        ]));
        assert_eq!(load.snapshot().ns_per_unit_ewma, [seed; 2]);
    }

    #[test]
    fn paired_observations_are_applied_in_parity_order() {
        let mut first = SourceWorkerLoad::new(12000, 48_000);
        let mut second = SourceWorkerLoad::new(12000, 48_000);
        let pair = [
            SourceWorkerLoadObservation {
                dsp_duration_ns: 100_000,
                active_cost_units: 10,
            },
            SourceWorkerLoadObservation {
                dsp_duration_ns: 200_000,
                active_cost_units: 20,
            },
        ];
        assert!(first.observe_pair(pair));
        assert!(second.observe_pair([pair[0], pair[1]]));
        assert_eq!(first.snapshot(), second.snapshot());
    }

    #[test]
    fn coefficient_edges_are_exact_for_supported_quanta_and_rates() {
        let expected = [
            ((32, 44_100), 2_902),
            ((64, 44_100), 5_804),
            ((128, 44_100), 11_609),
            ((256, 44_100), 23_219),
            ((2048, 44_100), 185_759),
            ((32, 48_000), 2_666),
            ((64, 48_000), 5_333),
            ((128, 48_000), 10_666),
            ((256, 48_000), 21_333),
            ((2048, 48_000), 170_666),
        ];
        for ((frames, sample_rate), coefficient) in expected {
            assert_eq!(
                SourceWorkerLoad::new(frames, sample_rate).ewma_coefficient_ppm,
                coefficient
            );
        }
    }

    #[test]
    fn seed_and_projection_use_nanoseconds_only() {
        let load = SourceWorkerLoad::new(128, 48_000);
        let snapshot = load.snapshot();
        assert_eq!(snapshot.quantum_ns, 2_666_666);
        assert_eq!(snapshot.ns_per_unit_ewma, [16_666; 2]);
        assert_eq!(snapshot.projected_ns([3, 160]), [49_998, 2_666_560]);
    }

    #[test]
    fn empty_baseline_keeps_measured_scratch_time_and_uses_seed_for_units() {
        let snapshot = SourceWorkerLoadSnapshot {
            quantum_ns: 1_000_000,
            ewma_coefficient_ppm: 1_000_000,
            busy_ns_ewma: [1_000, 2_000],
            ns_per_unit_ewma: [9, 9],
            observed_active_cost_units: [0, 0],
            has_useful_measurement: [false, false],
            utilization_ppm: None,
            observed: [true, true],
        };
        assert_eq!(snapshot.projected_ns([3, 4]), [19_750, 27_000]);
    }

    #[test]
    fn worker_choice_ties_at_zero_and_uses_measured_ns_per_unit() {
        let snapshot = SourceWorkerLoadSnapshot {
            quantum_ns: 1_000_000,
            ewma_coefficient_ppm: 1_000_000,
            busy_ns_ewma: [0, 0],
            ns_per_unit_ewma: [100, 10],
            observed_active_cost_units: [0, 0],
            has_useful_measurement: [true, true],
            utilization_ppm: None,
            observed: [true, true],
        };
        assert_eq!(
            snapshot.choose_worker([0, 0], 3, None, [true, true]),
            Some(1)
        );
        assert_eq!(
            SourceWorkerLoadSnapshot {
                ns_per_unit_ewma: [10, 10],
                ..snapshot
            }
            .choose_worker([0, 0], 3, None, [true, true]),
            Some(0)
        );
        assert_eq!(
            SourceWorkerLoadSnapshot {
                ns_per_unit_ewma: [u64::MAX, 10],
                ..snapshot
            }
            .choose_worker([0, 0], 3, None, [true, false]),
            Some(0)
        );
    }

    #[test]
    fn worker_choice_subtracts_victim_before_adding_new_cost() {
        let snapshot = SourceWorkerLoadSnapshot {
            quantum_ns: 1_000_000,
            ewma_coefficient_ppm: 1_000_000,
            busy_ns_ewma: [0, 0],
            ns_per_unit_ewma: [10, 10],
            observed_active_cost_units: [0, 0],
            has_useful_measurement: [true, true],
            utilization_ppm: None,
            observed: [true, true],
        };
        assert_eq!(
            snapshot.choose_worker([3, 0], 3, Some((0, 3)), [false, true]),
            Some(0)
        );
    }

    #[test]
    fn utilization_uses_the_full_render_quantum_and_max_worker() {
        let mut load = SourceWorkerLoad::new(12000, 48_000);
        assert!(load.observe_pair([
            SourceWorkerLoadObservation {
                dsp_duration_ns: 250_000_000,
                active_cost_units: 1,
            },
            SourceWorkerLoadObservation {
                dsp_duration_ns: 500_000_000,
                active_cost_units: 1,
            },
        ]));
        assert_eq!(load.snapshot().utilization_ppm, Some(2_000_000));
    }

    #[test]
    fn fixed_scratch_overhead_does_not_make_a_full_worker_appear_cheaper() {
        let snapshot = SourceWorkerLoadSnapshot {
            quantum_ns: 1_000_000,
            ewma_coefficient_ppm: 1_000_000,
            busy_ns_ewma: [1_250, 1_000],
            ns_per_unit_ewma: [1, 200],
            observed_active_cost_units: [100, 1],
            has_useful_measurement: [true, true],
            utilization_ppm: None,
            observed: [true, true],
        };
        assert_eq!(snapshot.projected_ns([100, 1]), [1_250, 1_000]);
        assert_eq!(snapshot.projected_ns([101, 2]), [1_251, 1_200]);
        assert_eq!(
            snapshot.choose_worker([100, 1], 1, None, [true, true]),
            Some(1)
        );
    }

    #[test]
    fn projection_saturates_bounded_addition_and_subtraction() {
        let snapshot = SourceWorkerLoadSnapshot {
            quantum_ns: 1_000_000,
            ewma_coefficient_ppm: 1_000_000,
            busy_ns_ewma: [u64::MAX, 0],
            ns_per_unit_ewma: [u64::MAX, u64::MAX],
            observed_active_cost_units: [0, u16::MAX],
            has_useful_measurement: [true, true],
            utilization_ppm: None,
            observed: [true, true],
        };
        assert_eq!(snapshot.projected_ns([u16::MAX, 0]), [u64::MAX, 0]);
    }

    #[test]
    fn projection_anchors_at_measured_busy_and_applies_only_unit_delta() {
        let mut load = SourceWorkerLoad::new(12_000, 48_000);
        assert!(load.observe_pair([
            SourceWorkerLoadObservation {
                dsp_duration_ns: 100_000,
                active_cost_units: 10,
            },
            SourceWorkerLoadObservation {
                dsp_duration_ns: 200_000,
                active_cost_units: 20,
            },
        ]));
        let snapshot = load.snapshot();
        assert_eq!(snapshot.projected_ns([10, 20]), [100_000, 200_000]);
        assert_eq!(snapshot.projected_ns([13, 17]), [130_000, 170_000]);
        assert_eq!(snapshot.projected_ns([7, 23]), [70_000, 230_000]);
    }

    #[test]
    fn invalid_paired_observations_are_ignored_atomically() {
        let mut load = SourceWorkerLoad::new(128, 48_000);
        let before = load.snapshot();
        assert!(!load.observe_pair([
            SourceWorkerLoadObservation {
                dsp_duration_ns: 100,
                active_cost_units: SOURCE_WORKER_MAX_COST_UNITS + 1,
            },
            SourceWorkerLoadObservation {
                dsp_duration_ns: 200,
                active_cost_units: 2,
            },
        ]));
        assert_eq!(load.snapshot(), before);
    }

    #[test]
    fn load_math_does_not_allocate() {
        let mut load = SourceWorkerLoad::new(128, 48_000);
        let (_, allocations, deallocations) =
            crate::synth::test_allocator::count_allocations_and_deallocations(|| {
                assert!(load.observe_pair([
                    SourceWorkerLoadObservation {
                        dsp_duration_ns: 100,
                        active_cost_units: 3,
                    },
                    SourceWorkerLoadObservation {
                        dsp_duration_ns: 200,
                        active_cost_units: 2,
                    },
                ]));
                let _ = load.snapshot();
            });
        assert_eq!((allocations, deallocations), (0, 0));
    }
}
