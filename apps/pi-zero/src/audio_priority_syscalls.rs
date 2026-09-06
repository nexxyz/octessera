use super::{CpuMask, EffectiveScheduling, SchedulingFailure, SchedulingFailureStage};

#[cfg(test)]
pub(super) const SCHED_FIFO_POLICY: i32 = 1;
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
        configure_sched(requested_cpu, requested_priority, observed)
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
        let mut observed_mask = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        let thread = unsafe { libc::pthread_self() };
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
    ) -> Result<EffectiveScheduling, SchedulingFailure> {
        let params = libc::sched_param {
            sched_priority: requested_priority,
        };
        let thread = unsafe { libc::pthread_self() };
        let result = unsafe { libc::pthread_setschedparam(thread, libc::SCHED_FIFO, &params) };
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
        if policy != libc::SCHED_FIFO || observed_params.sched_priority != requested_priority {
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
        let mut mask = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
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
    let prefix = if label == "worker" {
        "Orange DSP worker RT placement not qualified".to_string()
    } else {
        format!("{label} audio callback RT placement not qualified")
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

#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
pub(crate) fn scheduling_policy_name(policy: i32) -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if policy == libc::SCHED_FIFO {
            "SCHED_FIFO"
        } else {
            "unknown"
        }
    }
    #[cfg(all(not(target_os = "linux"), test))]
    {
        if policy == SCHED_FIFO_POLICY {
            "SCHED_FIFO"
        } else {
            "unsupported"
        }
    }
    #[cfg(all(not(target_os = "linux"), not(test)))]
    {
        let _ = policy;
        "unsupported"
    }
}

#[cfg(feature = "source-worker-benchmark-timing")]
pub(crate) fn orange_cpu_sampler() -> u32 {
    #[cfg(target_os = "linux")]
    {
        let cpu = unsafe { libc::sched_getcpu() };
        if cpu >= 0 {
            cpu as u32
        } else {
            u32::MAX
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        u32::MAX
    }
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
#[path = "audio_priority_syscalls_tests.rs"]
mod test;
