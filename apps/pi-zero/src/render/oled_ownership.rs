#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
pub(crate) enum OledOwnershipStage {
    PrepareRelease,
    PrepareCommit,
    ResumeRelease,
    ResumeComplete,
    Rollback,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum OledOwnershipState {
    #[default]
    Normal,
    QuiescedAttached,
    PrepareHardwareDetachedLeaseAttached,
    PrepareReleased,
    QuiescedCommitted,
    CommittedHardwareDetachedLeaseAttached,
    CommittedReleased,
}

impl OledOwnershipState {
    pub(crate) fn is_quiesced(self) -> bool {
        self != Self::Normal
    }
}

pub(crate) trait OledRenderControl {
    fn clear_oled_retry(&mut self);
    fn detach_hardware(&mut self) -> Result<(), String>;
    fn detach_handoff(&mut self) -> Result<(), String>;
    fn reacquire_handoff(&mut self) -> Result<(), String>;
    fn reacquire_hardware(&mut self) -> Result<(), String>;
    fn force_latest_frame(&mut self) -> Result<(), String>;
}

pub(crate) fn handle_stage<C: OledRenderControl>(
    stage: OledOwnershipStage,
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    match stage {
        OledOwnershipStage::PrepareRelease => prepare_release(control, state),
        OledOwnershipStage::PrepareCommit => prepare_commit(control, state),
        OledOwnershipStage::ResumeRelease => resume_release(control, state),
        OledOwnershipStage::ResumeComplete => resume_complete(control, state),
        OledOwnershipStage::Rollback => restore(control, state),
    }
}

pub(crate) fn restore<C: OledRenderControl>(
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    match *state {
        OledOwnershipState::Normal => Ok(()),
        OledOwnershipState::QuiescedAttached => finish_restore(control, state),
        OledOwnershipState::QuiescedCommitted => finish_restore(control, state),
        OledOwnershipState::PrepareHardwareDetachedLeaseAttached => {
            control.reacquire_hardware()?;
            *state = OledOwnershipState::QuiescedAttached;
            finish_restore(control, state)
        }
        OledOwnershipState::PrepareReleased => {
            control.reacquire_handoff()?;
            *state = OledOwnershipState::PrepareHardwareDetachedLeaseAttached;
            control.reacquire_hardware()?;
            *state = OledOwnershipState::QuiescedAttached;
            finish_restore(control, state)
        }
        OledOwnershipState::CommittedHardwareDetachedLeaseAttached => {
            control.reacquire_hardware()?;
            *state = OledOwnershipState::QuiescedCommitted;
            finish_restore(control, state)
        }
        OledOwnershipState::CommittedReleased => {
            control.reacquire_handoff()?;
            *state = OledOwnershipState::CommittedHardwareDetachedLeaseAttached;
            control.reacquire_hardware()?;
            *state = OledOwnershipState::QuiescedCommitted;
            finish_restore(control, state)
        }
    }
}

pub(crate) fn restore_after_dropped_ack<C: OledRenderControl>(
    ack_dropped: bool,
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    if ack_dropped && state.is_quiesced() {
        restore(control, state)
    } else {
        Ok(())
    }
}

fn prepare_release<C: OledRenderControl>(
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    if *state != OledOwnershipState::Normal {
        return Err("OLED prepare release requires normal ownership".into());
    }
    control.clear_oled_retry();
    *state = OledOwnershipState::QuiescedAttached;
    control.detach_hardware()?;
    *state = OledOwnershipState::PrepareHardwareDetachedLeaseAttached;
    control.detach_handoff()?;
    *state = OledOwnershipState::PrepareReleased;
    Ok(())
}

fn prepare_commit<C: OledRenderControl>(
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    if *state != OledOwnershipState::PrepareReleased {
        return Err("OLED prepare commit requires released ownership".into());
    }
    control.reacquire_handoff()?;
    *state = OledOwnershipState::PrepareHardwareDetachedLeaseAttached;
    control.reacquire_hardware()?;
    *state = OledOwnershipState::QuiescedCommitted;
    Ok(())
}

fn resume_release<C: OledRenderControl>(
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    if *state != OledOwnershipState::QuiescedCommitted {
        return Err("OLED resume release requires committed ownership".into());
    }
    control.clear_oled_retry();
    control.detach_hardware()?;
    *state = OledOwnershipState::CommittedHardwareDetachedLeaseAttached;
    control.detach_handoff()?;
    *state = OledOwnershipState::CommittedReleased;
    Ok(())
}

fn resume_complete<C: OledRenderControl>(
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    if *state != OledOwnershipState::CommittedReleased {
        return Err("OLED resume complete requires released ownership".into());
    }
    control.reacquire_handoff()?;
    *state = OledOwnershipState::CommittedHardwareDetachedLeaseAttached;
    control.reacquire_hardware()?;
    *state = OledOwnershipState::QuiescedCommitted;
    finish_restore(control, state)
}

fn finish_restore<C: OledRenderControl>(
    control: &mut C,
    state: &mut OledOwnershipState,
) -> Result<(), String> {
    control.force_latest_frame()?;
    *state = OledOwnershipState::Normal;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Failure {
        DetachHardware,
        DetachHandoff,
        ReacquireHardware,
        ReacquireHandoff,
        ForceFrame,
    }

    struct FakeControl {
        failure: Option<Failure>,
        events: Vec<&'static str>,
        hardware_attached: bool,
        lease_attached: bool,
        forced_frames: u32,
    }

    impl FakeControl {
        fn new(failure: Option<Failure>) -> Self {
            Self {
                failure,
                events: Vec::new(),
                hardware_attached: true,
                lease_attached: true,
                forced_frames: 0,
            }
        }

        fn for_state(state: OledOwnershipState, failure: Option<Failure>) -> Self {
            let mut control = Self::new(failure);
            (control.hardware_attached, control.lease_attached) = match state {
                OledOwnershipState::Normal
                | OledOwnershipState::QuiescedAttached
                | OledOwnershipState::QuiescedCommitted => (true, true),
                OledOwnershipState::PrepareHardwareDetachedLeaseAttached
                | OledOwnershipState::CommittedHardwareDetachedLeaseAttached => (false, true),
                OledOwnershipState::PrepareReleased | OledOwnershipState::CommittedReleased => {
                    (false, false)
                }
            };
            control
        }

        fn step(&mut self, event: &'static str, failure: Failure) -> Result<(), String> {
            self.events.push(event);
            if self.failure == Some(failure) {
                Err(format!("injected {event} failure"))
            } else {
                Ok(())
            }
        }
    }

    impl OledRenderControl for FakeControl {
        fn clear_oled_retry(&mut self) {
            self.events.push("clear_retry");
        }

        fn detach_hardware(&mut self) -> Result<(), String> {
            if !self.hardware_attached {
                return Err("duplicate hardware detach".into());
            }
            self.step("detach_hardware", Failure::DetachHardware)?;
            self.hardware_attached = false;
            Ok(())
        }

        fn detach_handoff(&mut self) -> Result<(), String> {
            if !self.lease_attached {
                return Err("duplicate lease detach".into());
            }
            self.step("detach_handoff", Failure::DetachHandoff)?;
            self.lease_attached = false;
            Ok(())
        }

        fn reacquire_handoff(&mut self) -> Result<(), String> {
            if self.lease_attached {
                return Err("duplicate lease reacquire".into());
            }
            self.step("reacquire_handoff", Failure::ReacquireHandoff)?;
            self.lease_attached = true;
            Ok(())
        }

        fn reacquire_hardware(&mut self) -> Result<(), String> {
            if self.hardware_attached {
                return Err("duplicate hardware reacquire".into());
            }
            self.step("reacquire_hardware", Failure::ReacquireHardware)?;
            self.hardware_attached = true;
            Ok(())
        }

        fn force_latest_frame(&mut self) -> Result<(), String> {
            if !self.hardware_attached || !self.lease_attached {
                return Err("forced frame requires attached ownership".into());
            }
            self.step("force_frame", Failure::ForceFrame)?;
            self.forced_frames += 1;
            Ok(())
        }
    }

    fn assert_physical_state(control: &FakeControl, state: OledOwnershipState) {
        let expected = match state {
            OledOwnershipState::Normal
            | OledOwnershipState::QuiescedAttached
            | OledOwnershipState::QuiescedCommitted => (true, true),
            OledOwnershipState::PrepareHardwareDetachedLeaseAttached
            | OledOwnershipState::CommittedHardwareDetachedLeaseAttached => (false, true),
            OledOwnershipState::PrepareReleased | OledOwnershipState::CommittedReleased => {
                (false, false)
            }
        };
        assert_eq!(
            (control.hardware_attached, control.lease_attached),
            expected
        );
    }

    fn prepare_committed() -> (FakeControl, OledOwnershipState) {
        let mut control = FakeControl::new(None);
        let mut state = OledOwnershipState::default();
        handle_stage(OledOwnershipStage::PrepareRelease, &mut control, &mut state).unwrap();
        handle_stage(OledOwnershipStage::PrepareCommit, &mut control, &mut state).unwrap();
        assert_physical_state(&control, state);
        (control, state)
    }

    #[test]
    fn transitions_use_hardware_then_lease_and_reacquire_in_reverse_order() {
        let (mut control, mut state) = prepare_committed();
        assert_eq!(
            control.events,
            vec![
                "clear_retry",
                "detach_hardware",
                "detach_handoff",
                "reacquire_handoff",
                "reacquire_hardware",
            ]
        );
        control.events.clear();
        handle_stage(OledOwnershipStage::ResumeRelease, &mut control, &mut state).unwrap();
        handle_stage(OledOwnershipStage::ResumeComplete, &mut control, &mut state).unwrap();
        assert_physical_state(&control, state);
        assert_eq!(control.forced_frames, 1);
        assert_eq!(
            control.events,
            vec![
                "clear_retry",
                "detach_hardware",
                "detach_handoff",
                "reacquire_handoff",
                "reacquire_hardware",
                "force_frame",
            ]
        );
        assert_eq!(state, OledOwnershipState::Normal);
    }

    #[test]
    fn partial_detach_is_explicit_and_rollback_restores_once() {
        let mut control = FakeControl::new(Some(Failure::DetachHandoff));
        let mut state = OledOwnershipState::default();
        assert!(
            handle_stage(OledOwnershipStage::PrepareRelease, &mut control, &mut state).is_err()
        );
        assert_eq!(
            state,
            OledOwnershipState::PrepareHardwareDetachedLeaseAttached
        );
        control.failure = None;
        assert!(restore(&mut control, &mut state).is_ok());
        assert_eq!(state, OledOwnershipState::Normal);
        assert_physical_state(&control, state);
        assert_eq!(
            control.events,
            vec![
                "clear_retry",
                "detach_hardware",
                "detach_handoff",
                "reacquire_hardware",
                "force_frame"
            ]
        );
    }

    #[test]
    fn hardware_reacquire_failure_keeps_the_lease_attached_state() {
        let mut state = OledOwnershipState::PrepareReleased;
        let mut control = FakeControl::for_state(state, Some(Failure::ReacquireHardware));
        assert!(handle_stage(OledOwnershipStage::PrepareCommit, &mut control, &mut state).is_err());
        assert_eq!(
            state,
            OledOwnershipState::PrepareHardwareDetachedLeaseAttached
        );
        assert_physical_state(&control, state);
        control.failure = None;
        restore(&mut control, &mut state).unwrap();
        assert_eq!(state, OledOwnershipState::Normal);
        assert_eq!(
            control.events,
            vec![
                "reacquire_handoff",
                "reacquire_hardware",
                "reacquire_hardware",
                "force_frame"
            ]
        );
    }

    #[test]
    fn hardware_detach_and_lease_reacquire_failures_are_retryable() {
        let mut detach_control = FakeControl::new(Some(Failure::DetachHardware));
        let mut detach_state = OledOwnershipState::default();
        assert!(handle_stage(
            OledOwnershipStage::PrepareRelease,
            &mut detach_control,
            &mut detach_state
        )
        .is_err());
        assert_eq!(detach_state, OledOwnershipState::QuiescedAttached);
        assert_physical_state(&detach_control, detach_state);

        let mut reacquire_state = OledOwnershipState::PrepareReleased;
        let mut reacquire_control =
            FakeControl::for_state(reacquire_state, Some(Failure::ReacquireHandoff));
        assert!(handle_stage(
            OledOwnershipStage::PrepareCommit,
            &mut reacquire_control,
            &mut reacquire_state
        )
        .is_err());
        assert_eq!(reacquire_state, OledOwnershipState::PrepareReleased);
        assert_physical_state(&reacquire_control, reacquire_state);
        reacquire_control.failure = None;
        restore(&mut reacquire_control, &mut reacquire_state).unwrap();
        assert_eq!(reacquire_state, OledOwnershipState::Normal);
        assert_physical_state(&reacquire_control, reacquire_state);
    }

    #[test]
    fn fake_control_rejects_duplicate_resource_transitions() {
        let mut control = FakeControl::new(None);
        control.detach_hardware().unwrap();
        assert!(control.detach_hardware().is_err());
        control.detach_handoff().unwrap();
        assert!(control.detach_handoff().is_err());
        control.reacquire_handoff().unwrap();
        assert!(control.reacquire_handoff().is_err());
        control.reacquire_hardware().unwrap();
        assert!(control.reacquire_hardware().is_err());
    }

    #[test]
    fn forced_frame_failure_retains_quiesced_attached_state() {
        let mut state = OledOwnershipState::CommittedReleased;
        let mut control = FakeControl::for_state(state, Some(Failure::ForceFrame));
        assert!(
            handle_stage(OledOwnershipStage::ResumeComplete, &mut control, &mut state).is_err()
        );
        assert_eq!(state, OledOwnershipState::QuiescedCommitted);
        assert_physical_state(&control, state);
        control.failure = None;
        restore(&mut control, &mut state).unwrap();
        assert_eq!(state, OledOwnershipState::Normal);
        assert_physical_state(&control, state);
    }

    #[test]
    fn dropped_ack_restores_quiesced_ownership() {
        let mut control = FakeControl::new(None);
        let mut state = OledOwnershipState::QuiescedCommitted;
        let (ack, receiver) = std::sync::mpsc::channel::<Result<(), String>>();
        drop(receiver);
        let dropped = ack.send(Ok(()));
        restore_after_dropped_ack(dropped.is_err(), &mut control, &mut state).unwrap();
        assert_eq!(state, OledOwnershipState::Normal);
        assert_physical_state(&control, state);
        assert_eq!(control.events, vec!["force_frame"]);
    }

    #[test]
    fn started_command_cancelled_after_timeout_restores_after_dropped_ack() {
        let mut control = FakeControl::new(None);
        let mut state = OledOwnershipState::default();
        handle_stage(OledOwnershipStage::PrepareRelease, &mut control, &mut state).unwrap();
        assert_eq!(state, OledOwnershipState::PrepareReleased);
        assert_physical_state(&control, state);
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        cancellation.store(true, std::sync::atomic::Ordering::Release);
        let (ack, receiver) = std::sync::mpsc::channel::<Result<(), String>>();
        drop(receiver);
        let dropped = ack.send(Ok(()));
        assert!(cancellation.load(std::sync::atomic::Ordering::Acquire));
        restore_after_dropped_ack(dropped.is_err(), &mut control, &mut state).unwrap();
        assert_eq!(state, OledOwnershipState::Normal);
        assert_physical_state(&control, state);
    }

    #[test]
    fn shutdown_and_abort_restore_every_ownership_state() {
        let states = [
            OledOwnershipState::QuiescedAttached,
            OledOwnershipState::PrepareHardwareDetachedLeaseAttached,
            OledOwnershipState::PrepareReleased,
            OledOwnershipState::QuiescedCommitted,
            OledOwnershipState::CommittedHardwareDetachedLeaseAttached,
            OledOwnershipState::CommittedReleased,
        ];
        for _operation in ["shutdown", "abort"] {
            for state in states {
                let mut control = FakeControl::for_state(state, Some(Failure::ForceFrame));
                let mut state = state;
                assert!(restore(&mut control, &mut state).is_err());
                assert_ne!(state, OledOwnershipState::Normal);
                assert_physical_state(&control, state);
                control.failure = None;
                restore(&mut control, &mut state).unwrap();
                assert_eq!(state, OledOwnershipState::Normal);
                assert_physical_state(&control, state);
            }
        }
    }
}
