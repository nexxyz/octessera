use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[path = "audio_priority_syscalls.rs"]
mod syscalls;

#[cfg(test)]
use syscalls::SCHED_FIFO_POLICY;
use syscalls::{configure_legacy, configure_strict, CPU_MASK_WORDS};

pub(crate) const ORANGE_JACK_CPU: usize = 1;
pub(crate) const ORANGE_WORKER_CPUS: [usize; 2] = [2, 3];
pub(crate) const ORANGE_WORKER_PRIORITY: i32 = 70;
pub(crate) const ORANGE_CALLBACK_PRIORITY: i32 = 70;
pub(crate) const ORANGE_SECONDARY_CALLBACK_PRIORITY: i32 = 69;
pub(crate) const RASPBERRY_CALLBACK_PRIORITY: i32 = 70;

#[path = "orange_worker_scheduling.rs"]
mod worker_scheduling;

const STATE_PENDING: u8 = 0;
const STATE_CONFIGURING: u8 = 1;
const STATE_QUALIFIED: u8 = 2;
const STATE_FAILED: u8 = 3;
const STATE_UNSUPPORTED: u8 = 4;
const STATE_TIMED_OUT: u8 = 5;
const STATE_PUBLISHING_QUALIFIED: u8 = 6;
const STATE_PUBLISHING_FAILED: u8 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum SchedulingFailureStage {
    AffinitySet = 1,
    AffinityGet = 2,
    AffinityMismatch = 3,
    SchedulingSet = 4,
    SchedulingGet = 5,
    SchedulingMismatch = 6,
    Unsupported = 7,
    Timeout = 8,
}

impl SchedulingFailureStage {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::AffinitySet,
            2 => Self::AffinityGet,
            3 => Self::AffinityMismatch,
            4 => Self::SchedulingSet,
            5 => Self::SchedulingGet,
            6 => Self::SchedulingMismatch,
            8 => Self::Timeout,
            _ => Self::Unsupported,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::AffinitySet => "affinity_set",
            Self::AffinityGet => "affinity_get",
            Self::AffinityMismatch => "affinity_mismatch",
            Self::SchedulingSet => "scheduling_set",
            Self::SchedulingGet => "scheduling_get",
            Self::SchedulingMismatch => "scheduling_mismatch",
            Self::Unsupported => "unsupported",
            Self::Timeout => "timeout",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CpuMask {
    pub(crate) bits: [u64; CPU_MASK_WORDS],
    pub(crate) extra_cpu: bool,
}

impl CpuMask {
    pub(crate) const fn empty() -> Self {
        Self {
            bits: [0; CPU_MASK_WORDS],
            extra_cpu: false,
        }
    }

    #[cfg(any(test, all(not(test), target_os = "linux")))]
    pub(crate) fn single(cpu: usize) -> Self {
        let mut mask = Self::empty();
        if cpu < u64::BITS as usize {
            mask.bits[cpu / u64::BITS as usize] |= 1 << (cpu % u64::BITS as usize);
        } else {
            mask.extra_cpu = true;
        }
        mask
    }

    #[cfg(all(not(test), target_os = "linux"))]
    pub(crate) fn from_linux(set: &libc::cpu_set_t) -> Self {
        let mut mask = Self::empty();
        for cpu in 0..libc::CPU_SETSIZE as usize {
            if unsafe { libc::CPU_ISSET(cpu, set) } {
                if cpu < u64::BITS as usize {
                    mask.bits[cpu / u64::BITS as usize] |= 1 << (cpu % u64::BITS as usize);
                } else {
                    mask.extra_cpu = true;
                }
            }
        }
        mask
    }
}

impl std::ops::BitOr for CpuMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            bits: std::array::from_fn(|index| self.bits[index] | rhs.bits[index]),
            extra_cpu: self.extra_cpu || rhs.extra_cpu,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EffectiveScheduling {
    pub(crate) policy: i32,
    pub(crate) priority: i32,
    pub(crate) cpu: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SchedulingFailure {
    pub(crate) stage: SchedulingFailureStage,
    pub(crate) errno: i32,
    pub(crate) requested_cpu: usize,
    pub(crate) requested_priority: i32,
    pub(crate) observed_mask: CpuMask,
    pub(crate) observed_policy: i32,
    pub(crate) observed_priority: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallbackSchedulingStatus {
    Pending,
    Qualified(EffectiveScheduling),
    Failed(SchedulingFailure),
    Unsupported,
    TimedOut,
}

struct CallbackSchedulingState {
    state: AtomicU8,
    failure_stage: AtomicU8,
    failure_errno: AtomicI32,
    requested_cpu: AtomicI32,
    requested_priority: AtomicI32,
    observed_mask: [AtomicU64; CPU_MASK_WORDS],
    observed_mask_extra_cpu: AtomicBool,
    observed_policy: AtomicI32,
    observed_priority: AtomicI32,
    effective_policy: AtomicI32,
    effective_priority: AtomicI32,
    effective_cpu: AtomicI32,
}

#[derive(Clone, Copy)]
enum CallbackSchedulingRole {
    Legacy,
    OrangeJack,
}

#[derive(Clone)]
pub(crate) struct CallbackSchedulingHandle {
    state: Arc<CallbackSchedulingState>,
    role: CallbackSchedulingRole,
    requested_priority: i32,
}

impl CallbackSchedulingHandle {
    pub(crate) fn new(requested_priority: i32) -> Self {
        Self::new_with_role(requested_priority, CallbackSchedulingRole::Legacy, None)
    }

    pub(crate) fn new_orange_jack() -> Self {
        Self::new_with_role(
            ORANGE_CALLBACK_PRIORITY,
            CallbackSchedulingRole::OrangeJack,
            Some(ORANGE_JACK_CPU),
        )
    }

    fn new_with_role(
        requested_priority: i32,
        role: CallbackSchedulingRole,
        requested_cpu: Option<usize>,
    ) -> Self {
        Self {
            state: Arc::new(CallbackSchedulingState {
                state: AtomicU8::new(STATE_PENDING),
                failure_stage: AtomicU8::new(0),
                failure_errno: AtomicI32::new(0),
                requested_cpu: AtomicI32::new(requested_cpu.map_or(-1, |cpu| cpu as i32)),
                requested_priority: AtomicI32::new(requested_priority),
                observed_mask: std::array::from_fn(|_| AtomicU64::new(0)),
                observed_mask_extra_cpu: AtomicBool::new(false),
                observed_policy: AtomicI32::new(0),
                observed_priority: AtomicI32::new(0),
                effective_policy: AtomicI32::new(0),
                effective_priority: AtomicI32::new(0),
                effective_cpu: AtomicI32::new(-1),
            }),
            role,
            requested_priority,
        }
    }

    pub(crate) fn configure_callback_thread(&self) -> bool {
        let state = self.state.state.load(Ordering::Acquire);
        if state != STATE_PENDING {
            return state == STATE_QUALIFIED;
        }
        if self
            .state
            .state
            .compare_exchange(
                STATE_PENDING,
                STATE_CONFIGURING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            return self.state.state.load(Ordering::Acquire) == STATE_QUALIFIED;
        }
        let result = match self.role {
            CallbackSchedulingRole::Legacy => configure_legacy(self.requested_priority),
            CallbackSchedulingRole::OrangeJack => {
                configure_strict(ORANGE_JACK_CPU, self.requested_priority)
            }
        };
        match result {
            Ok(effective) => self.publish_qualified(effective),
            Err(failure) => {
                if !matches!(self.role, CallbackSchedulingRole::Legacy)
                    || failure.stage != SchedulingFailureStage::Unsupported
                {
                    self.publish_failed(failure);
                    false
                } else {
                    let _ = self.state.state.compare_exchange(
                        STATE_CONFIGURING,
                        STATE_UNSUPPORTED,
                        Ordering::Release,
                        Ordering::Acquire,
                    );
                    false
                }
            }
        }
    }

    pub(crate) fn status(&self) -> CallbackSchedulingStatus {
        match self.state.state.load(Ordering::Acquire) {
            STATE_QUALIFIED => CallbackSchedulingStatus::Qualified(EffectiveScheduling {
                policy: self.state.effective_policy.load(Ordering::Relaxed),
                priority: self.state.effective_priority.load(Ordering::Relaxed),
                cpu: match self.state.effective_cpu.load(Ordering::Relaxed) {
                    value if value < 0 => None,
                    value => Some(value as usize),
                },
            }),
            STATE_FAILED => CallbackSchedulingStatus::Failed(self.failure()),
            STATE_UNSUPPORTED => CallbackSchedulingStatus::Unsupported,
            STATE_TIMED_OUT => CallbackSchedulingStatus::TimedOut,
            _ => CallbackSchedulingStatus::Pending,
        }
    }

    pub(crate) fn wait_for_status(&self, timeout: Duration) -> CallbackSchedulingStatus {
        let started = std::time::Instant::now();
        loop {
            let status = self.status();
            if !matches!(status, CallbackSchedulingStatus::Pending) {
                return status;
            }
            if started.elapsed() >= timeout {
                return status;
            }
            std::thread::yield_now();
        }
    }

    pub(crate) fn requested_priority(&self) -> i32 {
        self.requested_priority
    }

    pub(crate) fn is_strict(&self) -> bool {
        matches!(self.role, CallbackSchedulingRole::OrangeJack)
    }

    fn publish_timeout(&self) -> CallbackSchedulingStatus {
        let mut state = self.state.state.load(Ordering::Acquire);
        loop {
            if !matches!(state, STATE_PENDING | STATE_CONFIGURING) {
                break;
            }
            match self.state.state.compare_exchange(
                state,
                STATE_TIMED_OUT,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => return CallbackSchedulingStatus::TimedOut,
                Err(next) => state = next,
            }
        }
        loop {
            let status = self.status();
            if !matches!(status, CallbackSchedulingStatus::Pending) {
                return status;
            }
            std::thread::yield_now();
        }
    }

    fn timeout_failure(&self) -> SchedulingFailure {
        SchedulingFailure {
            stage: SchedulingFailureStage::Timeout,
            errno: 0,
            requested_cpu: ORANGE_JACK_CPU,
            requested_priority: self.requested_priority,
            observed_mask: CpuMask::empty(),
            observed_policy: 0,
            observed_priority: 0,
        }
    }

    fn publish_qualified(&self, effective: EffectiveScheduling) -> bool {
        if self
            .state
            .state
            .compare_exchange(
                STATE_CONFIGURING,
                STATE_PUBLISHING_QUALIFIED,
                Ordering::Acquire,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        self.state
            .effective_policy
            .store(effective.policy, Ordering::Relaxed);
        self.state
            .effective_priority
            .store(effective.priority, Ordering::Relaxed);
        self.state.effective_cpu.store(
            effective.cpu.map_or(-1, |cpu| cpu as i32),
            Ordering::Relaxed,
        );
        self.state.state.store(STATE_QUALIFIED, Ordering::Release);
        true
    }

    fn publish_failed(&self, failure: SchedulingFailure) -> bool {
        if self
            .state
            .state
            .compare_exchange(
                STATE_CONFIGURING,
                STATE_PUBLISHING_FAILED,
                Ordering::Acquire,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        self.state
            .failure_stage
            .store(failure.stage as u8, Ordering::Relaxed);
        self.state
            .failure_errno
            .store(failure.errno, Ordering::Relaxed);
        self.state
            .requested_cpu
            .store(failure.requested_cpu as i32, Ordering::Relaxed);
        self.state
            .requested_priority
            .store(failure.requested_priority, Ordering::Relaxed);
        for (slot, word) in self
            .state
            .observed_mask
            .iter()
            .zip(failure.observed_mask.bits)
        {
            slot.store(word, Ordering::Relaxed);
        }
        self.state
            .observed_mask_extra_cpu
            .store(failure.observed_mask.extra_cpu, Ordering::Relaxed);
        self.state
            .observed_policy
            .store(failure.observed_policy, Ordering::Relaxed);
        self.state
            .observed_priority
            .store(failure.observed_priority, Ordering::Relaxed);
        self.state.state.store(STATE_FAILED, Ordering::Release);
        true
    }

    fn failure(&self) -> SchedulingFailure {
        SchedulingFailure {
            stage: SchedulingFailureStage::from_u8(
                self.state.failure_stage.load(Ordering::Relaxed),
            ),
            errno: self.state.failure_errno.load(Ordering::Relaxed),
            requested_cpu: match self.state.requested_cpu.load(Ordering::Relaxed) {
                value if value < 0 => usize::MAX,
                value => value as usize,
            },
            requested_priority: self.state.requested_priority.load(Ordering::Relaxed),
            observed_mask: CpuMask {
                bits: std::array::from_fn(|index| {
                    self.state.observed_mask[index].load(Ordering::Relaxed)
                }),
                extra_cpu: self.state.observed_mask_extra_cpu.load(Ordering::Relaxed),
            },
            observed_policy: self.state.observed_policy.load(Ordering::Relaxed),
            observed_priority: self.state.observed_priority.load(Ordering::Relaxed),
        }
    }
}

pub(crate) fn callback_priority() -> i32 {
    if cfg!(feature = "hardware-orange-pi-zero-2w") {
        ORANGE_SECONDARY_CALLBACK_PRIORITY
    } else {
        RASPBERRY_CALLBACK_PRIORITY
    }
}

pub(crate) fn qualify_callback_scheduler(
    sink_label: &str,
    scheduler: &CallbackSchedulingHandle,
    timeout: Duration,
) -> Result<EffectiveScheduling, String> {
    let mut status = scheduler.wait_for_status(timeout);
    if matches!(status, CallbackSchedulingStatus::Pending) && scheduler.is_strict() {
        status = scheduler.publish_timeout();
    }
    match status {
        CallbackSchedulingStatus::Qualified(effective) => {
            eprintln!(
                "{sink_label} audio callback scheduling qualified: policy=SCHED_FIFO priority={}",
                effective.priority
            );
            Ok(effective)
        }
        CallbackSchedulingStatus::Pending => Err(format!(
            "{sink_label} audio callback RT promotion not qualified: callback did not report within {} ms (requested policy=SCHED_FIFO priority={})",
            timeout.as_millis(),
            scheduler.requested_priority()
        )),
        CallbackSchedulingStatus::Failed(failure) => {
            Err(syscalls::format_failure(sink_label, failure))
        }
        CallbackSchedulingStatus::TimedOut => {
            Err(syscalls::format_failure(sink_label, scheduler.timeout_failure()))
        }
        CallbackSchedulingStatus::Unsupported => Err(format!(
            "{sink_label} audio callback RT promotion not qualified: pthread scheduling is unsupported on this platform"
        )),
    }
}

#[cfg(test)]
pub(crate) use syscalls::{
    install as install_test_scheduling, install_blocked as install_blocked_test_scheduling,
    InjectedSchedulingOutcomes, SchedulingSyscall,
};

#[cfg(feature = "source-worker-benchmark-timing")]
pub(crate) use syscalls::orange_cpu_sampler;
#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use worker_scheduling::orange_worker_start_hook;

#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
pub(crate) fn scheduling_policy_name(policy: i32) -> &'static str {
    syscalls::scheduling_policy_name(policy)
}
#[cfg(test)]
#[path = "audio_priority_tests.rs"]
mod tests;
