use super::*;

#[test]
fn orange_roles_use_cpu_one_for_jack_and_cpu_two_three_for_workers() {
    assert_eq!(ORANGE_JACK_CPU, 1);
    assert_eq!(ORANGE_WORKER_CPUS, [2, 3]);
    assert_eq!(ORANGE_WORKER_PRIORITY, 70);
    assert_eq!(ORANGE_CALLBACK_PRIORITY, 70);
    assert_eq!(ORANGE_SECONDARY_CALLBACK_PRIORITY, 69);
    assert_eq!(RASPBERRY_CALLBACK_PRIORITY, 70);
}

#[test]
fn strict_setup_verifies_affinity_before_fifo() {
    let guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(1));
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    assert!(scheduler.configure_callback_thread());
    assert!(matches!(
        scheduler.status(),
        CallbackSchedulingStatus::Qualified(EffectiveScheduling { cpu: Some(1), .. })
    ));
    assert_eq!(
        guard.trace_for_cpu(1),
        vec![
            SchedulingSyscall::SetAffinity,
            SchedulingSyscall::GetAffinity,
            SchedulingSyscall::SetScheduling,
            SchedulingSyscall::GetScheduling,
        ]
    );
}

#[test]
fn worker_hook_uses_the_fixed_parity_cpu_map() {
    for (parity, cpu) in ORANGE_WORKER_CPUS.into_iter().enumerate() {
        let guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(cpu));
        assert!(orange_worker_start_hook(parity).is_ok());
        assert_eq!(
            guard.trace_for_cpu(cpu),
            vec![
                SchedulingSyscall::SetAffinity,
                SchedulingSyscall::GetAffinity,
                SchedulingSyscall::SetScheduling,
                SchedulingSyscall::GetScheduling,
            ]
        );
    }
}

#[test]
fn strict_failure_stores_fixed_observed_details() {
    let mut outcomes = InjectedSchedulingOutcomes::success();
    outcomes.target_cpu = Some(1);
    outcomes.observed_affinity = Some(CpuMask::single(1) | CpuMask::single(4));
    let _guard = install_test_scheduling(outcomes);
    let scheduler = CallbackSchedulingHandle::new_orange_jack();

    assert!(!scheduler.configure_callback_thread());
    assert!(matches!(
        scheduler.status(),
        CallbackSchedulingStatus::Failed(SchedulingFailure {
            stage: SchedulingFailureStage::AffinityMismatch,
            requested_cpu: 1,
            requested_priority: 70,
            observed_mask,
            ..
        }) if observed_mask == (CpuMask::single(1) | CpuMask::single(4))
    ));
}

#[test]
fn strict_timeout_is_terminal_and_does_not_retry() {
    let _guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(1));
    let scheduler = CallbackSchedulingHandle::new_orange_jack();

    let error = qualify_callback_scheduler("Jack", &scheduler, Duration::ZERO).unwrap_err();
    assert!(error.contains("stage=timeout"));
    assert!(matches!(
        scheduler.status(),
        CallbackSchedulingStatus::TimedOut
    ));
    assert!(!scheduler.configure_callback_thread());
}

#[test]
fn strict_failure_detail_is_formatted_after_callback_setup() {
    let mut outcomes = InjectedSchedulingOutcomes::success();
    outcomes.target_cpu = Some(1);
    outcomes.observed_affinity = Some(CpuMask::single(1) | CpuMask::single(4));
    let _guard = install_test_scheduling(outcomes);
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    assert!(!scheduler.configure_callback_thread());

    assert_eq!(
        qualify_callback_scheduler("Jack", &scheduler, Duration::from_millis(250)),
        Err("Jack audio callback RT placement not qualified: stage=affinity_mismatch errno=0 requested_cpu=1 requested_policy=SCHED_FIFO requested_priority=70 observed_mask=0x12 observed_policy=0 observed_priority=0".into())
    );
}

#[test]
fn legacy_handles_keep_two_operation_promotion() {
    let guard = install_test_scheduling(InjectedSchedulingOutcomes::success());
    let scheduler = CallbackSchedulingHandle::new(69);
    assert!(scheduler.configure_callback_thread());
    assert_eq!(
        guard.trace_for_cpu(usize::MAX),
        vec![
            SchedulingSyscall::SetScheduling,
            SchedulingSyscall::GetScheduling,
        ]
    );
}

#[test]
fn strict_syscall_failures_keep_their_stage_and_observations() {
    let cases = [
        (
            InjectedSchedulingOutcomes {
                affinity_set_errno: Some(11),
                ..InjectedSchedulingOutcomes::success_for_cpu(1)
            },
            SchedulingFailureStage::AffinitySet,
            11,
            vec![SchedulingSyscall::SetAffinity],
        ),
        (
            InjectedSchedulingOutcomes {
                affinity_get_errno: Some(12),
                ..InjectedSchedulingOutcomes::success_for_cpu(1)
            },
            SchedulingFailureStage::AffinityGet,
            12,
            vec![
                SchedulingSyscall::SetAffinity,
                SchedulingSyscall::GetAffinity,
            ],
        ),
        (
            InjectedSchedulingOutcomes {
                observed_affinity: Some(CpuMask::single(1) | CpuMask::single(4)),
                ..InjectedSchedulingOutcomes::success_for_cpu(1)
            },
            SchedulingFailureStage::AffinityMismatch,
            0,
            vec![
                SchedulingSyscall::SetAffinity,
                SchedulingSyscall::GetAffinity,
            ],
        ),
        (
            InjectedSchedulingOutcomes {
                observed_affinity: Some(CpuMask::single(1)),
                scheduling_set_errno: Some(13),
                ..InjectedSchedulingOutcomes::success_for_cpu(1)
            },
            SchedulingFailureStage::SchedulingSet,
            13,
            vec![
                SchedulingSyscall::SetAffinity,
                SchedulingSyscall::GetAffinity,
                SchedulingSyscall::SetScheduling,
            ],
        ),
        (
            InjectedSchedulingOutcomes {
                observed_affinity: Some(CpuMask::single(1)),
                scheduling_get_errno: Some(14),
                ..InjectedSchedulingOutcomes::success_for_cpu(1)
            },
            SchedulingFailureStage::SchedulingGet,
            14,
            vec![
                SchedulingSyscall::SetAffinity,
                SchedulingSyscall::GetAffinity,
                SchedulingSyscall::SetScheduling,
                SchedulingSyscall::GetScheduling,
            ],
        ),
        (
            InjectedSchedulingOutcomes {
                observed_scheduling: Some(EffectiveScheduling {
                    policy: SCHED_FIFO_POLICY + 1,
                    priority: 69,
                    cpu: Some(1),
                }),
                ..InjectedSchedulingOutcomes::success_for_cpu(1)
            },
            SchedulingFailureStage::SchedulingMismatch,
            0,
            vec![
                SchedulingSyscall::SetAffinity,
                SchedulingSyscall::GetAffinity,
                SchedulingSyscall::SetScheduling,
                SchedulingSyscall::GetScheduling,
            ],
        ),
    ];
    for (outcomes, stage, errno, trace) in cases {
        let guard = install_test_scheduling(outcomes);
        let scheduler = CallbackSchedulingHandle::new_orange_jack();
        assert!(!scheduler.configure_callback_thread());
        assert_eq!(guard.trace_for_cpu(1), trace);
        assert!(matches!(
            scheduler.status(),
            CallbackSchedulingStatus::Failed(SchedulingFailure {
                stage: actual_stage,
                errno: actual_errno,
                requested_cpu: 1,
                requested_priority: 70,
                ..
            }) if actual_stage == stage && actual_errno == errno
        ));
    }
}
