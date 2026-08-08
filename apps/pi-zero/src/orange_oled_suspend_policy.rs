#![cfg_attr(not(unix), allow(dead_code))]

use crate::render::OledOwnershipStage;
use std::time::{Duration, Instant};

pub(crate) const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransactionAction {
    PrepareRelease,
    PrepareCommit,
    ResumeRelease,
    ResumeComplete,
    Rollback,
}

impl TransactionAction {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "prepare/release" => Self::PrepareRelease,
            "prepare/commit" => Self::PrepareCommit,
            "resume/release" => Self::ResumeRelease,
            "resume/complete" => Self::ResumeComplete,
            "rollback" => Self::Rollback,
            _ => return None,
        })
    }

    fn stage(self) -> OledOwnershipStage {
        match self {
            Self::PrepareRelease => OledOwnershipStage::PrepareRelease,
            Self::PrepareCommit => OledOwnershipStage::PrepareCommit,
            Self::ResumeRelease => OledOwnershipStage::ResumeRelease,
            Self::ResumeComplete => OledOwnershipStage::ResumeComplete,
            Self::Rollback => OledOwnershipStage::Rollback,
        }
    }
}

pub(crate) trait RenderOwnershipControl {
    fn ownership_stage(&self, stage: OledOwnershipStage) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransactionPhase {
    Normal,
    PrepareReleased,
    Committed,
    ResumeReleased,
}

#[derive(Debug)]
pub(crate) struct TransactionPolicy {
    phase: TransactionPhase,
    token: Option<String>,
    completed_token: Option<String>,
    settled_token: Option<String>,
    watchdog_deadline: Option<Instant>,
    recovering: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessOutcome {
    pub(crate) mutated: bool,
}

impl Default for TransactionPolicy {
    fn default() -> Self {
        Self {
            phase: TransactionPhase::Normal,
            token: None,
            completed_token: None,
            settled_token: None,
            watchdog_deadline: None,
            recovering: false,
        }
    }
}

impl TransactionPolicy {
    pub(crate) fn process<C: RenderOwnershipControl>(
        &mut self,
        action: TransactionAction,
        token: &str,
        now: Instant,
        control: &C,
    ) -> Result<ProcessOutcome, String> {
        if self.is_same_token_replay(action, token) {
            return Ok(ProcessOutcome { mutated: false });
        }
        match action {
            TransactionAction::Rollback => {
                if self.phase == TransactionPhase::Normal
                    && (self.settled_token.as_deref() == Some(token)
                        || self.completed_token.as_deref() == Some(token))
                {
                    return Ok(ProcessOutcome { mutated: false });
                }
                if self.phase == TransactionPhase::Normal || self.token.as_deref() != Some(token) {
                    return Err("OLED suspend rollback token is stale or conflicting".into());
                }
                if let Some(error) = self.rollback(now, control) {
                    return Err(error);
                }
                Ok(ProcessOutcome { mutated: true })
            }
            TransactionAction::PrepareRelease => {
                if self.phase != TransactionPhase::Normal
                    || self.completed_token.as_deref() == Some(token)
                    || self.settled_token.as_deref() == Some(token)
                {
                    return Err("OLED suspend prepare token is stale or conflicting".into());
                }
                self.begin(TransactionPhase::PrepareReleased, token, now);
                self.apply_stage(action, now, control)
            }
            TransactionAction::PrepareCommit => {
                self.require(TransactionPhase::PrepareReleased, token)?;
                self.apply_stage(action, now, control)?;
                self.phase = TransactionPhase::Committed;
                self.recovering = false;
                self.arm_watchdog(now);
                Ok(ProcessOutcome { mutated: true })
            }
            TransactionAction::ResumeRelease => {
                self.require(TransactionPhase::Committed, token)?;
                self.begin(TransactionPhase::ResumeReleased, token, now);
                self.apply_stage(action, now, control)
            }
            TransactionAction::ResumeComplete => {
                self.require(TransactionPhase::ResumeReleased, token)?;
                self.apply_stage(action, now, control)?;
                self.phase = TransactionPhase::Normal;
                self.completed_token = Some(token.to_string());
                self.token = None;
                self.settled_token = None;
                self.watchdog_deadline = None;
                self.recovering = false;
                Ok(ProcessOutcome { mutated: true })
            }
        }
    }

    pub(crate) fn rollback_if_due<C: RenderOwnershipControl>(
        &mut self,
        now: Instant,
        control: &C,
    ) -> Option<String> {
        if self.phase == TransactionPhase::Normal
            || self.watchdog_deadline.is_none_or(|deadline| now < deadline)
        {
            return None;
        }
        self.rollback(now, control)
    }

    pub(crate) fn rollback_after_response_failure<C: RenderOwnershipControl>(
        &mut self,
        outcome: ProcessOutcome,
        now: Instant,
        control: &C,
    ) -> Option<String> {
        if !outcome.mutated || self.phase == TransactionPhase::Normal {
            return None;
        }
        self.arm_watchdog(now);
        self.rollback(now, control)
    }

    pub(crate) fn rollback_now<C: RenderOwnershipControl>(
        &mut self,
        control: &C,
    ) -> Option<String> {
        if self.phase == TransactionPhase::Normal {
            return None;
        }
        self.rollback(Instant::now(), control)
    }

    #[cfg(test)]
    fn phase(&self) -> TransactionPhase {
        self.phase
    }

    #[cfg(test)]
    fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    #[cfg(test)]
    fn watchdog_deadline(&self) -> Option<Instant> {
        self.watchdog_deadline
    }

    fn is_same_token_replay(&self, action: TransactionAction, token: &str) -> bool {
        if self.recovering {
            return false;
        }
        if action == TransactionAction::ResumeComplete
            && self.phase == TransactionPhase::Normal
            && self.completed_token.as_deref() == Some(token)
        {
            return true;
        }
        if self.token.as_deref() != Some(token) {
            return false;
        }
        match action {
            TransactionAction::PrepareRelease => self.phase == TransactionPhase::PrepareReleased,
            TransactionAction::PrepareCommit => self.phase == TransactionPhase::Committed,
            TransactionAction::ResumeRelease => self.phase == TransactionPhase::ResumeReleased,
            TransactionAction::ResumeComplete => false,
            TransactionAction::Rollback => false,
        }
    }

    fn require(&self, phase: TransactionPhase, token: &str) -> Result<(), String> {
        if self.phase != phase || self.token.as_deref() != Some(token) || self.recovering {
            return Err("OLED suspend token is stale, conflicting, or out of order".into());
        }
        Ok(())
    }

    fn begin(&mut self, phase: TransactionPhase, token: &str, now: Instant) {
        self.phase = phase;
        self.token = Some(token.to_string());
        self.settled_token = None;
        self.recovering = false;
        self.arm_watchdog(now);
    }

    fn arm_watchdog(&mut self, now: Instant) {
        self.watchdog_deadline = Some(now + TRANSACTION_TIMEOUT);
    }

    fn apply_stage<C: RenderOwnershipControl>(
        &mut self,
        action: TransactionAction,
        now: Instant,
        control: &C,
    ) -> Result<ProcessOutcome, String> {
        match control.ownership_stage(action.stage()) {
            Ok(()) => Ok(ProcessOutcome { mutated: true }),
            Err(error) => {
                self.recovering = true;
                if let Some(rollback_error) = self.rollback(now, control) {
                    Err(format!("{error}; rollback failed: {rollback_error}"))
                } else {
                    Err(error)
                }
            }
        }
    }

    fn rollback<C: RenderOwnershipControl>(&mut self, now: Instant, control: &C) -> Option<String> {
        match control.ownership_stage(OledOwnershipStage::Rollback) {
            Ok(()) => {
                self.phase = TransactionPhase::Normal;
                self.settled_token = self.token.clone();
                self.token = None;
                self.watchdog_deadline = None;
                self.recovering = false;
                None
            }
            Err(error) => {
                self.recovering = true;
                self.arm_watchdog(now);
                Some(error)
            }
        }
    }
}

#[cfg(test)]
#[path = "orange_oled_suspend_policy_tests.rs"]
mod tests;
