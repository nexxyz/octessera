use super::{CpuMask, EffectiveScheduling, SchedulingFailure, SchedulingFailureStage};

#[cfg(test)]
pub(super) const SCHED_FIFO_POLICY: i32 = 1;
#[cfg(test)]
pub(super) const SCHED_OTHER_POLICY: i32 = 0;
#[cfg(test)]
pub(super) const SCHED_RESET_ON_FORK_FLAG: i32 = 0x4000_0000;
pub(super) const CPU_MASK_WORDS: usize = 1;

pub(super) fn configure_affinity_only(requested_cpu: usize) -> Result<CpuMask, SchedulingFailure> {
    #[cfg(test)]
    {
        test::configure_affinity_only(requested_cpu)
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        linux::configure_affinity_only(requested_cpu)
    }
    #[cfg(all(not(test), not(target_os = "linux")))]
    {
        Err(unsupported(requested_cpu, 0))
    }
}

pub(super) fn configure_strict(
    requested_cpu: usize,
    requested_priority: i32,
) -> Result<EffectiveScheduling, SchedulingFailure> {
    #[cfg(test)]
    {
        test::configure_strict(requested_cpu, requested_priority)
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        linux::configure_strict(requested_cpu, requested_priority)
    }
    #[cfg(all(not(test), not(target_os = "linux")))]
    {
        Err(unsupported(requested_cpu, requested_priority))
    }
}

#[cfg(any(test, all(target_os = "linux", not(board_host))))]
pub(super) fn configure_strict_reset_on_fork(
    requested_cpu: usize,
    requested_priority: i32,
) -> Result<EffectiveScheduling, SchedulingFailure> {
    #[cfg(test)]
    {
        test::configure_strict_reset_on_fork(requested_cpu, requested_priority)
    }
    #[cfg(not(test))]
    {
        linux::configure_strict_reset_on_fork(requested_cpu, requested_priority)
    }
}

/// Returns the calling thread to SCHED_OTHER; the kernel keeps its nice value. Reset-on-fork
/// stays set, because clearing it needs CAP_SYS_NICE, which the Orange service lacks.
#[cfg(any(test, all(target_os = "linux", not(board_host))))]
pub(super) fn restore_normal_scheduling() -> Result<i32, i32> {
    #[cfg(test)]
    {
        test::restore_normal_scheduling(SCHED_OTHER_POLICY | SCHED_RESET_ON_FORK_FLAG)
    }
    #[cfg(not(test))]
    {
        let policy = libc::SCHED_OTHER | libc::SCHED_RESET_ON_FORK;
        let params = libc::sched_param { sched_priority: 0 };
        // SAFETY: pthread_self names the live calling thread and `params` outlives the call.
        match unsafe { libc::pthread_setschedparam(libc::pthread_self(), policy, &params) } {
            0 => Ok(policy),
            errno => Err(errno),
        }
    }
}

#[cfg(all(not(test), not(target_os = "linux")))]
fn unsupported(requested_cpu: usize, requested_priority: i32) -> SchedulingFailure {
    SchedulingFailure {
        stage: SchedulingFailureStage::Unsupported,
        errno: 0,
        requested_cpu,
        requested_priority,
        observed_mask: CpuMask::empty(),
        observed_policy: 0,
        observed_priority: 0,
    }
}

#[cfg(all(not(test), target_os = "linux"))]
mod linux {
    use super::*;
    use std::mem::size_of;

    pub(super) fn configure_strict(
        requested_cpu: usize,
        requested_priority: i32,
    ) -> Result<EffectiveScheduling, SchedulingFailure> {
        let observed = configure_affinity(requested_cpu, requested_priority)?;
        configure_sched(requested_cpu, requested_priority, observed, 0)
    }

    #[cfg(not(board_host))]
    pub(super) fn configure_strict_reset_on_fork(
        requested_cpu: usize,
        requested_priority: i32,
    ) -> Result<EffectiveScheduling, SchedulingFailure> {
        let observed = configure_affinity(requested_cpu, requested_priority)?;
        configure_sched(
            requested_cpu,
            requested_priority,
            observed,
            libc::SCHED_RESET_ON_FORK,
        )
    }

    pub(super) fn configure_affinity_only(
        requested_cpu: usize,
    ) -> Result<CpuMask, SchedulingFailure> {
        configure_affinity(requested_cpu, 0)
    }

    fn configure_affinity(
        requested_cpu: usize,
        requested_priority: i32,
    ) -> Result<CpuMask, SchedulingFailure> {
        let requested_mask = cpu_mask(requested_cpu);
        // SAFETY: cpu_set_t is a plain bit array, so all-zero bytes are a valid empty set.
        let mut observed_mask = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        // SAFETY: pthread_self has no preconditions and always returns the calling thread.
        let thread = unsafe { libc::pthread_self() };
        // SAFETY: `thread` is the live calling thread and the size matches the borrowed cpu_set_t.
        let result = unsafe {
            libc::pthread_setaffinity_np(thread, size_of::<libc::cpu_set_t>(), &requested_mask)
        };
        if result != 0 {
            return Err(failure(
                SchedulingFailureStage::AffinitySet,
                result,
                requested_cpu,
                requested_priority,
                CpuMask::empty(),
                0,
                0,
            ));
        }
        // SAFETY: `thread` is the calling thread; `observed_mask` is a writable cpu_set_t.
        let result = unsafe {
            libc::pthread_getaffinity_np(thread, size_of::<libc::cpu_set_t>(), &mut observed_mask)
        };
        let observed = CpuMask::from_linux(&observed_mask);
        if result != 0 {
            return Err(failure(
                SchedulingFailureStage::AffinityGet,
                result,
                requested_cpu,
                requested_priority,
                observed,
                0,
                0,
            ));
        }
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
        Ok(observed)
    }

    fn configure_sched(
        requested_cpu: usize,
        requested_priority: i32,
        observed_mask: CpuMask,
        policy_flags: i32,
    ) -> Result<EffectiveScheduling, SchedulingFailure> {
        let policy_requested = libc::SCHED_FIFO | policy_flags;
        let params = libc::sched_param {
            sched_priority: requested_priority,
        };
        // SAFETY: pthread_self has no preconditions and always returns the calling thread.
        let thread = unsafe { libc::pthread_self() };
        // SAFETY: `thread` is the live calling thread and `params` outlives the call.
        let result = unsafe { libc::pthread_setschedparam(thread, policy_requested, &params) };
        if result != 0 {
            return Err(failure(
                SchedulingFailureStage::SchedulingSet,
                result,
                requested_cpu,
                requested_priority,
                observed_mask,
                0,
                0,
            ));
        }
        let mut policy = 0;
        let mut observed_params = libc::sched_param { sched_priority: 0 };
        // SAFETY: `thread` is the calling thread; both out-pointers are writable locals.
        let result =
            unsafe { libc::pthread_getschedparam(thread, &mut policy, &mut observed_params) };
        if result != 0 {
            return Err(failure(
                SchedulingFailureStage::SchedulingGet,
                result,
                requested_cpu,
                requested_priority,
                observed_mask,
                policy,
                observed_params.sched_priority,
            ));
        }
        let effective = EffectiveScheduling {
            policy,
            priority: observed_params.sched_priority,
            cpu: (requested_cpu != usize::MAX).then_some(requested_cpu),
        };
        if policy != policy_requested || observed_params.sched_priority != requested_priority {
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
        Ok(effective)
    }

    fn cpu_mask(cpu: usize) -> libc::cpu_set_t {
        // SAFETY: cpu_set_t is a plain bit array, so all-zero bytes are a valid empty set.
        let mut mask = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        // SAFETY: both macros only touch the owned local mask; CPU_SET bounds-checks `cpu`.
        unsafe {
            libc::CPU_ZERO(&mut mask);
            libc::CPU_SET(cpu, &mut mask);
        }
        mask
    }
}

#[cfg(any(test, all(not(test), target_os = "linux")))]
fn failure(
    stage: SchedulingFailureStage,
    errno: i32,
    requested_cpu: usize,
    requested_priority: i32,
    observed_mask: CpuMask,
    observed_policy: i32,
    observed_priority: i32,
) -> SchedulingFailure {
    SchedulingFailure {
        stage,
        errno,
        requested_cpu,
        requested_priority,
        observed_mask,
        observed_policy,
        observed_priority,
    }
}

pub(super) fn format_failure(label: &str, failure: SchedulingFailure) -> String {
    let prefix = match label {
        "worker" => "Orange DSP worker RT placement not qualified".to_string(),
        "runtime" => "runtime thread RT placement not qualified".to_string(),
        _ => format!("{label} audio callback RT placement not qualified"),
    };
    if failure.stage == SchedulingFailureStage::Unsupported {
        return format!(
            "{prefix}: stage={} errno={} requested_cpu={} requested_policy=SCHED_FIFO requested_priority={} observed_mask={} observed_policy={} observed_priority={}",
            failure.stage.name(),
            failure.errno,
            failure.requested_cpu,
            failure.requested_priority,
            format_cpu_mask(failure.observed_mask),
            failure.observed_policy,
            failure.observed_priority,
        );
    }
    if label != "worker" && failure.requested_cpu == usize::MAX {
        let detail = match failure.stage {
            SchedulingFailureStage::SchedulingSet => {
                format!("pthread_setschedparam returned error {}", failure.errno)
            }
            SchedulingFailureStage::SchedulingGet => {
                format!("pthread_getschedparam returned error {}", failure.errno)
            }
            SchedulingFailureStage::SchedulingMismatch => format!(
                "effective policy={} priority={} did not match SCHED_FIFO priority={}",
                failure.observed_policy, failure.observed_priority, failure.requested_priority
            ),
            _ => format!("scheduling failed at {}", failure.stage.name()),
        };
        return format!("{label} audio callback RT promotion not qualified: {detail}");
    }
    format!(
        "{prefix}: stage={} errno={} requested_cpu={} requested_policy=SCHED_FIFO requested_priority={} observed_mask={} observed_policy={} observed_priority={}",
        failure.stage.name(),
        failure.errno,
        failure.requested_cpu,
        failure.requested_priority,
        format_cpu_mask(failure.observed_mask),
        failure.observed_policy,
        failure.observed_priority,
    )
}

pub(super) fn format_affinity_failure(label: &str, failure: SchedulingFailure) -> String {
    format!(
        "{label} thread CPU affinity not qualified: stage={} errno={} requested_cpu={} observed_mask={}",
        failure.stage.name(),
        failure.errno,
        failure.requested_cpu,
        format_cpu_mask(failure.observed_mask),
    )
}

fn format_cpu_mask(mask: CpuMask) -> String {
    if mask.extra_cpu {
        format!("0x{:x}+extra", mask.bits[0])
    } else {
        format!("0x{:x}", mask.bits[0])
    }
}

#[cfg(test)]
pub(crate) use test::{install, install_blocked, InjectedSchedulingOutcomes, SchedulingSyscall};

#[cfg(test)]
mod test;
