use super::cli::BenchmarkExecutorMode;
use rodio_engine_source::PersistentOutputCounters;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{fence, AtomicU64, Ordering};

pub(crate) struct PersistentOutputCountersMirror {
    sequence: AtomicU64,
    rendered_quantums: AtomicU64,
    repeated_quantums: AtomicU64,
    dropped_quantums: AtomicU64,
    deadline_misses: AtomicU64,
    deadline_recoveries: AtomicU64,
}

impl PersistentOutputCountersMirror {
    pub(crate) fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            rendered_quantums: AtomicU64::new(0),
            repeated_quantums: AtomicU64::new(0),
            dropped_quantums: AtomicU64::new(0),
            deadline_misses: AtomicU64::new(0),
            deadline_recoveries: AtomicU64::new(0),
        }
    }

    pub(crate) fn publish(&self, counters: PersistentOutputCounters) {
        let sequence = self.sequence.fetch_add(1, Ordering::AcqRel);
        debug_assert!(sequence.is_multiple_of(2));
        self.rendered_quantums
            .store(counters.rendered_quantums, Ordering::Relaxed);
        self.repeated_quantums
            .store(counters.repeated_quantums, Ordering::Relaxed);
        self.dropped_quantums
            .store(counters.dropped_quantums, Ordering::Relaxed);
        self.deadline_misses
            .store(counters.deadline_misses, Ordering::Relaxed);
        self.deadline_recoveries
            .store(counters.deadline_recoveries, Ordering::Relaxed);
        self.sequence
            .store(sequence.wrapping_add(2), Ordering::Release);
    }

    pub(crate) fn snapshot(&self) -> PersistentOutputCounters {
        loop {
            let sequence = self.sequence.load(Ordering::Acquire);
            if !sequence.is_multiple_of(2) {
                std::hint::spin_loop();
                continue;
            }
            let counters = PersistentOutputCounters {
                rendered_quantums: self.rendered_quantums.load(Ordering::Relaxed),
                repeated_quantums: self.repeated_quantums.load(Ordering::Relaxed),
                dropped_quantums: self.dropped_quantums.load(Ordering::Relaxed),
                deadline_misses: self.deadline_misses.load(Ordering::Relaxed),
                deadline_recoveries: self.deadline_recoveries.load(Ordering::Relaxed),
            };
            fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Acquire) == sequence {
                return counters;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PersistentOutputCountersEvidence {
    pub observable: bool,
    pub warmup: PersistentOutputCounters,
    pub start: PersistentOutputCounters,
    pub end: PersistentOutputCounters,
    pub delta: PersistentOutputCounters,
}

impl PersistentOutputCountersEvidence {
    pub fn for_executor(executor_mode: BenchmarkExecutorMode) -> Self {
        Self {
            observable: matches!(
                executor_mode,
                BenchmarkExecutorMode::PersistentTwoWorkers
                    | BenchmarkExecutorMode::RoutingTreePersistent
            ),
            ..Self::default()
        }
    }

    pub fn set_end(&mut self, counters: PersistentOutputCounters) {
        self.end = counters;
    }

    pub fn calculate_delta(&mut self) -> Result<(), String> {
        self.delta = subtract_counters(self.end, self.start)?;
        Ok(())
    }

    pub fn validate(&self, executor_mode: BenchmarkExecutorMode) -> Result<(), String> {
        let expected_observable = matches!(
            executor_mode,
            BenchmarkExecutorMode::PersistentTwoWorkers
                | BenchmarkExecutorMode::RoutingTreePersistent
        );
        if self.observable != expected_observable {
            return Err("persistent output counter observability does not match executor".into());
        }
        validate_cumulative_snapshot("warmup", self.warmup)?;
        validate_cumulative_snapshot("start", self.start)?;
        validate_cumulative_snapshot("end", self.end)?;
        if !counters_le(self.warmup, self.start) || !counters_le(self.start, self.end) {
            return Err("persistent output counter snapshots are not monotonic".into());
        }
        if subtract_counters(self.end, self.start)? != self.delta {
            return Err("persistent output counter delta is not end minus start".into());
        }
        validate_window_delta("delta", self.start, self.delta)?;
        if !self.observable
            && [self.warmup, self.start, self.end, self.delta]
                .into_iter()
                .any(|counters| counters != PersistentOutputCounters::default())
        {
            return Err("inline executor must report zero persistent output counters".into());
        }
        Ok(())
    }

    pub fn detected_continuity_events(&self, callback_overruns: u64) -> u64 {
        let carry_in = self.start.deadline_misses > self.start.deadline_recoveries
            && (self.delta.repeated_quantums > 0 || self.delta.dropped_quantums > 0);
        callback_overruns.max(
            self.delta
                .deadline_misses
                .saturating_add(u64::from(carry_in)),
        )
    }
}

fn subtract_counters(
    end: PersistentOutputCounters,
    start: PersistentOutputCounters,
) -> Result<PersistentOutputCounters, String> {
    Ok(PersistentOutputCounters {
        rendered_quantums: end
            .rendered_quantums
            .checked_sub(start.rendered_quantums)
            .ok_or_else(|| "rendered quantum counters are not monotonic".to_string())?,
        repeated_quantums: end
            .repeated_quantums
            .checked_sub(start.repeated_quantums)
            .ok_or_else(|| "repeated quantum counters are not monotonic".to_string())?,
        dropped_quantums: end
            .dropped_quantums
            .checked_sub(start.dropped_quantums)
            .ok_or_else(|| "dropped quantum counters are not monotonic".to_string())?,
        deadline_misses: end
            .deadline_misses
            .checked_sub(start.deadline_misses)
            .ok_or_else(|| "deadline miss counters are not monotonic".to_string())?,
        deadline_recoveries: end
            .deadline_recoveries
            .checked_sub(start.deadline_recoveries)
            .ok_or_else(|| "deadline recovery counters are not monotonic".to_string())?,
    })
}

fn validate_cumulative_snapshot(
    name: &str,
    counters: PersistentOutputCounters,
) -> Result<(), String> {
    if counters.deadline_recoveries > counters.deadline_misses {
        return Err(format!(
            "persistent output {name} recoveries exceed deadline misses"
        ));
    }
    if counters.repeated_quantums > counters.deadline_misses
        || counters.deadline_misses
            > counters
                .repeated_quantums
                .checked_add(counters.dropped_quantums)
                .ok_or_else(|| format!("persistent output {name} dispositions overflow"))?
    {
        return Err(format!(
            "persistent output {name} dispositions are impossible"
        ));
    }
    Ok(())
}

fn validate_window_delta(
    name: &str,
    start: PersistentOutputCounters,
    delta: PersistentOutputCounters,
) -> Result<(), String> {
    let carry_in = u64::from(start.deadline_misses > start.deadline_recoveries);
    let maximum_recoveries = delta
        .deadline_misses
        .checked_add(carry_in)
        .ok_or_else(|| format!("persistent output {name} recovery allowance overflow"))?;
    if delta.deadline_recoveries > maximum_recoveries {
        return Err(format!(
            "persistent output {name} recoveries exceed misses plus one valid carry-in"
        ));
    }
    if delta.repeated_quantums > delta.deadline_misses
        || delta.deadline_misses
            > delta
                .repeated_quantums
                .checked_add(delta.dropped_quantums)
                .ok_or_else(|| format!("persistent output {name} dispositions overflow"))?
    {
        return Err(format!(
            "persistent output {name} dispositions are impossible"
        ));
    }
    Ok(())
}

fn counters_le(left: PersistentOutputCounters, right: PersistentOutputCounters) -> bool {
    left.rendered_quantums <= right.rendered_quantums
        && left.repeated_quantums <= right.repeated_quantums
        && left.dropped_quantums <= right.dropped_quantums
        && left.deadline_misses <= right.deadline_misses
        && left.deadline_recoveries <= right.deadline_recoveries
}

#[cfg(test)]
#[path = "output_counters_tests.rs"]
mod tests;
