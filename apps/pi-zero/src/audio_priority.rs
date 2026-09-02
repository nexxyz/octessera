use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) const ORANGE_WORKER_PRIORITY: i32 = 70;
pub(crate) const ORANGE_CALLBACK_PRIORITY: i32 = 69;
pub(crate) const RASPBERRY_CALLBACK_PRIORITY: i32 = 70;

const STATE_PENDING: u8 = 0;
const STATE_CONFIGURING: u8 = 1;
const STATE_QUALIFIED: u8 = 2;
const STATE_FAILED: u8 = 3;
const STATE_UNSUPPORTED: u8 = 4;

const FAILURE_SET: u8 = 1;
const FAILURE_GET: u8 = 2;
const FAILURE_EFFECTIVE: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EffectiveScheduling {
    pub(crate) policy: i32,
    pub(crate) priority: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallbackSchedulingFailure {
    Set { error: i32 },
    Get { error: i32 },
    Effective(EffectiveScheduling),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallbackSchedulingStatus {
    Pending,
    Qualified(EffectiveScheduling),
    Failed(CallbackSchedulingFailure),
    Unsupported,
}

struct CallbackSchedulingState {
    state: AtomicU8,
    failure_kind: AtomicU8,
    failure_error: AtomicI32,
    effective_policy: AtomicI32,
    effective_priority: AtomicI32,
}

#[derive(Clone)]
pub(crate) struct CallbackSchedulingHandle {
    state: Arc<CallbackSchedulingState>,
    requested_priority: i32,
}

impl CallbackSchedulingHandle {
    pub(crate) fn new(requested_priority: i32) -> Self {
        Self {
            state: Arc::new(CallbackSchedulingState {
                state: AtomicU8::new(STATE_PENDING),
                failure_kind: AtomicU8::new(0),
                failure_error: AtomicI32::new(0),
                effective_policy: AtomicI32::new(0),
                effective_priority: AtomicI32::new(0),
            }),
            requested_priority,
        }
    }

    pub(crate) fn configure_callback_thread(&self) {
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
            return;
        }

        #[cfg(target_os = "linux")]
        match imp::configure(self.requested_priority) {
            Ok(effective) => {
                self.state
                    .effective_policy
                    .store(effective.policy, Ordering::Relaxed);
                self.state
                    .effective_priority
                    .store(effective.priority, Ordering::Relaxed);
                self.state.state.store(STATE_QUALIFIED, Ordering::Release);
            }
            Err(failure) => self.publish_failure(failure),
        }

        #[cfg(not(target_os = "linux"))]
        self.state.state.store(STATE_UNSUPPORTED, Ordering::Release);
    }

    pub(crate) fn status(&self) -> CallbackSchedulingStatus {
        match self.state.state.load(Ordering::Acquire) {
            STATE_QUALIFIED => CallbackSchedulingStatus::Qualified(EffectiveScheduling {
                policy: self.state.effective_policy.load(Ordering::Relaxed),
                priority: self.state.effective_priority.load(Ordering::Relaxed),
            }),
            STATE_FAILED => CallbackSchedulingStatus::Failed(self.failure()),
            STATE_UNSUPPORTED => CallbackSchedulingStatus::Unsupported,
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

    #[cfg(target_os = "linux")]
    fn publish_failure(&self, failure: CallbackSchedulingFailure) {
        match failure {
            CallbackSchedulingFailure::Set { error } => {
                self.state
                    .failure_kind
                    .store(FAILURE_SET, Ordering::Relaxed);
                self.state.failure_error.store(error, Ordering::Relaxed);
            }
            CallbackSchedulingFailure::Get { error } => {
                self.state
                    .failure_kind
                    .store(FAILURE_GET, Ordering::Relaxed);
                self.state.failure_error.store(error, Ordering::Relaxed);
            }
            CallbackSchedulingFailure::Effective(effective) => {
                self.state
                    .failure_kind
                    .store(FAILURE_EFFECTIVE, Ordering::Relaxed);
                self.state
                    .effective_policy
                    .store(effective.policy, Ordering::Relaxed);
                self.state
                    .effective_priority
                    .store(effective.priority, Ordering::Relaxed);
            }
        }
        self.state.state.store(STATE_FAILED, Ordering::Release);
    }

    fn failure(&self) -> CallbackSchedulingFailure {
        match self.state.failure_kind.load(Ordering::Relaxed) {
            FAILURE_SET => CallbackSchedulingFailure::Set {
                error: self.state.failure_error.load(Ordering::Relaxed),
            },
            FAILURE_GET => CallbackSchedulingFailure::Get {
                error: self.state.failure_error.load(Ordering::Relaxed),
            },
            FAILURE_EFFECTIVE => CallbackSchedulingFailure::Effective(EffectiveScheduling {
                policy: self.state.effective_policy.load(Ordering::Relaxed),
                priority: self.state.effective_priority.load(Ordering::Relaxed),
            }),
            _ => CallbackSchedulingFailure::Get { error: 0 },
        }
    }
}

pub(crate) fn callback_priority() -> i32 {
    if cfg!(feature = "hardware-orange-pi-zero-2w") {
        ORANGE_CALLBACK_PRIORITY
    } else {
        RASPBERRY_CALLBACK_PRIORITY
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn orange_worker_start_hook(_parity: usize) -> Result<(), ()> {
    #[cfg(target_os = "linux")]
    {
        imp::configure(ORANGE_WORKER_PRIORITY)
            .map(|_| ())
            .map_err(|_| ())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(())
    }
}

pub(crate) fn qualify_callback_scheduler(
    sink_label: &str,
    scheduler: &CallbackSchedulingHandle,
    timeout: Duration,
) -> Result<(), String> {
    let status = scheduler.wait_for_status(timeout);
    match status {
        CallbackSchedulingStatus::Qualified(effective) => {
            eprintln!(
                "{sink_label} audio callback scheduling qualified: policy=SCHED_FIFO priority={}",
                effective.priority
            );
            Ok(())
        }
        CallbackSchedulingStatus::Pending => Err(format!(
            "{sink_label} audio callback RT promotion not qualified: callback did not report within {} ms (requested policy=SCHED_FIFO priority={})",
            timeout.as_millis(),
            scheduler.requested_priority()
        )),
        CallbackSchedulingStatus::Failed(failure) => {
            Err(format_scheduling_failure(sink_label, scheduler.requested_priority(), failure))
        }
        CallbackSchedulingStatus::Unsupported => Err(format!(
            "{sink_label} audio callback RT promotion not qualified: pthread scheduling is unsupported on this platform"
        )),
    }
}

fn format_scheduling_failure(
    sink_label: &str,
    requested_priority: i32,
    failure: CallbackSchedulingFailure,
) -> String {
    let detail = match failure {
        CallbackSchedulingFailure::Set { error } => {
            format!("pthread_setschedparam returned error {error}")
        }
        CallbackSchedulingFailure::Get { error } => {
            format!("pthread_getschedparam returned error {error}")
        }
        CallbackSchedulingFailure::Effective(effective) => format!(
            "effective policy={} priority={} did not match SCHED_FIFO priority={requested_priority}",
            effective.policy, effective.priority
        ),
    };
    format!("{sink_label} audio callback RT promotion not qualified: {detail}")
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{CallbackSchedulingFailure, EffectiveScheduling};

    pub(super) fn configure(
        requested_priority: i32,
    ) -> Result<EffectiveScheduling, CallbackSchedulingFailure> {
        let params = libc::sched_param {
            sched_priority: requested_priority,
        };
        let set_result =
            unsafe { libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &params) };
        let set_result = if set_result == 0 {
            Ok(())
        } else {
            Err(set_result)
        };
        let effective_result = if set_result.is_ok() {
            let mut policy = 0;
            let mut params = libc::sched_param { sched_priority: 0 };
            let get_result = unsafe {
                libc::pthread_getschedparam(libc::pthread_self(), &mut policy, &mut params)
            };
            if get_result == 0 {
                Ok(EffectiveScheduling {
                    policy,
                    priority: params.sched_priority,
                })
            } else {
                Err(get_result)
            }
        } else {
            Err(0)
        };
        classify_results(requested_priority, set_result, effective_result)
    }

    fn classify_results(
        requested_priority: i32,
        set_result: Result<(), i32>,
        effective_result: Result<EffectiveScheduling, i32>,
    ) -> Result<EffectiveScheduling, CallbackSchedulingFailure> {
        if let Err(error) = set_result {
            return Err(CallbackSchedulingFailure::Set { error });
        }
        let effective =
            effective_result.map_err(|error| CallbackSchedulingFailure::Get { error })?;
        verify_effective(requested_priority, effective.policy, effective.priority)
    }

    fn verify_effective(
        requested_priority: i32,
        policy: i32,
        priority: i32,
    ) -> Result<EffectiveScheduling, CallbackSchedulingFailure> {
        let effective = EffectiveScheduling { policy, priority };
        if policy == libc::SCHED_FIFO && priority == requested_priority {
            Ok(effective)
        } else {
            Err(CallbackSchedulingFailure::Effective(effective))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn exact_fifo_priority_is_qualified_only_after_effective_verification() {
            assert_eq!(
                verify_effective(70, libc::SCHED_FIFO, 70),
                Ok(EffectiveScheduling {
                    policy: libc::SCHED_FIFO,
                    priority: 70,
                })
            );
            assert_eq!(
                verify_effective(70, libc::SCHED_FIFO, 69),
                Err(CallbackSchedulingFailure::Effective(EffectiveScheduling {
                    policy: libc::SCHED_FIFO,
                    priority: 69,
                }))
            );
        }

        #[test]
        fn non_fifo_policy_is_not_qualified() {
            assert!(matches!(
                verify_effective(70, libc::SCHED_FIFO - 1, 70),
                Err(CallbackSchedulingFailure::Effective(EffectiveScheduling {
                    priority: 70,
                    ..
                }))
            ));
        }

        #[test]
        fn set_failure_is_not_qualified_even_if_effective_values_look_right() {
            assert_eq!(
                classify_results(
                    70,
                    Err(libc::EPERM),
                    Ok(EffectiveScheduling {
                        policy: libc::SCHED_FIFO,
                        priority: 70,
                    })
                ),
                Err(CallbackSchedulingFailure::Set { error: libc::EPERM })
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orange_workers_precede_callbacks_with_fixed_qualification_priorities() {
        assert_eq!(ORANGE_WORKER_PRIORITY, 70);
        assert_eq!(ORANGE_CALLBACK_PRIORITY, 69);
        const {
            assert!(ORANGE_CALLBACK_PRIORITY < ORANGE_WORKER_PRIORITY);
            assert!(ORANGE_WORKER_PRIORITY <= 70);
        }
    }

    #[test]
    fn callback_priority_is_fixed_for_each_hardware_profile() {
        assert_eq!(RASPBERRY_CALLBACK_PRIORITY, 70);
        assert_eq!(
            callback_priority(),
            if cfg!(feature = "hardware-orange-pi-zero-2w") {
                ORANGE_CALLBACK_PRIORITY
            } else {
                RASPBERRY_CALLBACK_PRIORITY
            }
        );
    }

    #[test]
    fn scheduler_starts_pending() {
        let scheduler = CallbackSchedulingHandle::new(70);

        assert_eq!(scheduler.status(), CallbackSchedulingStatus::Pending);
        assert_eq!(scheduler.requested_priority(), 70);
    }

    #[test]
    fn startup_timeout_is_not_a_qualification() {
        let scheduler = CallbackSchedulingHandle::new(70);

        let error = qualify_callback_scheduler("DAC", &scheduler, Duration::ZERO).unwrap_err();

        assert!(error.contains("DAC"));
        assert!(error.contains("not qualified"));
    }
}
