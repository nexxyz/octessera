use super::{HardwareRenderCache, HardwareRenderTargets, LatestPresentation};
use super::{OledOwnershipStage, OledRenderControl};

pub(crate) struct HardwareOwnershipControl<'a> {
    pub(crate) targets: &'a mut HardwareRenderTargets,
    pub(crate) cache: &'a mut HardwareRenderCache,
    pub(crate) latest: &'a Option<LatestPresentation>,
}

impl OledRenderControl for HardwareOwnershipControl<'_> {
    fn clear_oled_retry(&mut self) {
        self.cache.clear_oled_retry();
    }

    fn detach_hardware(&mut self) -> Result<(), String> {
        self.targets.oled.detach_preserving()
    }

    fn detach_handoff(&mut self) -> Result<(), String> {
        self.targets
            .oled_handoff
            .as_mut()
            .ok_or_else(|| "OLED ownership handoff is unavailable".to_string())?
            .detach_preserving()
    }

    fn reacquire_handoff(&mut self) -> Result<(), String> {
        self.targets
            .oled_handoff
            .as_mut()
            .ok_or_else(|| "OLED ownership handoff is unavailable".to_string())?
            .reacquire_existing()
    }

    fn reacquire_hardware(&mut self) -> Result<(), String> {
        self.targets.oled.reacquire_existing()
    }

    fn force_latest_frame(&mut self) -> Result<(), String> {
        let latest = self
            .latest
            .as_ref()
            .ok_or_else(|| "OLED restore has no latest native frame".to_string())?;
        latest.force_oled(self.targets, self.cache)
    }
}

pub(crate) fn ownership_stage_for_render(
    stage: OledOwnershipStage,
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    latest: &Option<LatestPresentation>,
    ownership: &mut super::OledOwnershipState,
) -> Result<(), String> {
    let mut control = HardwareOwnershipControl {
        targets,
        cache,
        latest,
    };
    super::handle_stage(stage, &mut control, ownership)
}

pub(crate) fn restore_for_render(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    latest: &Option<LatestPresentation>,
    ownership: &mut super::OledOwnershipState,
) -> Result<(), String> {
    let mut control = HardwareOwnershipControl {
        targets,
        cache,
        latest,
    };
    super::restore(&mut control, ownership)
}

pub(crate) fn restore_after_dropped_ack_for_render(
    ack_dropped: bool,
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    latest: &Option<LatestPresentation>,
    ownership: &mut super::OledOwnershipState,
) -> Result<(), String> {
    let mut control = HardwareOwnershipControl {
        targets,
        cache,
        latest,
    };
    super::restore_after_dropped_ack(ack_dropped, &mut control, ownership)
}
