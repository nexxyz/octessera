use super::*;
use std::cell::RefCell;

struct FakeControl {
    stages: RefCell<Vec<OledOwnershipStage>>,
    failures: RefCell<Vec<OledOwnershipStage>>,
}

impl FakeControl {
    fn new() -> Self {
        Self {
            stages: RefCell::new(Vec::new()),
            failures: RefCell::new(Vec::new()),
        }
    }

    fn fail_once(&self, stage: OledOwnershipStage) {
        self.failures.borrow_mut().push(stage);
    }
}

impl RenderOwnershipControl for FakeControl {
    fn ownership_stage(&self, stage: OledOwnershipStage) -> Result<(), String> {
        self.stages.borrow_mut().push(stage);
        let failure_index = self
            .failures
            .borrow()
            .iter()
            .position(|value| *value == stage);
        if let Some(index) = failure_index {
            self.failures.borrow_mut().remove(index);
            Err(format!("injected {stage:?} failure"))
        } else {
            Ok(())
        }
    }
}

fn token() -> &'static str {
    "0123456789abcdef0123456789abcdef"
}

fn second_token() -> &'static str {
    "abcdefabcdefabcdefabcdefabcdefab"
}

fn start_committed(policy: &mut TransactionPolicy, control: &FakeControl, now: Instant) {
    policy
        .process(TransactionAction::PrepareRelease, token(), now, control)
        .unwrap();
    policy
        .process(TransactionAction::PrepareCommit, token(), now, control)
        .unwrap();
}

#[test]
fn same_token_replays_are_idempotent_but_conflicts_are_rejected() {
    let now = Instant::now();
    let control = FakeControl::new();
    let mut policy = TransactionPolicy::default();
    assert!(
        policy
            .process(TransactionAction::PrepareRelease, token(), now, &control)
            .unwrap()
            .mutated
    );
    assert!(
        !policy
            .process(TransactionAction::PrepareRelease, token(), now, &control)
            .unwrap()
            .mutated
    );
    assert!(policy
        .process(
            TransactionAction::PrepareRelease,
            second_token(),
            now,
            &control
        )
        .is_err());
    assert!(policy
        .process(TransactionAction::ResumeRelease, token(), now, &control)
        .is_err());
}

#[test]
fn committed_state_keeps_a_bounded_monotonic_watchdog() {
    let now = Instant::now();
    let control = FakeControl::new();
    let mut policy = TransactionPolicy::default();
    start_committed(&mut policy, &control, now);
    assert_eq!(policy.phase(), TransactionPhase::Committed);
    assert_eq!(policy.token(), Some(token()));
    assert_eq!(policy.watchdog_deadline(), Some(now + TRANSACTION_TIMEOUT));
    assert!(policy
        .rollback_if_due(now + TRANSACTION_TIMEOUT, &control)
        .is_none());
    assert_eq!(policy.phase(), TransactionPhase::Normal);
}

#[test]
fn watchdog_rolls_back_released_and_committed_states() {
    for committed in [false, true] {
        let now = Instant::now();
        let control = FakeControl::new();
        let mut policy = TransactionPolicy::default();
        if committed {
            start_committed(&mut policy, &control, now);
        } else {
            policy
                .process(TransactionAction::PrepareRelease, token(), now, &control)
                .unwrap();
        }
        assert!(policy
            .rollback_if_due(now + TRANSACTION_TIMEOUT, &control)
            .is_none());
        assert_eq!(policy.phase(), TransactionPhase::Normal);
    }
}

#[test]
fn response_failure_rolls_back_only_the_mutating_request() {
    let now = Instant::now();
    let control = FakeControl::new();
    let mut policy = TransactionPolicy::default();
    let outcome = policy
        .process(TransactionAction::PrepareRelease, token(), now, &control)
        .unwrap();
    assert!(policy
        .rollback_after_response_failure(outcome, now, &control)
        .is_none());
    assert_eq!(policy.phase(), TransactionPhase::Normal);

    policy
        .process(
            TransactionAction::PrepareRelease,
            second_token(),
            now,
            &control,
        )
        .unwrap();
    policy
        .process(
            TransactionAction::PrepareCommit,
            second_token(),
            now,
            &control,
        )
        .unwrap();
    let replay = policy
        .process(
            TransactionAction::PrepareCommit,
            second_token(),
            now,
            &control,
        )
        .unwrap();
    assert!(!replay.mutated);
    assert!(policy
        .rollback_after_response_failure(replay, now, &control)
        .is_none());
    assert_eq!(policy.phase(), TransactionPhase::Committed);
}

#[test]
fn response_failure_after_prepare_commit_rolls_committed_state_back() {
    let now = Instant::now();
    let control = FakeControl::new();
    let mut policy = TransactionPolicy::default();
    policy
        .process(TransactionAction::PrepareRelease, token(), now, &control)
        .unwrap();
    let outcome = policy
        .process(TransactionAction::PrepareCommit, token(), now, &control)
        .unwrap();
    assert_eq!(policy.phase(), TransactionPhase::Committed);
    assert!(policy
        .rollback_after_response_failure(outcome, now, &control)
        .is_none());
    assert_eq!(policy.phase(), TransactionPhase::Normal);
    assert_eq!(
        control.stages.borrow().last(),
        Some(&OledOwnershipStage::Rollback)
    );
}

#[test]
fn stage_failure_keeps_watchdog_and_retries_rollback() {
    let now = Instant::now();
    let control = FakeControl::new();
    control.fail_once(OledOwnershipStage::PrepareCommit);
    control.fail_once(OledOwnershipStage::Rollback);
    let mut policy = TransactionPolicy::default();
    policy
        .process(TransactionAction::PrepareRelease, token(), now, &control)
        .unwrap();
    assert!(policy
        .process(TransactionAction::PrepareCommit, token(), now, &control)
        .is_err());
    assert!(policy.watchdog_deadline().is_some());
    assert!(policy
        .rollback_if_due(now + TRANSACTION_TIMEOUT, &control)
        .is_none());
    assert_eq!(policy.phase(), TransactionPhase::Normal);
}

#[test]
fn failed_watchdog_rollback_rearms_a_future_bounded_deadline() {
    let now = Instant::now();
    let due = now + TRANSACTION_TIMEOUT;
    let control = FakeControl::new();
    control.fail_once(OledOwnershipStage::Rollback);
    let mut policy = TransactionPolicy::default();
    policy
        .process(TransactionAction::PrepareRelease, token(), now, &control)
        .unwrap();
    assert!(policy.rollback_if_due(due, &control).is_some());
    assert_eq!(policy.watchdog_deadline(), Some(due + TRANSACTION_TIMEOUT));
    assert!(policy
        .rollback_if_due(due + TRANSACTION_TIMEOUT, &control)
        .is_none());
    assert_eq!(policy.phase(), TransactionPhase::Normal);
}

#[test]
fn explicit_rollback_is_token_correlated_and_idempotent() {
    let now = Instant::now();
    let control = FakeControl::new();
    let mut policy = TransactionPolicy::default();
    policy
        .process(TransactionAction::PrepareRelease, token(), now, &control)
        .unwrap();
    assert!(policy
        .process(TransactionAction::Rollback, second_token(), now, &control)
        .is_err());
    assert!(
        policy
            .process(TransactionAction::Rollback, token(), now, &control)
            .unwrap()
            .mutated
    );
    assert!(
        !policy
            .process(TransactionAction::Rollback, token(), now, &control)
            .unwrap()
            .mutated
    );
}

#[test]
fn transition_order_is_owned_by_the_policy() {
    let now = Instant::now();
    let control = FakeControl::new();
    let mut policy = TransactionPolicy::default();
    start_committed(&mut policy, &control, now);
    policy
        .process(TransactionAction::ResumeRelease, token(), now, &control)
        .unwrap();
    policy
        .process(TransactionAction::ResumeComplete, token(), now, &control)
        .unwrap();
    assert_eq!(
        *control.stages.borrow(),
        vec![
            OledOwnershipStage::PrepareRelease,
            OledOwnershipStage::PrepareCommit,
            OledOwnershipStage::ResumeRelease,
            OledOwnershipStage::ResumeComplete,
        ]
    );
}
