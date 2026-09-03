use super::*;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SchedulingSyscall {
    SetAffinity,
    GetAffinity,
    SetScheduling,
    GetScheduling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InjectedSchedulingOutcomes {
    pub(crate) target_cpu: Option<usize>,
    pub(crate) affinity_set_errno: Option<i32>,
    pub(crate) affinity_get_errno: Option<i32>,
    pub(crate) observed_affinity: Option<CpuMask>,
    pub(crate) scheduling_set_errno: Option<i32>,
    pub(crate) scheduling_get_errno: Option<i32>,
    pub(crate) observed_scheduling: Option<EffectiveScheduling>,
}

impl InjectedSchedulingOutcomes {
    pub(crate) const fn success() -> Self {
        Self {
            target_cpu: None,
            affinity_set_errno: None,
            affinity_get_errno: None,
            observed_affinity: None,
            scheduling_set_errno: None,
            scheduling_get_errno: None,
            observed_scheduling: None,
        }
    }

    pub(crate) const fn success_for_cpu(cpu: usize) -> Self {
        Self {
            target_cpu: Some(cpu),
            ..Self::success()
        }
    }
}

const TRACE_CAPACITY: usize = 32;
static TEST_OUTCOMES: Mutex<InjectedSchedulingOutcomes> =
    Mutex::new(InjectedSchedulingOutcomes::success());
static TEST_TRACE: Mutex<Trace> = Mutex::new(Trace::new());
static TEST_SERIAL: Mutex<()> = Mutex::new(());
static TEST_BLOCK: Mutex<Option<Arc<SchedulingBlock>>> = Mutex::new(None);

pub(crate) struct SchedulingBlock {
    operation: SchedulingSyscall,
    state: Mutex<BlockingState>,
    changed: Condvar,
}

struct BlockingState {
    entered: bool,
    released: bool,
}

impl SchedulingBlock {
    fn new(operation: SchedulingSyscall) -> Self {
        Self {
            operation,
            state: Mutex::new(BlockingState {
                entered: false,
                released: false,
            }),
            changed: Condvar::new(),
        }
    }

    pub(crate) fn wait_until_entered(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.entered {
            let (next, result) = self
                .changed
                .wait_timeout(state, Duration::from_secs(1))
                .unwrap();
            assert!(!result.timed_out(), "scheduling syscall did not block");
            state = next;
        }
    }

    pub(crate) fn release(&self) {
        let mut state = self.state.lock().unwrap();
        state.released = true;
        self.changed.notify_all();
    }

    fn wait(&self) {
        let mut state = self.state.lock().unwrap();
        state.entered = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap();
        }
    }
}

struct Trace {
    operations: [Option<(SchedulingSyscall, usize)>; TRACE_CAPACITY],
    len: usize,
}

impl Trace {
    const fn new() -> Self {
        Self {
            operations: [None; TRACE_CAPACITY],
            len: 0,
        }
    }
}

pub(crate) struct TestSchedulingGuard {
    _serial: MutexGuard<'static, ()>,
    previous: InjectedSchedulingOutcomes,
}

pub(crate) fn install(outcomes: InjectedSchedulingOutcomes) -> TestSchedulingGuard {
    let serial = TEST_SERIAL.lock().unwrap();
    let mut configured = TEST_OUTCOMES.lock().unwrap();
    let previous = *configured;
    *configured = outcomes;
    TEST_TRACE.lock().unwrap().len = 0;
    *TEST_BLOCK.lock().unwrap() = None;
    TestSchedulingGuard {
        _serial: serial,
        previous,
    }
}

pub(crate) fn install_blocked(
    outcomes: InjectedSchedulingOutcomes,
    operation: SchedulingSyscall,
) -> (TestSchedulingGuard, Arc<SchedulingBlock>) {
    let guard = install(outcomes);
    let block = Arc::new(SchedulingBlock::new(operation));
    *TEST_BLOCK.lock().unwrap() = Some(Arc::clone(&block));
    (guard, block)
}

impl TestSchedulingGuard {
    pub(crate) fn trace_for_cpu(&self, requested_cpu: usize) -> Vec<SchedulingSyscall> {
        let trace = TEST_TRACE.lock().unwrap();
        trace.operations[..trace.len]
            .iter()
            .flatten()
            .filter(|(_, cpu)| *cpu == requested_cpu)
            .map(|(operation, _)| *operation)
            .collect()
    }
}

impl Drop for TestSchedulingGuard {
    fn drop(&mut self) {
        *TEST_OUTCOMES.lock().unwrap() = self.previous;
        TEST_TRACE.lock().unwrap().len = 0;
        if let Some(block) = TEST_BLOCK.lock().unwrap().take() {
            block.release();
        }
    }
}

pub(super) fn configure_strict(
    requested_cpu: usize,
    requested_priority: i32,
) -> Result<EffectiveScheduling, SchedulingFailure> {
    let outcomes = *TEST_OUTCOMES.lock().unwrap();
    if outcomes
        .target_cpu
        .is_some_and(|target_cpu| target_cpu != requested_cpu)
    {
        return Ok(EffectiveScheduling {
            policy: SCHED_FIFO_POLICY,
            priority: requested_priority,
            cpu: Some(requested_cpu),
        });
    }
    record(SchedulingSyscall::SetAffinity, requested_cpu);
    maybe_block(SchedulingSyscall::SetAffinity);
    if let Some(errno) = outcomes.affinity_set_errno {
        return Err(failure(
            SchedulingFailureStage::AffinitySet,
            errno,
            requested_cpu,
            requested_priority,
            CpuMask::empty(),
            0,
            0,
        ));
    }
    record(SchedulingSyscall::GetAffinity, requested_cpu);
    if let Some(errno) = outcomes.affinity_get_errno {
        return Err(failure(
            SchedulingFailureStage::AffinityGet,
            errno,
            requested_cpu,
            requested_priority,
            CpuMask::empty(),
            0,
            0,
        ));
    }
    let observed = outcomes
        .observed_affinity
        .unwrap_or_else(|| CpuMask::single(requested_cpu));
    if observed != CpuMask::single(requested_cpu) {
        return Err(failure(
            SchedulingFailureStage::AffinityMismatch,
            0,
            requested_cpu,
            requested_priority,
            observed,
            0,
            0,
        ));
    }
    configure_sched(&outcomes, requested_cpu, requested_priority, observed)
}

pub(super) fn configure_legacy(
    requested_priority: i32,
) -> Result<EffectiveScheduling, SchedulingFailure> {
    let outcomes = *TEST_OUTCOMES.lock().unwrap();
    if outcomes
        .target_cpu
        .is_some_and(|target_cpu| target_cpu != usize::MAX)
    {
        return Ok(EffectiveScheduling {
            policy: SCHED_FIFO_POLICY,
            priority: requested_priority,
            cpu: None,
        });
    }
    configure_sched(&outcomes, usize::MAX, requested_priority, CpuMask::empty())
}

fn configure_sched(
    outcomes: &InjectedSchedulingOutcomes,
    requested_cpu: usize,
    requested_priority: i32,
    observed_mask: CpuMask,
) -> Result<EffectiveScheduling, SchedulingFailure> {
    record(SchedulingSyscall::SetScheduling, requested_cpu);
    maybe_block(SchedulingSyscall::SetScheduling);
    if let Some(errno) = outcomes.scheduling_set_errno {
        return Err(failure(
            SchedulingFailureStage::SchedulingSet,
            errno,
            requested_cpu,
            requested_priority,
            observed_mask,
            0,
            0,
        ));
    }
    record(SchedulingSyscall::GetScheduling, requested_cpu);
    if let Some(errno) = outcomes.scheduling_get_errno {
        return Err(failure(
            SchedulingFailureStage::SchedulingGet,
            errno,
            requested_cpu,
            requested_priority,
            observed_mask,
            0,
            0,
        ));
    }
    let effective = outcomes.observed_scheduling.unwrap_or(EffectiveScheduling {
        policy: SCHED_FIFO_POLICY,
        priority: requested_priority,
        cpu: (requested_cpu != usize::MAX).then_some(requested_cpu),
    });
    if effective.policy != SCHED_FIFO_POLICY || effective.priority != requested_priority {
        return Err(failure(
            SchedulingFailureStage::SchedulingMismatch,
            0,
            requested_cpu,
            requested_priority,
            observed_mask,
            effective.policy,
            effective.priority,
        ));
    }
    Ok(EffectiveScheduling {
        policy: effective.policy,
        priority: effective.priority,
        cpu: (requested_cpu != usize::MAX).then_some(requested_cpu),
    })
}

fn record(operation: SchedulingSyscall, requested_cpu: usize) {
    let mut trace = TEST_TRACE.lock().unwrap();
    if trace.len < TRACE_CAPACITY {
        let index = trace.len;
        trace.operations[index] = Some((operation, requested_cpu));
        trace.len = index + 1;
    }
}

fn maybe_block(operation: SchedulingSyscall) {
    let block = TEST_BLOCK.lock().unwrap().clone();
    if block
        .as_ref()
        .is_some_and(|block| block.operation == operation)
    {
        block.expect("scheduling block disappeared").wait();
    }
}
