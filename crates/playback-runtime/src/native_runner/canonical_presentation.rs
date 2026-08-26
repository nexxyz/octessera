use super::{NativeOledMode, NativeRunner};

impl NativeRunner {
    pub fn is_canonical_menu_presentation(&self) -> bool {
        self.display.oled_mode == NativeOledMode::Normal
            && self.display.oled_splash_until.is_none()
            && self.display.confirm_dialog.is_none()
            && self
                .display
                .setup_portal
                .as_ref()
                .is_none_or(|setup| !setup.visible)
            && self.display.usb_sd_transfer_modal.is_none()
            && self.display.system_info_modal.is_none()
            && self.display.help_popup.is_none()
            && self.display.runtime_error_presentation.is_none()
            && self.aux_mapping_overlay().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::native_menu::NativeMenuAction;

    fn canonical_runner() -> NativeRunner {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.skip_startup_splash();
        runner
    }

    #[test]
    fn canonical_menu_presentation_requires_normal_unobstructed_display() {
        let mut runner = canonical_runner();
        assert!(runner.is_canonical_menu_presentation());

        runner.display.oled_mode = NativeOledMode::Splash;
        assert!(!runner.is_canonical_menu_presentation());
        runner.display.oled_mode = NativeOledMode::Off;
        assert!(!runner.is_canonical_menu_presentation());

        let modal_factories: [fn(&mut NativeRunner); 8] = [
            |runner| {
                runner.display.confirm_dialog = Some(NativeConfirmDialog {
                    title: "Confirm".into(),
                    lines: Vec::new(),
                    options: vec!["Back".into()],
                    cursor: 0,
                    action: NativeMenuAction::NavigateBack,
                    cancel_toast: None,
                    confirm_before_execute: false,
                });
            },
            |runner| {
                runner.display.help_popup = Some(NativeHelpPopup {
                    title: "Help".into(),
                    lines: vec!["Help".into()],
                    scroll: 0,
                });
            },
            |runner| {
                runner.display.usb_sd_transfer_modal = Some(NativeUsbSdTransferModal {
                    title: "Transfer".into(),
                    lines: vec!["Transfer".into()],
                });
            },
            |runner| {
                runner.display.system_info_modal = Some(NativeSystemInfoModal::loading());
            },
            |runner| {
                runner.display.setup_portal = Some(NativeSetupPortalState {
                    status: RuntimeSetupPortalStatus {
                        phase: RuntimeSetupPortalPhase::Starting,
                        disposition: None,
                        portal_suffix: None,
                        reboot_required: false,
                        error_code: None,
                    },
                    request_id: None,
                    revision: None,
                    visible: true,
                });
            },
            |runner| {
                runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
                    title: "Error".into(),
                    lines: vec!["Error".into()],
                });
            },
            |runner| {
                runner.display.oled_splash_until = Some(Instant::now());
            },
            |runner| {
                runner.display.ui.fn_held = true;
                runner.display.fn_hold_started_at = Some(Instant::now() - Duration::from_secs(2));
                runner.aux_bindings[0] = Some(NativeAuxBinding {
                    turn_key: Some("displayBrightness".into()),
                    press_action: None,
                });
            },
        ];
        for install_modal in modal_factories {
            let mut runner = canonical_runner();
            install_modal(&mut runner);
            assert!(!runner.is_canonical_menu_presentation());
        }
    }
}
