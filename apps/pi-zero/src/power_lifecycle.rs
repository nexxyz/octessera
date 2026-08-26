#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PowerAction {
    Reboot,
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PowerLifecyclePhase {
    RecoverySave,
    Safety,
    TerminalAck,
    PowerSubmit,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PowerLifecycleFailure {
    pub(crate) action: PowerAction,
    pub(crate) phase: PowerLifecyclePhase,
    pub(crate) message: String,
    pub(crate) accepted: bool,
}

impl std::fmt::Display for PowerLifecycleFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "ordinary {:?} failed during {:?}: {}",
            self.action, self.phase, self.message
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PowerLifecycleResult {
    Submitted,
    Failed(PowerLifecycleFailure),
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PowerLifecycleState {
    Ready,
    Accepted(PowerAction),
    SafetyComplete(PowerAction),
    TerminalAcknowledged(PowerAction),
    Submitted(PowerAction),
    Failed {
        action: PowerAction,
        phase: PowerLifecyclePhase,
        accepted: bool,
    },
}

pub(crate) trait PowerLifecycleCallbacks {
    fn save_recovery(&mut self) -> Result<(), String>;
    fn panic_external_midi(&mut self) -> Result<(), String>;
    fn silence_internal_audio(&mut self) -> Result<(), String>;
    fn acknowledge_terminal(&mut self, action: PowerAction) -> Result<(), String>;
    fn submit_power(&mut self, action: PowerAction) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub(crate) struct PowerLifecycle {
    state: Option<PowerLifecycleState>,
}

impl PowerLifecycle {
    pub(crate) fn state(&self) -> PowerLifecycleState {
        self.state.unwrap_or(PowerLifecycleState::Ready)
    }

    pub(crate) fn blocks_nonterminal(&self) -> bool {
        matches!(
            self.state(),
            PowerLifecycleState::Accepted(_)
                | PowerLifecycleState::SafetyComplete(_)
                | PowerLifecycleState::TerminalAcknowledged(_)
                | PowerLifecycleState::Submitted(_)
                | PowerLifecycleState::Failed { accepted: true, .. }
        )
    }

    pub(crate) fn execute<C: PowerLifecycleCallbacks>(
        &mut self,
        action: PowerAction,
        callbacks: &mut C,
    ) -> PowerLifecycleResult {
        if self.blocks_nonterminal() {
            return PowerLifecycleResult::Duplicate;
        }

        if let Err(error) = callbacks.save_recovery() {
            return self.fail(action, PowerLifecyclePhase::RecoverySave, error, false);
        }

        self.state = Some(PowerLifecycleState::Accepted(action));
        let panic_error = callbacks.panic_external_midi().err();
        let silence_error = callbacks.silence_internal_audio().err();
        if let Some(error) = safety_error(panic_error, silence_error) {
            return self.fail(action, PowerLifecyclePhase::Safety, error, true);
        }

        self.state = Some(PowerLifecycleState::SafetyComplete(action));
        if let Err(error) = callbacks.acknowledge_terminal(action) {
            return self.fail(action, PowerLifecyclePhase::TerminalAck, error, true);
        }

        self.state = Some(PowerLifecycleState::TerminalAcknowledged(action));
        if let Err(error) = callbacks.submit_power(action) {
            return self.fail(action, PowerLifecyclePhase::PowerSubmit, error, true);
        }

        self.state = Some(PowerLifecycleState::Submitted(action));
        PowerLifecycleResult::Submitted
    }

    fn fail(
        &mut self,
        action: PowerAction,
        phase: PowerLifecyclePhase,
        message: String,
        accepted: bool,
    ) -> PowerLifecycleResult {
        self.state = Some(PowerLifecycleState::Failed {
            action,
            phase,
            accepted,
        });
        PowerLifecycleResult::Failed(PowerLifecycleFailure {
            action,
            phase,
            message,
            accepted,
        })
    }
}

fn safety_error(panic_error: Option<String>, silence_error: Option<String>) -> Option<String> {
    match (panic_error, silence_error) {
        (None, None) => None,
        (Some(panic), None) => Some(format!("external MIDI panic failed: {panic}")),
        (None, Some(silence)) => Some(format!("internal audio silence failed: {silence}")),
        (Some(panic), Some(silence)) => Some(format!(
            "external MIDI panic failed: {panic}; internal audio silence failed: {silence}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TraceCallbacks {
        events: Vec<&'static str>,
        save_error: Option<&'static str>,
        panic_error: Option<&'static str>,
        silence_error: Option<&'static str>,
        terminal_error: Option<&'static str>,
        submit_error: Option<&'static str>,
    }

    impl PowerLifecycleCallbacks for TraceCallbacks {
        fn save_recovery(&mut self) -> Result<(), String> {
            self.events.push("save");
            self.save_error.map_or(Ok(()), |error| Err(error.into()))
        }

        fn panic_external_midi(&mut self) -> Result<(), String> {
            self.events.push("midi-panic");
            self.panic_error.map_or(Ok(()), |error| Err(error.into()))
        }

        fn silence_internal_audio(&mut self) -> Result<(), String> {
            self.events.push("internal-silence");
            self.silence_error.map_or(Ok(()), |error| Err(error.into()))
        }

        fn acknowledge_terminal(&mut self, _action: PowerAction) -> Result<(), String> {
            self.events.push("terminal-ack");
            self.terminal_error
                .map_or(Ok(()), |error| Err(error.into()))
        }

        fn submit_power(&mut self, _action: PowerAction) -> Result<(), String> {
            self.events.push("power-submit");
            self.submit_error.map_or(Ok(()), |error| Err(error.into()))
        }
    }

    #[test]
    fn ordinary_reboot_and_shutdown_have_the_same_order() {
        for action in [PowerAction::Reboot, PowerAction::Shutdown] {
            let mut callbacks = TraceCallbacks::default();
            let mut lifecycle = PowerLifecycle::default();
            assert_eq!(
                lifecycle.execute(action, &mut callbacks),
                PowerLifecycleResult::Submitted
            );
            assert_eq!(
                callbacks.events,
                [
                    "save",
                    "midi-panic",
                    "internal-silence",
                    "terminal-ack",
                    "power-submit"
                ]
            );
        }
    }

    #[test]
    fn safety_attempts_both_operations_when_panic_fails() {
        let mut callbacks = TraceCallbacks {
            panic_error: Some("midi"),
            ..TraceCallbacks::default()
        };
        let mut lifecycle = PowerLifecycle::default();
        let result = lifecycle.execute(PowerAction::Reboot, &mut callbacks);
        assert!(matches!(
            result,
            PowerLifecycleResult::Failed(PowerLifecycleFailure {
                phase: PowerLifecyclePhase::Safety,
                accepted: true,
                ..
            })
        ));
        assert_eq!(callbacks.events, ["save", "midi-panic", "internal-silence"]);
        assert!(lifecycle.blocks_nonterminal());
    }

    #[test]
    fn every_failed_phase_prevents_later_phases() {
        for (phase, callbacks) in [
            (
                PowerLifecyclePhase::RecoverySave,
                TraceCallbacks {
                    save_error: Some("save"),
                    ..TraceCallbacks::default()
                },
            ),
            (
                PowerLifecyclePhase::Safety,
                TraceCallbacks {
                    silence_error: Some("silence"),
                    ..TraceCallbacks::default()
                },
            ),
            (
                PowerLifecyclePhase::TerminalAck,
                TraceCallbacks {
                    terminal_error: Some("terminal"),
                    ..TraceCallbacks::default()
                },
            ),
            (
                PowerLifecyclePhase::PowerSubmit,
                TraceCallbacks {
                    submit_error: Some("power"),
                    ..TraceCallbacks::default()
                },
            ),
        ] {
            let mut callbacks = callbacks;
            let mut lifecycle = PowerLifecycle::default();
            let result = lifecycle.execute(PowerAction::Shutdown, &mut callbacks);
            assert!(matches!(
                result,
                PowerLifecycleResult::Failed(PowerLifecycleFailure { phase: actual, .. }) if actual == phase
            ));
            match phase {
                PowerLifecyclePhase::RecoverySave => {
                    assert_eq!(callbacks.events, ["save"]);
                    assert!(!lifecycle.blocks_nonterminal());
                }
                PowerLifecyclePhase::Safety => {
                    assert_eq!(callbacks.events, ["save", "midi-panic", "internal-silence"]);
                    assert!(lifecycle.blocks_nonterminal());
                }
                PowerLifecyclePhase::TerminalAck => {
                    assert_eq!(
                        callbacks.events,
                        ["save", "midi-panic", "internal-silence", "terminal-ack"]
                    );
                    assert!(lifecycle.blocks_nonterminal());
                }
                PowerLifecyclePhase::PowerSubmit => {
                    assert_eq!(
                        callbacks.events,
                        [
                            "save",
                            "midi-panic",
                            "internal-silence",
                            "terminal-ack",
                            "power-submit"
                        ]
                    );
                    assert!(lifecycle.blocks_nonterminal());
                }
            }
        }
    }

    #[test]
    fn accepted_lifecycle_suppresses_duplicate_requests() {
        let mut callbacks = TraceCallbacks::default();
        let mut lifecycle = PowerLifecycle::default();
        assert_eq!(
            lifecycle.execute(PowerAction::Reboot, &mut callbacks),
            PowerLifecycleResult::Submitted
        );
        assert_eq!(
            lifecycle.execute(PowerAction::Shutdown, &mut callbacks),
            PowerLifecycleResult::Duplicate
        );
        assert_eq!(
            callbacks.events,
            [
                "save",
                "midi-panic",
                "internal-silence",
                "terminal-ack",
                "power-submit"
            ]
        );
    }
}
