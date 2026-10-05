use playback_runtime::{NativeRunner, RuntimeOperation, RuntimeStoreResult};
use std::time::{Duration, Instant};

pub(super) const BASELINE: Duration = Duration::from_secs(5);
pub(super) const PLATEAU: Duration = Duration::from_millis(500);
pub(super) const RAPID: Duration = Duration::from_secs(3);
pub(super) const SAVE_COMPLETION_TIMEOUT: Duration = Duration::from_secs(18);
pub(super) const TURN_INTERVAL: Duration = Duration::from_millis(16);

pub(super) struct AutoAuxSequence {
    pub(super) phase: Phase,
    pub(super) aux_turns: u32,
    pub(super) rapid_turns: u32,
    pub(super) missed_turns: u32,
    pub(super) final_revision: Option<u64>,
    pub(super) save_completion: Option<(String, Duration)>,
}

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Baseline {
        started: Instant,
    },
    PlateauOne {
        until: Instant,
    },
    PlateauTwo {
        until: Instant,
    },
    Rapid {
        started: Instant,
        next_turn: Instant,
        delta: i8,
    },
    AwaitSave {
        started: Instant,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AutoAuxAction {
    None,
    Turn(i8),
    Completed,
}

impl AutoAuxSequence {
    pub(super) fn new(started: Instant) -> Self {
        Self {
            phase: Phase::Baseline { started },
            aux_turns: 0,
            rapid_turns: 0,
            missed_turns: 0,
            final_revision: None,
            save_completion: None,
        }
    }

    pub(super) fn next_action(
        &mut self,
        now: Instant,
        runner: &NativeRunner,
    ) -> Result<AutoAuxAction, String> {
        match self.phase {
            Phase::Baseline { started } if now.duration_since(started) >= BASELINE => {
                Ok(AutoAuxAction::Turn(1))
            }
            Phase::PlateauOne { until } if now >= until => Ok(AutoAuxAction::Turn(1)),
            Phase::PlateauTwo { until } if now >= until => {
                self.phase = Phase::Rapid {
                    started: now,
                    next_turn: now + TURN_INTERVAL,
                    delta: -1,
                };
                Ok(AutoAuxAction::None)
            }
            Phase::Rapid { started, .. } if now.duration_since(started) >= RAPID => {
                let expected = (RAPID.as_millis() / TURN_INTERVAL.as_millis()) as u32;
                self.missed_turns = expected.saturating_sub(self.rapid_turns);
                self.phase = Phase::AwaitSave { started: now };
                Ok(AutoAuxAction::None)
            }
            Phase::Rapid {
                next_turn, delta, ..
            } if now >= next_turn => Ok(AutoAuxAction::Turn(delta)),
            Phase::AwaitSave { started } => {
                self.refresh_final_revision(now, runner);
                if self
                    .save_completion
                    .as_ref()
                    .is_some_and(|(_, elapsed)| *elapsed <= now.duration_since(started))
                {
                    self.phase = Phase::Done;
                    return Ok(AutoAuxAction::Completed);
                }
                if now.duration_since(started) >= SAVE_COMPLETION_TIMEOUT {
                    return Err("Aux timing smoke timed out waiting for the final automatic default save completion".into());
                }
                Ok(AutoAuxAction::None)
            }
            Phase::Done => Ok(AutoAuxAction::None),
            _ => Ok(AutoAuxAction::None),
        }
    }

    pub(super) fn refresh_final_revision(&mut self, now: Instant, runner: &NativeRunner) {
        if self.final_revision.is_some() || !matches!(self.phase, Phase::AwaitSave { .. }) {
            return;
        }
        self.final_revision = runner
            .persistence_intent_at(now)
            .filter(|intent| intent.default_eligible())
            .map(|intent| intent.revision());
    }

    pub(super) fn turn_issued(&mut self, now: Instant) {
        match self.phase {
            Phase::Baseline { .. } => {
                self.aux_turns = self.aux_turns.saturating_add(1);
                self.phase = Phase::PlateauOne {
                    until: now + PLATEAU,
                };
            }
            Phase::PlateauOne { .. } => {
                self.aux_turns = self.aux_turns.saturating_add(1);
                self.phase = Phase::PlateauTwo {
                    until: now + PLATEAU,
                };
            }
            Phase::Rapid { started, delta, .. } => {
                self.aux_turns = self.aux_turns.saturating_add(1);
                self.rapid_turns = self.rapid_turns.saturating_add(1);
                self.phase = Phase::Rapid {
                    started,
                    next_turn: now + TURN_INTERVAL,
                    delta: -delta,
                };
            }
            _ => {}
        }
    }

    pub(super) fn accept_store_result(
        &mut self,
        result: &RuntimeStoreResult,
        accepted_at: Instant,
    ) -> Result<(), String> {
        let Some(expected_revision) = self.final_revision else {
            return Ok(());
        };
        let RuntimeStoreResult::Identified {
            result,
            request_id,
            revision: Some(revision),
        } = result
        else {
            return Ok(());
        };
        if request_id.is_empty() || *revision != expected_revision {
            return Ok(());
        }
        match result.as_ref() {
            RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            } => {
                let Phase::AwaitSave { started } = self.phase else {
                    return Ok(());
                };
                self.save_completion = Some((
                    request_id.clone(),
                    accepted_at.saturating_duration_since(started),
                ));
                Ok(())
            }
            RuntimeStoreResult::SaveDefaultResult {
                ok: false,
                is_auto: Some(true),
            }
            | RuntimeStoreResult::RuntimeFailure { .. }
                if result.operation() == RuntimeOperation::StoreSaveDefault =>
            {
                Err(format!(
                    "Aux timing smoke final automatic default save failed for revision {expected_revision} ({request_id})"
                ))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;
