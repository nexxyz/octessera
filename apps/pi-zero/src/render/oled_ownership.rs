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
mod tests;
