use super::schema::{BenchmarkProfileSnapshot, BenchmarkWorkerDelta};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerPolicy {
    pub expected_effective: bool,
    pub require_dispatch: bool,
}

pub fn policy(internal_frames: usize, workers: usize, scenario: &str) -> WorkerPolicy {
    WorkerPolicy {
        expected_effective: internal_frames >= 256 && workers > 0,
        require_dispatch: internal_frames == 256
            && workers > 0
            && matches!(
                scenario,
                "synth_cross_slot_96_steal"
                    | "mixed_cross_slot_48_48_steal"
                    | "synth_cross_slot_32_no_steal"
                    | "mixed_16_synth_32_sample"
            ),
    }
}

pub fn delta(
    start: &BenchmarkProfileSnapshot,
    end: &BenchmarkProfileSnapshot,
) -> Result<BenchmarkWorkerDelta, String> {
    Ok(BenchmarkWorkerDelta {
        synth_parallel_dispatches: end
            .synth_parallel_dispatches
            .checked_sub(start.synth_parallel_dispatches)
            .ok_or_else(|| "worker dispatch counter regressed".to_string())?,
        synth_parallel_light_skips: end
            .synth_parallel_light_skips
            .checked_sub(start.synth_parallel_light_skips)
            .ok_or_else(|| "worker light-skip counter regressed".to_string())?,
        synth_parallel_backoff_skips: end
            .synth_parallel_backoff_skips
            .checked_sub(start.synth_parallel_backoff_skips)
            .ok_or_else(|| "worker backoff counter regressed".to_string())?,
        synth_parallel_timing_backoffs: end
            .synth_parallel_timing_backoffs
            .checked_sub(start.synth_parallel_timing_backoffs)
            .ok_or_else(|| "worker timing-backoff counter regressed".to_string())?,
        synth_parallel_failures: end
            .synth_parallel_failures
            .checked_sub(start.synth_parallel_failures)
            .ok_or_else(|| "worker failure counter regressed".to_string())?,
        synth_parallel_unhealthy: start.synth_parallel_unhealthy || end.synth_parallel_unhealthy,
    })
}

pub fn validate_configuration(
    policy: WorkerPolicy,
    effective: bool,
    start: &BenchmarkProfileSnapshot,
    end: &BenchmarkProfileSnapshot,
) -> Result<(), String> {
    if effective != policy.expected_effective {
        return Err(format!(
            "worker effectiveness mismatch: actual={effective} expected={}",
            policy.expected_effective
        ));
    }
    if !policy.expected_effective && !worker_counters_are_zero(start, end) {
        return Err("ineffective workers reported nonzero worker telemetry".into());
    }
    Ok(())
}

pub fn validate_policy(
    policy: WorkerPolicy,
    delta: &BenchmarkWorkerDelta,
    scenario: &str,
) -> Result<(), String> {
    if delta.synth_parallel_light_skips > 0
        || delta.synth_parallel_backoff_skips > 0
        || delta.synth_parallel_timing_backoffs > 0
        || delta.synth_parallel_failures > 0
        || delta.synth_parallel_unhealthy
    {
        return Err("worker telemetry reported a skip, failure, or unhealthy state".into());
    }
    if policy.require_dispatch && delta.synth_parallel_dispatches == 0 {
        return Err(format!("worker dispatches were absent for {scenario}"));
    }
    Ok(())
}

fn worker_counters_are_zero(
    start: &BenchmarkProfileSnapshot,
    end: &BenchmarkProfileSnapshot,
) -> bool {
    [start, end].iter().all(|snapshot| {
        snapshot.synth_parallel_dispatches == 0
            && snapshot.synth_parallel_light_skips == 0
            && snapshot.synth_parallel_backoff_skips == 0
            && snapshot.synth_parallel_timing_backoffs == 0
            && snapshot.synth_parallel_failures == 0
            && !snapshot.synth_parallel_unhealthy
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(dispatches: u64) -> BenchmarkProfileSnapshot {
        BenchmarkProfileSnapshot {
            synth_parallel_dispatches: dispatches,
            ..BenchmarkProfileSnapshot::default()
        }
    }

    fn validates(
        policy: WorkerPolicy,
        effective: bool,
        start: &BenchmarkProfileSnapshot,
        end: &BenchmarkProfileSnapshot,
        scenario: &str,
    ) -> Result<BenchmarkWorkerDelta, String> {
        let measured = delta(start, end)?;
        validate_configuration(policy, effective, start, end)?;
        validate_policy(policy, &measured, scenario)?;
        Ok(measured)
    }

    #[test]
    fn low_internal_blocks_and_zero_workers_are_ineffective() {
        for (internal, workers) in [(64, 2), (128, 2), (256, 0)] {
            let policy = policy(internal, workers, "synth_cross_slot_96_steal");
            assert!(!policy.expected_effective);
            assert!(validates(policy, false, &snapshot(0), &snapshot(0), "scenario").is_ok());
        }
    }

    #[test]
    fn effective_synth_and_mixed_profiles_require_clean_dispatching() {
        for scenario in [
            "synth_cross_slot_96_steal",
            "mixed_cross_slot_48_48_steal",
            "synth_cross_slot_32_no_steal",
            "mixed_16_synth_32_sample",
        ] {
            let policy = policy(256, 2, scenario);
            assert!(validates(policy, true, &snapshot(0), &snapshot(1), scenario).is_ok());
            assert!(validates(policy, true, &snapshot(0), &snapshot(0), scenario).is_err());
        }
    }

    #[test]
    fn baseline_worker_comparison_policy_covers_only_the_approved_scenarios() {
        for scenario in ["synth_cross_slot_32_no_steal", "mixed_16_synth_32_sample"] {
            for workers in [0, 2, 3] {
                let policy = policy(256, workers, scenario);
                assert_eq!(
                    policy.expected_effective,
                    workers > 0,
                    "{scenario}/{workers}"
                );
                assert_eq!(policy.require_dispatch, workers > 0, "{scenario}/{workers}");
            }
        }
        for scenario in [
            "synth_cross_slot_16",
            "sample_cross_slot_64",
            "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
            "synth_cross_slot_64_no_steal",
        ] {
            assert!(!policy(256, 2, scenario).require_dispatch, "{scenario}");
        }
    }

    #[test]
    fn worker_skip_or_failure_rejects_policy() {
        let mut end = snapshot(1);
        end.synth_parallel_backoff_skips = 1;
        let policy = policy(256, 3, "mixed_cross_slot_48_48_steal");
        assert!(validates(
            policy,
            true,
            &snapshot(0),
            &end,
            "mixed_cross_slot_48_48_steal"
        )
        .is_err());
    }

    #[test]
    fn c2_policy_failure_retains_the_complete_measured_delta() {
        let policy = policy(256, 2, "synth_cross_slot_96_steal");
        let start = snapshot(0);
        let end = BenchmarkProfileSnapshot {
            synth_parallel_dispatches: 4_639,
            synth_parallel_backoff_skips: 1_395,
            synth_parallel_timing_backoffs: 22,
            ..BenchmarkProfileSnapshot::default()
        };
        let measured = delta(&start, &end).unwrap();
        assert_eq!(
            measured,
            BenchmarkWorkerDelta {
                synth_parallel_dispatches: 4_639,
                synth_parallel_light_skips: 0,
                synth_parallel_backoff_skips: 1_395,
                synth_parallel_timing_backoffs: 22,
                synth_parallel_failures: 0,
                synth_parallel_unhealthy: false,
            }
        );
        assert!(validate_policy(policy, &measured, "synth_cross_slot_96_steal").is_err());
    }

    #[test]
    fn clean_dispatch_has_no_policy_error() {
        let policy = policy(256, 2, "synth_cross_slot_96_steal");
        let measured = delta(&snapshot(0), &snapshot(1)).unwrap();
        assert!(validate_policy(policy, &measured, "synth_cross_slot_96_steal").is_ok());
    }

    #[test]
    fn every_counter_regression_is_rejected_as_invalid_evidence() {
        for counter in 0..5 {
            let mut start = snapshot(0);
            let end = snapshot(0);
            match counter {
                0 => start.synth_parallel_dispatches = 1,
                1 => start.synth_parallel_light_skips = 1,
                2 => start.synth_parallel_backoff_skips = 1,
                3 => start.synth_parallel_timing_backoffs = 1,
                4 => start.synth_parallel_failures = 1,
                _ => unreachable!(),
            }
            assert!(delta(&start, &end).is_err());
            assert!(validates(
                policy(256, 2, "synth_cross_slot_96_steal"),
                true,
                &start,
                &end,
                "synth_cross_slot_96_steal"
            )
            .is_err());
        }
    }
}
