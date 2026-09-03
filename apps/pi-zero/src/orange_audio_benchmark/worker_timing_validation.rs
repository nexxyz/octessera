use super::{BenchmarkCoordinatorTiming, BenchmarkWorkerTiming, BenchmarkWorkerTimingWorker};
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer};

const WORKER_MASK: u8 = 0b11;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkWorkerTimingUnchecked {
    workers: [BenchmarkWorkerTimingWorker; 2],
    coordinator: BenchmarkCoordinatorTiming,
    late_after_deadline_ns: Option<u64>,
    cpu_endpoint_changed: bool,
}

impl<'de> Deserialize<'de> for BenchmarkWorkerTiming {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = BenchmarkWorkerTimingUnchecked::deserialize(deserializer)?;
        validate(unchecked).map_err(D::Error::custom)
    }
}

fn validate(unchecked: BenchmarkWorkerTimingUnchecked) -> Result<BenchmarkWorkerTiming, String> {
    let BenchmarkWorkerTimingUnchecked {
        workers,
        coordinator,
        late_after_deadline_ns,
        cpu_endpoint_changed,
    } = unchecked;
    if !coordinator.frozen {
        return Err("worker timing coordinator must be frozen".into());
    }

    if coordinator.sequence.is_none() {
        validate_unexecuted(
            &workers,
            &coordinator,
            late_after_deadline_ns,
            cpu_endpoint_changed,
        )?;
    } else {
        validate_executed(
            &workers,
            &coordinator,
            late_after_deadline_ns,
            cpu_endpoint_changed,
        )?;
    }

    Ok(BenchmarkWorkerTiming {
        workers,
        coordinator,
        late_after_deadline_ns,
        cpu_endpoint_changed,
    })
}

fn validate_unexecuted(
    workers: &[BenchmarkWorkerTimingWorker; 2],
    coordinator: &BenchmarkCoordinatorTiming,
    late_after_deadline_ns: Option<u64>,
    cpu_endpoint_changed: bool,
) -> Result<(), String> {
    let measurements = [
        ("deadline_ns", coordinator.deadline_ns.is_some()),
        (
            "dispatch_to_deadline_start_ns",
            coordinator.dispatch_to_deadline_start_ns.is_some(),
        ),
        (
            "dispatch_to_deadline_elapsed_ns",
            coordinator.dispatch_to_deadline_elapsed_ns.is_some(),
        ),
        ("in_flight_mask", coordinator.in_flight_mask.is_some()),
        ("completed_mask", coordinator.completed_mask.is_some()),
        ("first_parity", coordinator.first_parity.is_some()),
        (
            "dispatch_to_first_ns",
            coordinator.dispatch_to_first_ns.is_some(),
        ),
        (
            "dispatch_to_both_ns",
            coordinator.dispatch_to_both_ns.is_some(),
        ),
        ("reduction_ns", coordinator.reduction_ns.is_some()),
        (
            "coordinator_remainder_ns",
            coordinator.coordinator_remainder_ns.is_some(),
        ),
        (
            "engine_block_total_ns",
            coordinator.engine_block_total_ns.is_some(),
        ),
        ("callback_total_ns", coordinator.callback_total_ns.is_some()),
    ];
    if let Some((name, _)) = measurements.into_iter().find(|(_, present)| *present) {
        return Err(format!(
            "unexecuted coordinator has non-null measurement: {name}"
        ));
    }
    if workers
        .iter()
        .any(|worker| worker.finished || worker_has_evidence(worker))
    {
        return Err("unexecuted coordinator has worker evidence".into());
    }
    if late_after_deadline_ns.is_some() || cpu_endpoint_changed {
        return Err("unexecuted coordinator has a non-null summary".into());
    }
    Ok(())
}

fn validate_executed(
    workers: &[BenchmarkWorkerTimingWorker; 2],
    coordinator: &BenchmarkCoordinatorTiming,
    late_after_deadline_ns: Option<u64>,
    cpu_endpoint_changed: bool,
) -> Result<(), String> {
    let sequence = coordinator.sequence.expect("executed coordinator sequence");
    let dispatch_to_deadline_start = required(
        coordinator.dispatch_to_deadline_start_ns,
        "dispatch_to_deadline_start_ns",
    )?;
    let deadline_duration = required(coordinator.deadline_ns, "deadline_ns")?;
    let deadline_boundary = dispatch_to_deadline_start
        .checked_add(deadline_duration)
        .ok_or_else(|| "deadline boundary overflows elapsed timing".to_string())?;
    let in_flight = required(coordinator.in_flight_mask, "in_flight_mask")?;
    let completed = required(coordinator.completed_mask, "completed_mask")?;
    let engine_total = required(coordinator.engine_block_total_ns, "engine_block_total_ns")?;
    let callback_total = required(coordinator.callback_total_ns, "callback_total_ns")?;

    if in_flight & !WORKER_MASK != 0 || completed & !WORKER_MASK != 0 {
        return Err("coordinator masks contain an unknown worker bit".into());
    }
    if in_flight & completed != 0 || in_flight | completed != WORKER_MASK {
        return Err("coordinator masks do not partition both dispatched workers".into());
    }
    if engine_total > callback_total {
        return Err("engine block total exceeds callback total".into());
    }
    if coordinator
        .dispatch_to_deadline_elapsed_ns
        .is_some_and(|elapsed| elapsed < deadline_boundary)
    {
        return Err("deadline elapsed timing precedes the dispatch deadline boundary".into());
    }
    if !coordinator.failed && coordinator.dispatch_to_deadline_elapsed_ns.is_some() {
        return Err("healthy timing has deadline elapsed evidence".into());
    }
    if !coordinator.failed {
        if in_flight != 0 || completed != WORKER_MASK {
            return Err("healthy timing did not complete both workers".into());
        }
        required(coordinator.reduction_ns, "reduction_ns")?;
        required(
            coordinator.coordinator_remainder_ns,
            "coordinator_remainder_ns",
        )?;
    } else if completed != WORKER_MASK
        && (coordinator.reduction_ns.is_some() || coordinator.coordinator_remainder_ns.is_some())
    {
        return Err("failed timing contains reduction evidence without both completions".into());
    } else if coordinator.coordinator_remainder_ns.is_some() && coordinator.reduction_ns.is_none() {
        return Err("coordinator remainder evidence has no reduction evidence".into());
    }

    let worker_state = validate_workers(workers, sequence, cpu_endpoint_changed)?;
    validate_completion_observations(coordinator, &worker_state, deadline_boundary, completed)?;
    validate_summaries(
        workers,
        sequence,
        deadline_boundary,
        late_after_deadline_ns,
        &worker_state,
    )
}

fn validate_workers(
    workers: &[BenchmarkWorkerTimingWorker; 2],
    sequence: u64,
    cpu_endpoint_changed: bool,
) -> Result<[WorkerEvidence; 2], String> {
    let mut evidence = [WorkerEvidence::default(); 2];
    for (parity, worker) in workers.iter().enumerate() {
        if !worker.finished {
            if worker_has_evidence(worker) {
                return Err(format!("unfinished worker {parity} has evidence"));
            }
            continue;
        }
        let worker_sequence = required(worker.sequence, &format!("worker {parity} sequence"))?;
        let render = required(worker.render_ns, &format!("worker {parity} render_ns"))?;
        let dispatch_to_finish = required(
            worker.dispatch_to_finish_ns,
            &format!("worker {parity} dispatch_to_finish_ns"),
        )?;
        if worker_sequence != sequence {
            return Err(format!(
                "worker {parity} sequence does not match coordinator"
            ));
        }
        if dispatch_to_finish < render {
            return Err(format!(
                "worker {parity} dispatch timing precedes render timing"
            ));
        }
        let expected_cpu = [2_u32, 3_u32][parity];
        match (worker.cpu_start, worker.cpu_end) {
            (Some(start), Some(end)) if start == expected_cpu && end == expected_cpu => {}
            (Some(_), Some(_)) => {
                return Err(format!(
                    "worker {parity} CPU evidence does not match its fixed Orange CPU"
                ));
            }
            (None, None) => {
                return Err(format!("worker {parity} is missing fixed CPU evidence"));
            }
            _ => return Err(format!("worker {parity} has partial CPU evidence")),
        }
        evidence[parity] = WorkerEvidence {
            dispatch_to_finish: Some(dispatch_to_finish),
            finished: true,
        };
    }
    if cpu_endpoint_changed {
        return Err("worker CPU endpoint-change summary must be false".into());
    }
    Ok(evidence)
}

fn validate_completion_observations(
    coordinator: &BenchmarkCoordinatorTiming,
    worker_state: &[WorkerEvidence; 2],
    deadline_boundary: u64,
    completed: u8,
) -> Result<(), String> {
    if completed == 0 {
        if coordinator.first_parity.is_some()
            || coordinator.dispatch_to_first_ns.is_some()
            || coordinator.dispatch_to_both_ns.is_some()
        {
            return Err("zero completed workers have completion evidence".into());
        }
        return Ok(());
    }

    let first_parity = required(coordinator.first_parity, "first_parity")?;
    if first_parity > 1 || completed & (1 << first_parity) == 0 {
        return Err("first completion parity is absent from the completed mask".into());
    }
    let first = required(coordinator.dispatch_to_first_ns, "dispatch_to_first_ns")?;
    let first_worker = worker_state[first_parity]
        .dispatch_to_finish
        .ok_or_else(|| "first completion has no finished worker evidence".to_string())?;
    if first < first_worker {
        return Err("first completion observation precedes the selected worker finish".into());
    }
    if first > deadline_boundary || first_worker > deadline_boundary {
        return Err("completed-before-deadline evidence exceeds the deadline".into());
    }

    if completed == WORKER_MASK {
        let both = required(coordinator.dispatch_to_both_ns, "dispatch_to_both_ns")?;
        if first > both {
            return Err("first completion observation follows both completion".into());
        }
        for (parity, worker) in worker_state.iter().enumerate() {
            let finish = worker
                .dispatch_to_finish
                .ok_or_else(|| format!("completed worker {parity} has no finish evidence"))?;
            if both < finish || finish > deadline_boundary {
                return Err("both completion observation is temporally impossible".into());
            }
        }
        if both > deadline_boundary {
            return Err("both completion evidence exceeds the deadline".into());
        }
    } else if coordinator.dispatch_to_both_ns.is_some() {
        return Err("both completion evidence exists without both completions".into());
    }
    Ok(())
}

fn validate_summaries(
    workers: &[BenchmarkWorkerTimingWorker; 2],
    sequence: u64,
    deadline_boundary: u64,
    late_after_deadline_ns: Option<u64>,
    worker_state: &[WorkerEvidence; 2],
) -> Result<(), String> {
    let expected_late = worker_state
        .iter()
        .zip(workers)
        .filter(|(state, worker)| {
            state.finished
                && worker.sequence == Some(sequence)
                && state
                    .dispatch_to_finish
                    .is_some_and(|finish| finish > deadline_boundary)
        })
        .map(|(state, _)| state.dispatch_to_finish.expect("late worker finish") - deadline_boundary)
        .max();
    if late_after_deadline_ns != expected_late {
        return Err("late-after-deadline summary does not match worker endpoints".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct WorkerEvidence {
    dispatch_to_finish: Option<u64>,
    finished: bool,
}

fn worker_has_evidence(worker: &BenchmarkWorkerTimingWorker) -> bool {
    worker.sequence.is_some()
        || worker.render_ns.is_some()
        || worker.dispatch_to_finish_ns.is_some()
        || worker.cpu_start.is_some()
        || worker.cpu_end.is_some()
}

fn required<T>(value: Option<T>, name: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("executed worker timing is missing {name}"))
}
