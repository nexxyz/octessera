#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OledStartupOperation {
    HardwareReset,
    ControllerConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OledStartupPlan {
    preserve_existing: bool,
}

impl OledStartupPlan {
    pub const fn new(preserve_existing: bool) -> Self {
        Self { preserve_existing }
    }

    pub const fn operations(self) -> &'static [OledStartupOperation] {
        if self.preserve_existing {
            &[]
        } else {
            &[
                OledStartupOperation::HardwareReset,
                OledStartupOperation::ControllerConfiguration,
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OledStartupOperation, OledStartupPlan};

    #[test]
    fn adopt_plan_does_not_reset_or_configure_the_controller() {
        assert!(OledStartupPlan::new(true).operations().is_empty());
    }

    #[test]
    fn fresh_plan_resets_before_configuration() {
        assert_eq!(
            OledStartupPlan::new(false).operations(),
            &[
                OledStartupOperation::HardwareReset,
                OledStartupOperation::ControllerConfiguration,
            ]
        );
    }
}
