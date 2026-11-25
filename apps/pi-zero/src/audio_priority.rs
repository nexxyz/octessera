mod callback_scheduling;
#[cfg(all(target_os = "linux", not(board_host)))]
mod memory_lock;
mod syscalls;
#[cfg(all(target_os = "linux", not(board_host)))]
mod timer_slack;

#[cfg(all(target_os = "linux", not(board_host)))]
pub(crate) use memory_lock::lock_process_memory;
#[cfg(all(target_os = "linux", not(board_host)))]
pub(crate) use timer_slack::tighten_timer_slack;

#[cfg(test)]
pub(crate) use callback_scheduling::CallbackSchedulingStatus;
pub(crate) use callback_scheduling::{qualify_callback_scheduler, CallbackSchedulingHandle};

#[cfg(any(test, board_orange, board_raspberry))]
use syscalls::configure_strict;
#[cfg(test)]
use syscalls::SCHED_FIFO_POLICY;
use syscalls::{configure_affinity_only, CPU_MASK_WORDS};

pub(crate) const PI_RUNTIME_CPU: usize = 0;
pub(crate) const PI_JACK_CPU: usize = 1;
pub(crate) const PI_MIRROR_CPU: usize = 0;
pub(crate) const PI_JACK_CALLBACK_PRIORITY: i32 = 70;
pub(crate) const PI_MIRROR_CALLBACK_PRIORITY: i32 = 60;
#[cfg(any(test, all(target_os = "linux", not(board_host))))]
pub(crate) const PI_RUNTIME_PRIORITY: i32 = 1;

#[cfg(any(test, board_orange, board_raspberry))]
mod dsp_worker_scheduling;

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
            // SAFETY: CPU_ISSET only reads the borrowed set, and `cpu` stays below CPU_SETSIZE.
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

pub(crate) fn pin_main_thread_to_cpu0() -> Result<(), String> {
    configure_affinity_only(PI_RUNTIME_CPU)
        .map(|_| ())
        .map_err(|failure| syscalls::format_affinity_failure("main", failure))
}

// Reset-on-fork keeps every thread and process the loop spawns later at SCHED_OTHER.
#[cfg(any(test, all(target_os = "linux", not(board_host))))]
pub(crate) fn promote_runtime_thread() {
    match syscalls::configure_strict_reset_on_fork(PI_RUNTIME_CPU, PI_RUNTIME_PRIORITY) {
        Ok(effective) => {
            eprintln!(
                "runtime thread scheduling qualified: cpu={} policy=SCHED_FIFO priority={} reset_on_fork=1",
                effective.cpu.unwrap_or(usize::MAX),
                effective.priority,
            );
        }
        Err(failure) => eprintln!("{}", syscalls::format_failure("runtime", failure)),
    }
}

#[cfg(any(test, all(target_os = "linux", not(board_host))))]
pub(crate) fn demote_runtime_thread() {
    if let Err(errno) = syscalls::restore_normal_scheduling() {
        eprintln!("runtime thread return to SCHED_OTHER failed: errno={errno}");
    }
}

#[cfg(test)]
pub(crate) use syscalls::{
    install as install_test_scheduling, install_blocked as install_blocked_test_scheduling,
    InjectedSchedulingOutcomes, SchedulingSyscall,
};

#[cfg(any(test, board_orange, board_raspberry))]
pub(crate) use dsp_worker_scheduling::pi_worker_start_hook;
#[cfg(not(board_host))]
pub(crate) use dsp_worker_scheduling::{
    idle_signal_analysis_thread, pin_background_worker_to_dsp_cpus, pin_midi_thread,
};
#[cfg(test)]
pub(crate) use dsp_worker_scheduling::{ORANGE_WORKER_CPUS, ORANGE_WORKER_PRIORITY};

#[cfg(test)]
mod tests;
